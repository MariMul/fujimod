//! In-process board transport. Execution yields between bounded, watched windows.
use crate::{backend::Backend, connection::Connection, *};
use fujimod_emulator::session::{Session, checkpoint::Snapshot as Checkpoint};

/// Access is scoped to stopped-board calls, so language owners can retain a board.
pub trait SessionOwner {
    fn with_session<T>(&mut self, call: impl FnOnce(&mut Session) -> Result<T>) -> Result<T>;
}
impl SessionOwner for &mut Session {
    fn with_session<T>(&mut self, call: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
        call(self)
    }
}
impl SessionOwner for Session {
    fn with_session<T>(&mut self, call: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
        call(self)
    }
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Options {
    pub quantum: usize,
    pub window_instructions: u64,
    pub scanout_node: String,
    pub output: String,
    /// Device observation and JSON pointer naming the completed frame counter.
    pub frame_counter_kind: String,
    pub frame_counter_pointer: String,
}
pub struct BoardBackend<S> {
    owner: S,
    options: Options,
    now: u64,
}
pub type BoardRemote<S> = Connection<BoardBackend<S>>;
impl<S: SessionOwner> Connection<BoardBackend<S>> {
    pub fn new(mut owner: S, facts: Facts, options: Options) -> Result<Self> {
        if options.quantum == 0 || options.window_instructions == 0 || options.output.is_empty() {
            return Err("Invalid board continuation options".into());
        }
        let now = owner.with_session(|s| Ok(s.board.now()))?;
        Self::connect(
            BoardBackend {
                owner,
                options,
                now,
            },
            facts,
        )
    }
    pub fn checkpoint(&mut self, previous: Option<&Checkpoint>) -> Result<Checkpoint> {
        self.backend.owner.with_session(|s| s.checkpoint(previous))
    }
    pub fn restore(&mut self, saved: &Checkpoint) -> Result<()> {
        self.backend.owner.with_session(|s| s.restore(saved))?;
        self.backend.now = saved.now();
        Ok(())
    }
    pub fn set_scene(&mut self, scene: &serde_json::Value) -> Result<()> {
        self.backend
            .owner
            .with_session(|s| s.board.set_scene(scene))
    }
    pub async fn advance_until(&mut self, deadline: u64) -> Result<()> {
        while self.now() < deadline {
            self.wait(&ReadSet::default(), deadline).await?;
        }
        Ok(())
    }
}
fn tracked<T>(
    session: &Session,
    reads: &mut ReadSet,
    call: impl FnOnce(&Session) -> Result<T>,
) -> Result<T> {
    session.board.read_set_begin()?;
    let result = call(session);
    let tracked = session.board.read_set_end()?;
    reads.extend(ReadSet {
        stores: tracked.stores,
        devices: tracked.devices,
    });
    result
}
impl<S: SessionOwner> Backend for BoardBackend<S> {
    fn now(&self) -> u64 {
        self.now
    }
    fn memory<'a>(
        &'a mut self,
        address: u64,
        size: usize,
        reads: &'a mut ReadSet,
    ) -> LocalFuture<'a, Vec<u8>> {
        Box::pin(async move {
            self.owner.with_session(|s| {
                tracked(s, reads, |s| {
                    let mut bytes = vec![0; size];
                    s.board.read_physical(address, &mut bytes)?;
                    Ok(bytes)
                })
            })
        })
    }
    fn input<'a>(&'a mut self, input: &'a Input, facts: &'a Facts) -> LocalFuture<'a, ()> {
        Box::pin(async move {
            let id = physical(facts, input.control.id());
            self.owner.with_session(|s| match input.state {
                InputState::Turn(steps) => {
                    let (node, cycles) = s.rotation(id, steps)?;
                    s.rotate(&node, cycles)
                }
                _ => {
                    let driver = s
                        .contact_driver
                        .as_ref()
                        .ok_or("BoardRemote requires checked contact acknowledgments")?;
                    if driver.busy() {
                        return Err("Board contact queue is busy".into());
                    }
                    let actuator = driver
                        .spec
                        .actuators
                        .iter()
                        .find(|a| a.id == id)
                        .ok_or_else(|| format!("No physical actuator for {id}"))?;
                    let index = usize::from(input.state == InputState::Pressed);
                    let pose = actuator
                        .states
                        .get(index)
                        .ok_or("Physical press pose is absent")?;
                    let position = pose.position.clone();
                    s.set_position(id, &position)
                }
            })
        })
    }
    fn accepted(&mut self, input: &Input, reads: &mut ReadSet) -> Result<bool> {
        if matches!(input.state, InputState::Turn(_)) {
            return Ok(true);
        }
        self.owner.with_session(|s| {
            tracked(s, reads, |s| {
                let driver = s
                    .contact_driver
                    .as_ref()
                    .ok_or("Physical acknowledgment driver is absent")?;
                let id = input.control.id();
                // input() resolved aliases; the live panel position identifies the pose.
                let actuator = driver
                    .spec
                    .actuators
                    .iter()
                    .find(|a| a.id == id)
                    .ok_or_else(|| format!("No checked actuator for {id}"))?;
                let pose = &actuator.states[usize::from(input.state == InputState::Pressed)];
                for check in pose.ready.iter().chain(&pose.expected) {
                    let address = if let Some(pointer) = check.pointer {
                        let mut raw = [0; 4];
                        s.board.read_physical(pointer, &mut raw)?;
                        u64::from(u32::from_le_bytes(raw))
                            .checked_add(check.address)
                            .ok_or("Acknowledgment address overflow")?
                    } else {
                        check.address
                    };
                    let mut raw = vec![0; check.bytes.len()];
                    s.board.read_physical(address, &mut raw)?;
                    if raw != check.bytes {
                        return Ok(false);
                    }
                }
                Ok(true)
            })
        })
    }
    fn frame<'a>(
        &'a mut self,
        facts: &'a Facts,
        reads: &'a mut ReadSet,
    ) -> LocalFuture<'a, Capture> {
        Box::pin(async move {
            let options = &self.options;
            self.owner
                .with_session(|s| {
                    tracked(s, reads, |s| {
                        let counters = s.board.device_states(&options.frame_counter_kind)?;
                        let sequence = counters
                            .pointer(&options.frame_counter_pointer)
                            .and_then(serde_json::Value::as_u64)
                            .ok_or("Completed frame counter is unavailable")?;
                        let frame = s
                            .recording_frame(
                                &options.scanout_node,
                                facts.firmware.scanout.instance as usize,
                                false,
                            )?
                            .ok_or("Scanout is inactive")?;
                        let rgb = frame.rgb.clone().ok_or("Scanout omitted RGB")?;
                        Ok(Capture {
                            frame: fujimod_recording::Frame::rgb(
                                frame.width as u32,
                                frame.height as u32,
                                rgb,
                            )?,
                            time: s.board.now(),
                            clock: Clock::GuestTicks,
                            output: options.output.clone(),
                            sequence,
                            reads: ReadSet::default(),
                        })
                    })
                })
                .map(|mut capture| {
                    capture.reads = reads.clone();
                    capture
                })
        })
    }
    fn wait<'a>(&'a mut self, reads: &'a ReadSet, deadline: u64) -> LocalFuture<'a, ()> {
        Box::pin(async move {
            if self.now >= deadline {
                return Ok(());
            }
            let options = &self.options;
            self.now = self.owner.with_session(|s| {
                let maps = serde_json::from_value(s.definition["execution"]["mappings"].clone())
                    .map_err(|e| e.to_string())?;
                s.board
                    .continuation_begin(reads.stores.clone(), reads.devices.clone(), maps)?;
                let result = s.run_with_limits(
                    options.window_instructions,
                    options.quantum,
                    None,
                    Some(deadline),
                );
                let cleanup = s.board.continuation_end();
                result.and(cleanup.map(|_| ()))?;
                Ok(s.board.now())
            })?;
            futures_lite::future::yield_now().await;
            Ok(())
        })
    }
    fn control_position(&mut self, id: &str) -> Result<Option<String>> {
        self.owner.with_session(|s| {
            Ok(s.controls
                .as_ref()
                .and_then(|p| p.positions().get(id).cloned()))
        })
    }
}
pub(crate) fn physical<'a>(facts: &'a Facts, id: &'a str) -> &'a str {
    facts
        .firmware
        .control_names
        .get(id)
        .map(String::as_str)
        .unwrap_or(id)
}

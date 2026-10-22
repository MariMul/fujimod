//! Host-only session dispatch. Unsupported lifecycle and input policies refuse.
use super::session::Session;
use fujimod_emulator::session::history::Input;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    action: String,
    options: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UsbNode {
    node: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UsbPacket {
    node: String,
    endpoint: u8,
    #[serde(default)]
    size: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    revision: Option<u64>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Read {
    path: String,
    revision: Option<u64>,
    max_bytes: Option<usize>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Insert {
    present: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    limit: Option<usize>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WatchPcs {
    addresses: Vec<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceWorkers {
    count: Option<usize>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceStates {
    kind: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Position {
    id: String,
    position: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Press {
    id: String,
    stage: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Release {
    id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rotate {
    id: String,
    steps: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Touch {
    id: String,
    phase: String,
    x: u16,
    y: u16,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemoryRead {
    address: String,
    size: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointId {
    id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rollback {
    tick: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Power {
    on: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cycle {
    inputs: Vec<super::lifecycle::BootInput>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    path: String,
    offset: usize,
    length: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Files {
    files: Vec<File>,
    #[serde(default)]
    inputs: Vec<super::lifecycle::BootInput>,
}
fn options<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}
impl Session {
    fn card_node(&self) -> Result<&str, String> {
        self.camera
            .as_ref()
            .and_then(|m| m.card.as_ref())
            .map(|c| c.node.as_str())
            .ok_or("session has no declared SD card".into())
    }
    fn card_state(&self) -> Result<Value, String> {
        match self.camera.as_ref().and_then(|m| m.card.as_ref()) {
            Some(c) => self.core.card_state(&c.node),
            None => Ok(
                json!({"supported":false,"present":false,"busy":false,"revision":null,"capacity_bytes":0}),
            ),
        }
    }
    pub(super) fn session_state(&self) -> Result<Value, String> {
        let mut state = if self.core.controls.is_some() {
            self.core.control_state()?
        } else {
            json!({"inputs":[],"positions":{}})
        };
        state["connections"] = json!([]);
        state["card"] = self.card_state()?;
        state["poweredOn"] = json!(self.powered_on);
        Ok(state)
    }
    pub fn command(&mut self, handle: u32, request: &[u8], bytes: &[u8]) -> Result<(), String> {
        let c: Command = serde_json::from_slice(request).map_err(|e| e.to_string())?;
        if !bytes.is_empty()
            && !matches!(
                c.action.as_str(),
                "replay" | "restore_durable" | "restart_card" | "usb_setup" | "usb_out"
            )
        {
            return Err(format!("binary upload is not supported for {}", c.action));
        }
        self.result_bytes.clear();
        let result = match c.action.as_str() {
            "usb_status" => {
                let o: UsbNode = options(c.options)?;
                self.core.usb_status(&o.node)?
            }
            "usb_reset" => {
                let o: UsbNode = options(c.options)?;
                self.core.usb_reset(&o.node)?;
                json!({})
            }
            "usb_setup" => {
                let o: UsbNode = options(c.options)?;
                self.core.usb_setup(&o.node, bytes)?;
                json!({})
            }
            "usb_out" => {
                let o: UsbPacket = options(c.options)?;
                self.core.usb_out(&o.node, o.endpoint, bytes)?;
                json!({})
            }
            "usb_in" => {
                let o: UsbPacket = options(c.options)?;
                self.result_bytes = self.core.usb_in(&o.node, o.endpoint, o.size)?;
                json!({})
            }
            "usb_in_token" => {
                let o: UsbPacket = options(c.options)?;
                self.core.usb_in_token(&o.node, o.endpoint)?;
                json!({})
            }
            "state" => {
                options::<Empty>(c.options)?;
                self.session_state()?
            }
            "card_state" => {
                options::<Empty>(c.options)?;
                self.card_state()?
            }
            "card_tree" => {
                let o: Revision = options(c.options)?;
                self.core.card_tree(self.card_node()?, o.revision)?
            }
            "card_read" => {
                let o: Read = options(c.options)?;
                self.result_bytes = self.core.card_read(
                    self.card_node()?,
                    &o.path,
                    o.revision,
                    o.max_bytes.unwrap_or(64 << 20),
                )?;
                json!({})
            }
            "card_insert" => {
                let o: Insert = options(c.options)?;
                let name = self.card_node()?.to_owned();
                self.mark_input()?;
                self.apply_input(Input::CardContacts {
                    node: name,
                    present: o.present,
                    protected: false,
                })?;
                self.session_state()?
            }
            "card_format" => {
                options::<Empty>(c.options)?;
                let card = self
                    .camera
                    .as_ref()
                    .and_then(|m| m.card.as_ref())
                    .ok_or("session has no declared SD card")?;
                let (node, format) = (card.node.clone(), card.format.clone());
                self.mark_input()?;
                self.apply_input(Input::CardFormat {
                    node,
                    options: format,
                    files: vec![],
                })?;
                self.session_state()?
            }
            "set_position" => {
                let o: Position = options(c.options)?;
                // Validate the complete contact network before taking an Undo mark.
                self.core.admit_position(&o.id, &o.position)?;
                self.mark_input()?;
                self.apply_input(Input::Position {
                    control: o.id,
                    position: o.position,
                })?;
                self.session_state()?
            }
            "press" => {
                let o: Press = options(c.options)?;
                self.core
                    .admit_contact_input(&o.id, o.stage.as_deref(), false)?;
                self.mark_input()?;
                self.apply_input(Input::Press {
                    control: o.id,
                    stage: o.stage,
                })?;
                self.session_state()?
            }
            "release" => {
                let o: Release = options(c.options)?;
                self.core.admit_contact_input(&o.id, None, true)?;
                self.mark_input()?;
                self.apply_input(Input::Release { control: o.id })?;
                self.session_state()?
            }
            "rotate" => {
                let o: Rotate = options(c.options)?;
                let (node, cycles) = self.core.rotation(&o.id, o.steps)?;
                self.mark_input()?;
                self.apply_input(Input::Rotate { node, cycles })?;
                self.session_state()?
            }
            "touch" => {
                let o: Touch = options(c.options)?;
                let (node, script) = self.core.touch_input(&o.id, &o.phase, o.x, o.y)?;
                self.mark_input()?;
                self.apply_input(Input::Touch { node, script })?;
                self.session_state()?
            }
            "preview" | "preview_state" => {
                options::<Empty>(c.options)?;
                let preview = self
                    .camera
                    .as_ref()
                    .and_then(|m| m.preview.clone())
                    .ok_or("session has no declared preview policy")?;
                if c.action == "preview" {
                    self.mark_input()?;
                    self.apply_input(Input::Frames {
                        node: preview.node.clone(),
                        period: preview.period_ticks,
                        cadence: preview.cadence,
                    })?;
                }
                json!({"ticksPerSecond":preview.tick_hz.to_string(),"state":self.core.frame_state(&preview.node)?})
            }
            "device_workers" => {
                let o: DeviceWorkers = options(c.options)?;
                if let Some(count) = o.count {
                    self.core.board.set_device_workers(count)?;
                }
                json!({"count":self.core.board.device_worker_preference(),"effective":self.core.board.device_worker_count()})
            }
            "device_states" => {
                let o: DeviceStates = options(c.options)?;
                if o.kind.is_empty() {
                    return Err("device kind must not be empty".into());
                }
                json!({"states":self.core.board.device_states(&o.kind)?})
            }
            "memory_read" => {
                let o: MemoryRead = options(c.options)?;
                if o.size == 0 || o.size > 64 << 20 {
                    return Err("memory read requires 1..67108864 bytes".into());
                }
                let address: u64 = o
                    .address
                    .parse()
                    .map_err(|_| "memory address is not a u64 decimal string")?;
                self.result_bytes.resize(o.size, 0);
                self.core
                    .board
                    .read_physical(address, &mut self.result_bytes)?;
                json!({"address":address.to_string(),"size":o.size})
            }
            "watch_pcs" => {
                let o: WatchPcs = options(c.options)?;
                self.core.board.watch_pcs(&o.addresses)?;
                json!({"addresses":o.addresses})
            }
            "profile" => {
                let o: Profile = options(c.options)?;
                let limit = o.limit.unwrap_or(1000);
                if limit > 10000 {
                    return Err("profile limit exceeds 10000".into());
                }
                let entries = self.core.board.pc_entries();
                json!({"ticks":self.core.board.now(),"instructions":self.core.board.instruction_counts(),"pcEntries":entries.into_iter().take(limit).collect::<Vec<_>>(),"failed":self.failed})
            }
            "error_report" => {
                options::<Empty>(c.options)?;
                json!({"report":self.core.board.state()})
            }
            "fingerprint" => {
                options::<Empty>(c.options)?;
                super::fingerprint::session(&self.core, self.powered_on)?
            }
            "checkpoint" => {
                options::<Empty>(c.options)?;
                self.checkpoint(handle)?
            }
            "restore" => {
                let o: CheckpointId = options(c.options)?;
                self.restore_checkpoint(&o.id)?
            }
            "undo" => {
                options::<Empty>(c.options)?;
                self.undo()?
            }
            "rollback" => {
                let o: Rollback = options(c.options)?;
                self.rollback(
                    o.tick
                        .parse()
                        .map_err(|_| "rollback tick is not a u64 decimal string")?,
                )?
            }
            "replay" => {
                options::<Empty>(c.options)?;
                self.replay(handle, bytes)?
            }
            "history" => {
                options::<Empty>(c.options)?;
                self.history_log(handle)?
            }
            "power" => {
                let o: Power = options(c.options)?;
                self.power(o.on)?;
                self.session_state()?
            }
            "power_cycle" => {
                let o: Cycle = options(c.options)?;
                self.power_cycle(&o.inputs)?;
                self.session_state()?
            }
            "restart_card" => {
                let o: Files = options(c.options)?;
                let mut files = Vec::new();
                let mut end = 0usize;
                for file in o.files {
                    if file.offset != end {
                        return Err("card file extents must be contiguous".into());
                    }
                    end = file
                        .offset
                        .checked_add(file.length)
                        .ok_or("card file extent overflow")?;
                    files.push((
                        file.path,
                        bytes
                            .get(file.offset..end)
                            .ok_or("card file extent exceeds upload")?
                            .to_vec(),
                    ));
                }
                if end != bytes.len() {
                    return Err("unused card file upload bytes".into());
                }
                self.restart_card(&files, &o.inputs)?;
                self.session_state()?
            }
            "save" => {
                options::<Empty>(c.options)?;
                self.save_durable()?
            }
            "restore_durable" => {
                options::<Empty>(c.options)?;
                self.restore_durable(bytes)?;
                self.session_state()?
            }
            _ => return Err(format!("unknown session action {}", c.action)),
        };
        self.result_json = serde_json::to_vec(&result).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn reserve_scene(&mut self, width: u32, height: u32) -> Result<(), String> {
        if self
            .camera
            .as_ref()
            .and_then(|m| m.scene.as_ref())
            .is_none()
        {
            return Err("session has no declared scene input".into());
        }
        let len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(3))
            .filter(|n| *n <= 64 << 20)
            .ok_or("scene RGB exceeds 64 MiB")?;
        if width == 0 || height == 0 || width > 8192 || height > 8192 {
            return Err("invalid scene RGB dimensions".into());
        }
        self.scene_upload.resize(len, 0);
        self.scene_dimensions = Some((width as usize, height as usize));
        Ok(())
    }
    pub fn commit_scene(&mut self, enabled: u32) -> Result<(), String> {
        let scene = self
            .camera
            .as_ref()
            .and_then(|m| m.scene.as_ref())
            .ok_or("session has no declared scene input")?;
        let (images, sensor, node, white_luminance) = (
            scene.images.clone(),
            scene.sensor.clone(),
            scene.node.clone(),
            scene.white_luminance,
        );
        match enabled {
            0 => {
                if sensor.is_some() {
                    return Err(
                        "analytic sensor reset is not yet available in the Rust session".into(),
                    );
                }
                self.mark_input()?;
                self.apply_input(Input::Analytic { source: images })?;
            }
            1 => {
                let (width, height) = self
                    .scene_dimensions
                    .ok_or("no scene RGB upload is reserved")?;
                self.mark_input()?;
                if let Some(sensor) = sensor {
                    self.apply_input(Input::SensorRgb {
                        source: sensor,
                        width,
                        height,
                        bytes: self.scene_upload.clone(),
                        white_luminance: white_luminance
                            .ok_or("sensor scene has no declared white luminance")?,
                    })?;
                    self.apply_input(Input::SensorImage { source: images })?;
                } else {
                    self.apply_input(Input::Rgb {
                        source: images,
                        width,
                        height,
                        bytes: self.scene_upload.clone(),
                    })?;
                }
            }
            _ => return Err("scene enabled must be zero or one".into()),
        }
        self.apply_input(Input::Wake { node })
    }
}

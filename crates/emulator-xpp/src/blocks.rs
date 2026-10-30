//! X-Processor Pro register blocks with no work in flight.
//!
//! Several imaging blocks of X-T20 2.11 are configured from section 5 with
//! plain word writes, often through the table copier 0xc41d30, and read back
//! with read-modify-write updates. Some have handshakes that wait for a
//! status bit after a stop or start: 0xc3c0c0 writes 0 to 0xfff3b000 and
//! waits until bit 0 reads set; 0xc3a954 does the same for the units at
//! 0xfff55000 + 0x1000*n, and their stop 0xc3be40 waits for +0xd8 bit 8.
//! Each block is a register file: words read back their last write and start
//! at 0, and the declared idle bits always read set, as no work is ever in
//! flight. What the blocks compute and any data they move are not modeled.
//!
//! A block may declare [`Frames`]: while its start bit is set it finishes a
//! frame at every multiple of the frame period and raises one interrupt edge;
//! its stopped bit reads set only while the start bit is clear. The live-view
//! output units at 0xfff55000 + 0x1000*n start with +0xc8 bit 28 (0xc3a98c),
//! their stop clears it and waits for +0xd8 bit 8, and their edge handlers
//! (GIC 107 + n) report the frame to the imaging dispatcher.
//!
//! A live-view output unit may declare a uniform image: before each frame
//! interrupt it writes `lines` lines of packed 4:2:2 pixel pairs (Y0 Y1 Cb Cr)
//! to the buffer at +0xc0, +0xc4 pixels wide (two bytes per pixel). The
//! firmware moves +0xc0 through its buffers between frames (unit 1:
//! 0xa7d6000 + 0x180000*k, 720 pixels) and the display shows the buffer the
//! unit finished last. Other data the unit or the rest of the pipeline moves
//! is not modeled.
//!
//! A block may instead declare a [`Job`]: each write that sets its start bit
//! while no job is in flight finishes one job after the declared ticks. The
//! job sets the status bit, the interrupt line is high while a status bit is
//! set, and writing a status bit clears it, as for the face detection engine.
//! The block at 0xfff7f000 (driver 0x1558424..0x1558a94) starts with +0x04
//! bit 0 (0x15589c4); its handler 0x1558108 reads +0x44, writes 1 to it and
//! calls the job's callback. A job without a status bit (the development
//! engines 0xfff98000 and 0xfffc3000, whose handlers read nothing) pulses
//! its interrupt line instead. Idle bits may share the status word when they
//! do not overlap the status bit (0xfffc4000: +0x3c bit 20 idle, bit 0
//! done). What a job computes is not modeled.
use fujimod_emulator_devices::{Cx, Device};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Frame law of a block: (offset, mask) of the start and stopped bits, the
/// interrupt line and the frame period in ticks.
#[derive(Clone, Copy, Debug)]
pub struct Frames {
    pub start: (u64, u32),
    pub stopped: (u64, u32),
    pub irq: u32,
    pub period: u64,
}
/// Job law of a block: (offset, mask) of the start and status bits, the
/// interrupt line (level with a status bit, pulsed without) and the ticks a
/// job takes.
#[derive(Clone, Copy, Debug)]
pub struct Job {
    pub start: (u64, u32),
    pub status: Option<(u64, u32)>,
    pub irq: u32,
    pub ticks: u64,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    words: BTreeMap<u64, u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    due: Option<u64>,
    #[serde(default, skip_serializing_if = "is_zero")]
    frames: u64,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    status: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    jobs: u64,
}
const BUFFER: u64 = 0xc0;
const WIDTH: u64 = 0xc4;
fn is_zero(n: &u64) -> bool {
    *n == 0
}
fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}
pub struct Block {
    base: u64,
    size: u64,
    /// (offset, mask) of status bits that always read set.
    idle: Vec<(u64, u32)>,
    frames: Option<Frames>,
    job: Option<Job>,
    /// Lines and the Y0 Y1 Cb Cr pixel pair of a uniform live-view image.
    image: Option<(u32, [u8; 4])>,
    state: State,
}
impl Block {
    pub fn new(
        base: u64,
        size: u64,
        idle: Vec<(u64, u32)>,
        frames: Option<Frames>,
    ) -> Result<Self, String> {
        let bit = |(o, mask): (u64, u32)| o & 3 != 0 || o >= size || mask == 0;
        if base & 0xfff != 0
            || size == 0
            || size > 0x1000
            || size & 3 != 0
            || idle.iter().any(|&b| bit(b))
            || frames.is_some_and(|f| {
                bit(f.start)
                    || bit(f.stopped)
                    || !(32..512).contains(&f.irq)
                    || f.period == 0
                    || idle.contains(&f.stopped)
            })
        {
            return Err("Invalid X-Processor Pro register block".into());
        }
        Ok(Self {
            base,
            size,
            idle,
            frames,
            job: None,
            image: None,
            state: State::default(),
        })
    }
    /// One job per start, finished after the declared ticks.
    pub fn with_job(mut self, job: Job) -> Result<Self, String> {
        let bit = |(o, mask): (u64, u32)| o & 3 != 0 || o >= self.size || mask == 0;
        if self.frames.is_some()
            || bit(job.start)
            || job.status.is_some_and(|status| {
                bit(status)
                    || job.start.0 == status.0
                    || self
                        .idle
                        .iter()
                        .any(|&(o, mask)| o == status.0 && mask & status.1 != 0)
            })
            || !(32..512).contains(&job.irq)
            || job.ticks == 0
        {
            return Err("Invalid X-Processor Pro register block job".into());
        }
        self.job = Some(job);
        Ok(self)
    }
    /// A uniform live-view image written at each frame: lines and Y, Cb, Cr.
    pub fn with_live_view_image(mut self, lines: u32, ycbcr: [u8; 3]) -> Result<Self, String> {
        if self.frames.is_none() || self.size <= WIDTH || !(1..=4096).contains(&lines) {
            return Err("Invalid X-Processor Pro live-view image".into());
        }
        let [y, cb, cr] = ycbcr;
        self.image = Some((lines, [y, y, cb, cr]));
        Ok(self)
    }
    fn write_image(&self, cx: &mut Cx<'_>) {
        let Some((lines, pair)) = self.image else {
            return;
        };
        let word = |o| self.state.words.get(&o).copied().unwrap_or(0);
        let (address, width) = (word(BUFFER), word(WIDTH));
        if width == 0 {
            return;
        }
        if width & 1 != 0 || width > 0x2000 {
            return cx.fail("Unsupported live-view line width");
        }
        let image = pair.repeat(width as usize / 2 * lines as usize);
        if cx.ram_write(u64::from(address), &image).is_err() {
            cx.fail("Live-view buffer outside RAM");
        }
    }
    fn started(&self, frames: &Frames) -> bool {
        let (offset, mask) = frames.start;
        self.state.words.get(&offset).is_some_and(|w| w & mask != 0)
    }
    /// Register window size in bytes.
    pub fn window(&self) -> u64 {
        self.size
    }
    /// The stopped bit reads the run state, not the last write.
    fn stopped_mask(&self, offset: u64) -> u32 {
        self.frames
            .filter(|f| f.stopped.0 == offset)
            .map_or(0, |f| f.stopped.1)
    }
    fn offset(&self, address: u64, size: u8) -> Option<u64> {
        let offset = address.checked_sub(self.base)?;
        (size == 4 && offset & 3 == 0 && offset < self.size).then_some(offset)
    }
}
impl Device for Block {
    fn name(&self) -> &'static str {
        "xpp-register-block"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        let Some(offset) = self.offset(address, size) else {
            cx.unmodeled();
            return 0;
        };
        let mut idle = self
            .idle
            .iter()
            .filter(|(o, _)| *o == offset)
            .fold(0, |bits, (_, mask)| bits | mask);
        if let Some((_, mask)) = self.job.and_then(|j| j.status).filter(|s| s.0 == offset) {
            return u64::from(self.state.status & mask | idle);
        }
        if let Some(frames) = self.frames.filter(|f| f.stopped.0 == offset) {
            idle = idle & !frames.stopped.1
                | if self.started(&frames) {
                    0
                } else {
                    frames.stopped.1
                };
        }
        u64::from(
            self.state.words.get(&offset).copied().unwrap_or(0) & !self.stopped_mask(offset) | idle,
        )
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        if let (Some(job), Some(offset), Ok(value)) =
            (self.job, self.offset(address, size), u32::try_from(value))
        {
            if job.status.is_some_and(|s| s.0 == offset) {
                self.state.status &= !value;
                if self.state.status == 0 {
                    cx.set_irq(job.irq, false);
                }
                return;
            }
            if offset == job.start.0 && value & job.start.1 != 0 && self.state.due.is_none() {
                self.state.due = cx.now().checked_add(job.ticks);
                if self.state.due.is_none() {
                    cx.fail("Register block job deadline overflow");
                }
            }
        }
        match (self.offset(address, size), u32::try_from(value)) {
            (Some(offset), Ok(0)) => {
                self.state.words.remove(&offset);
            }
            (Some(offset), Ok(value)) => {
                self.state.words.insert(offset, value);
            }
            _ => return cx.unmodeled(),
        }
        if let Some(frames) = self.frames {
            self.state.due = match (self.started(&frames), self.state.due) {
                (false, _) => None,
                (true, Some(at)) => Some(at),
                (true, None) => (cx.now() / frames.period)
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(frames.period)),
            };
        }
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        if let Some(job) = self.job {
            if self.state.due.is_some_and(|at| at <= cx.now()) {
                self.state.due = None;
                self.state.jobs += 1;
                match job.status {
                    Some((_, mask)) => {
                        self.state.status |= mask;
                        cx.set_irq(job.irq, true);
                    }
                    None => {
                        cx.set_irq(job.irq, true);
                        cx.set_irq(job.irq, false);
                    }
                }
            }
            return;
        }
        let (Some(frames), Some(due)) = (self.frames, self.state.due) else {
            return;
        };
        if cx.now() < due {
            return;
        }
        self.write_image(cx);
        cx.set_irq(frames.irq, true);
        cx.set_irq(frames.irq, false);
        self.state.frames += 1;
        let step = ((cx.now() - due) / frames.period + 1).checked_mul(frames.period);
        self.state.due = step.and_then(|step| due.checked_add(step));
        if self.state.due.is_none() {
            cx.fail("Register block frame deadline overflow");
        }
    }
    fn next_deadline(&self, _now: u64) -> Option<u64> {
        self.state.due
    }
    fn save(&self) -> Value {
        serde_json::to_value(&self.state).unwrap_or(Value::Null)
    }
    fn restore(&mut self, value: &Value) -> Result<(), String> {
        let state: State = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        let started = self.frames.is_some_and(|f| {
            let (offset, mask) = f.start;
            state.words.get(&offset).is_some_and(|w| w & mask != 0)
        });
        if state
            .words
            .iter()
            .any(|(o, v)| o & 3 != 0 || *o >= self.size || *v == 0)
            || self.frames.is_some() && started != state.due.is_some()
            || self.frames.is_none() && self.job.is_none() && state.due.is_some()
            || self.job.is_none() && (state.status != 0 || state.jobs != 0)
            || self.job.is_some_and(|j| match j.status {
                Some((offset, mask)) => {
                    state.status & !mask != 0 || state.words.contains_key(&offset)
                }
                None => state.status != 0,
            })
            || self
                .frames
                .zip(state.due)
                .is_some_and(|(f, at)| at % f.period != 0)
        {
            return Err("Invalid register block checkpoint".into());
        }
        self.state = state;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fujimod_emulator_devices::test_host::{TestHost, read, write};

    const BASE: u64 = 0xfff55000;

    #[test]
    fn words_read_back_and_idle_bits_read_set() {
        let mut unit = Block::new(BASE, 0x200, vec![(0, 1), (0xd8, 0x100)], None).unwrap();
        let mut h = TestHost::new();
        // 0xc3a954: stop, then wait for bit 0.
        write(&mut unit, &mut h, BASE, 0);
        assert_eq!(read(&mut unit, &mut h, BASE), 1);
        // 0xc3ab78 sets +0xd8 bit 0; 0xc3be40 waits for bit 8.
        write(&mut unit, &mut h, BASE + 0xd8, 1);
        assert_eq!(read(&mut unit, &mut h, BASE + 0xd8), 0x101);
        write(&mut unit, &mut h, BASE + 0xc0, 0xa7d_6000);
        let saved = unit.save();
        write(&mut unit, &mut h, BASE + 0xc0, 0);
        unit.restore(&saved).unwrap();
        assert_eq!(read(&mut unit, &mut h, BASE + 0xc0), 0xa7d_6000);
        let mut cx = h.cx();
        unit.read(&mut cx, BASE + 0x200, 4);
        assert!(cx.is_unmodeled());
        assert!(Block::new(BASE, 0x200, vec![(0x200, 1)], None).is_err());
    }

    #[test]
    fn started_blocks_finish_a_frame_each_period() {
        use fujimod_emulator_devices::test_host::advance;
        let frames = Frames {
            start: (0xc8, 1 << 28),
            stopped: (0xd8, 0x100),
            irq: 107,
            period: 10,
        };
        assert!(Block::new(BASE, 0x200, vec![(0xd8, 0x100)], Some(frames)).is_err());
        let mut unit = Block::new(BASE, 0x200, vec![(0, 1)], Some(frames)).unwrap();
        let mut h = TestHost::new();
        assert_eq!(read(&mut unit, &mut h, BASE + 0xd8), 0x100);
        assert_eq!(unit.next_deadline(0), None);
        h.now = 3;
        write(&mut unit, &mut h, BASE + 0xc8, 1 << 28 | 1 << 24);
        assert_eq!(read(&mut unit, &mut h, BASE + 0xd8), 0);
        assert_eq!(unit.next_deadline(3), Some(10));
        // 0xc3abe8 sets bit 0 for each frame; bit 8 keeps reading the run state.
        write(&mut unit, &mut h, BASE + 0xd8, 0x101);
        assert_eq!(read(&mut unit, &mut h, BASE + 0xd8), 1);
        advance(&mut unit, &mut h, 25);
        assert_eq!(unit.state.frames, 1);
        assert_eq!(unit.next_deadline(25), Some(30));
        let saved = unit.save();
        write(&mut unit, &mut h, BASE + 0xc8, 1 << 24);
        assert_eq!(read(&mut unit, &mut h, BASE + 0xd8), 0x101);
        assert_eq!(unit.next_deadline(25), None);
        unit.restore(&saved).unwrap();
        assert_eq!(unit.next_deadline(25), Some(30));
        assert!(
            unit.restore(&serde_json::json!({ "words": [] , "due": 30 }))
                .is_err()
        );
    }

    #[test]
    fn a_started_job_finishes_once_and_holds_its_level_interrupt() {
        use fujimod_emulator_devices::test_host::advance;
        const ENGINE: u64 = 0xfff7f000;
        let job = Job {
            start: (0x04, 1),
            status: Some((0x44, 1)),
            irq: 128,
            ticks: 100,
        };
        let block = || Block::new(ENGINE, 0x1000, vec![(0, 1), (0x40, 0x1_0001)], None).unwrap();
        assert!(block().with_job(Job { ticks: 0, ..job }).is_err());
        assert!(
            block()
                .with_job(Job {
                    status: Some((0x40, 1)),
                    ..job
                })
                .is_err()
        );
        let frames = Frames {
            start: (0xc8, 1 << 28),
            stopped: (0xd8, 0x100),
            irq: 107,
            period: 10,
        };
        assert!(
            Block::new(ENGINE, 0x1000, vec![], Some(frames))
                .unwrap()
                .with_job(job)
                .is_err()
        );
        let mut engine = block().with_job(job).unwrap();
        let mut h = TestHost::new();
        // 0x1558468 writes 1 to the status; 0x15589c4 waits for +0x40 and
        // sets +0x04 bit 0.
        write(&mut engine, &mut h, ENGINE + 0x44, 1);
        assert_eq!(read(&mut engine, &mut h, ENGINE + 0x40), 0x1_0001);
        h.now = 5;
        write(&mut engine, &mut h, ENGINE + 0x04, 1);
        assert_eq!(engine.next_deadline(5), Some(105));
        // A second start while the job runs does not move it.
        write(&mut engine, &mut h, ENGINE + 0x04, 1);
        assert_eq!(engine.next_deadline(5), Some(105));
        advance(&mut engine, &mut h, 104);
        assert_eq!(read(&mut engine, &mut h, ENGINE + 0x44), 0);
        advance(&mut engine, &mut h, 105);
        assert_eq!(read(&mut engine, &mut h, ENGINE + 0x44), 1);
        assert_eq!(engine.state.status, 1);
        assert_eq!(engine.next_deadline(105), None);
        let saved = engine.save();
        // 0x1558108 reads the status and writes 1 to acknowledge it.
        write(&mut engine, &mut h, ENGINE + 0x44, 1);
        assert_eq!(engine.state.status, 0);
        assert_eq!(read(&mut engine, &mut h, ENGINE + 0x44), 0);
        assert_eq!(read(&mut engine, &mut h, ENGINE + 0x04), 1);
        engine.restore(&saved).unwrap();
        assert_eq!(read(&mut engine, &mut h, ENGINE + 0x44), 1);
        assert!(block().restore(&saved).is_err());
        write(&mut engine, &mut h, ENGINE + 0x04, 1);
        advance(&mut engine, &mut h, 205);
        assert_eq!(engine.state.jobs, 2);
    }

    #[test]
    fn live_view_units_write_the_declared_image_each_frame() {
        use fujimod_emulator_devices::test_host::advance;
        let frames = Frames {
            start: (0xc8, 1 << 28),
            stopped: (0xd8, 0x100),
            irq: 108,
            period: 10,
        };
        let block = || Block::new(BASE, 0x200, vec![(0, 1)], Some(frames)).unwrap();
        assert!(
            Block::new(BASE, 0x200, vec![], None)
                .unwrap()
                .with_live_view_image(480, [73, 114, 138])
                .is_err()
        );
        assert!(block().with_live_view_image(0, [73, 114, 138]).is_err());
        let mut unit = block().with_live_view_image(3, [73, 114, 138]).unwrap();
        let mut h = TestHost::new();
        write(&mut unit, &mut h, BASE + BUFFER, 0x1000);
        write(&mut unit, &mut h, BASE + WIDTH, 4);
        write(&mut unit, &mut h, BASE + 0xc8, 1 << 28);
        advance(&mut unit, &mut h, 10);
        assert_eq!(h.ram[0x1000..0x1008], [73, 73, 114, 138, 73, 73, 114, 138]);
        assert_eq!(h.ram[0x1014..0x1018], [73, 73, 114, 138]);
        assert_eq!(h.ram[0x1018], 0);
        write(&mut unit, &mut h, BASE + BUFFER, 0x2000);
        advance(&mut unit, &mut h, 20);
        assert_eq!(h.ram[0x2000..0x2004], [73, 73, 114, 138]);
        write(&mut unit, &mut h, BASE + WIDTH, 3);
        h.now = 30;
        let mut cx = h.cx();
        unit.advance(&mut cx);
        assert_eq!(cx.refusal(), Some("Unsupported live-view line width"));
    }

    #[test]
    fn development_engine_jobs_share_idle_status_words_or_have_no_status() {
        use fujimod_emulator_devices::test_host::advance;
        // 0xfffc4000: start +0x00 bit 0, done +0x3c bit 0 beside idle bit 20.
        const ENGINE: u64 = 0xfffc4000;
        let job = Job {
            start: (0, 1),
            status: Some((0x3c, 1)),
            irq: 132,
            ticks: 10,
        };
        let block = |idle| Block::new(ENGINE, 0x1000, idle, None).unwrap();
        assert!(block(vec![(0x3c, 1 << 20 | 1)]).with_job(job).is_err());
        let mut engine = block(vec![(0, 1 << 28), (0x3c, 1 << 20)])
            .with_job(job)
            .unwrap();
        let mut h = TestHost::new();
        assert_eq!(read(&mut engine, &mut h, ENGINE + 0x3c), 1 << 20);
        write(&mut engine, &mut h, ENGINE, 1);
        advance(&mut engine, &mut h, 10);
        assert_eq!(read(&mut engine, &mut h, ENGINE + 0x3c), 1 << 20 | 1);
        write(&mut engine, &mut h, ENGINE + 0x3c, 1);
        assert_eq!(read(&mut engine, &mut h, ENGINE + 0x3c), 1 << 20);
        // 0xfff98000: start +0x04 bit 0, no status; the job pulses interrupt 130.
        let job = Job {
            start: (4, 1),
            status: None,
            irq: 130,
            ticks: 10,
        };
        let mut engine = Block::new(0xfff98000, 0x1000, vec![(0, 1)], None)
            .unwrap()
            .with_job(job)
            .unwrap();
        write(&mut engine, &mut h, 0xfff98004, 1);
        assert_eq!(engine.next_deadline(10), Some(20));
        advance(&mut engine, &mut h, 20);
        assert_eq!(engine.state.status, 0);
        assert_eq!(engine.state.jobs, 1);
        assert_eq!(engine.next_deadline(20), None);
        let saved = engine.save();
        engine.restore(&saved).unwrap();
    }
}

//! X-Processor Pro display frame interrupts.
//!
//! An interrupt law, not a register model of the display controller: each
//! declared line receives one edge per frame period, counted from tick zero.
//! The X-T20 2.11 frame handlers (0xc9d854, 0xc9d994) keep their callback
//! state in RAM and read no controller register, so nothing is acknowledged.
//! Periods that pass between device calls raise one edge per line. The period
//! is a declared input; the panel rate is not measured.
//!
//! [`Output`] is the register window of one display output: word registers
//! read back their last write, except for declared status bits that read set
//! (a request the controller completes at once) or clear (a busy bit the
//! controller clears at once). No frame timing is attached to the status bits.
//!
//! An output with a declared [`Scanout`] captures its OSD layer group once per
//! declared period, as the panel reads the buffers every frame. The X-T20
//! 2.11 commit routine 0xca5a0c copies its shadow block to the group: +0x200
//! enables the four layers by bit, then four words each from shadow +0x1c4,
//! +0x1c8 and +0x1cc to the origins at +0x210, the sizes at +0x220 and the
//! buffers at +0x230. Sizes, like the output size at +0x014, hold
//! `(width - 1) << 16 | (height - 1)`. The firmware writes no pitch; the
//! camera's OSD buffers (0x354d000, 0x384d000) are 720 x 480 premultiplied
//! RGBA words with a 2880-byte pitch, so rows are packed. Layers compose in
//! index order over black. The video layer (+0x000 bits, +0x010 format,
//! +0x030 buffer) carries the imaging pipeline's output, which is not
//! modeled, and is not captured.
//!
//! [`DisplayControl`] holds the display unit control pages, from GFX 50R 0xcf6adc.
//! Each bank retains every aligned word. A +0 read returns only bit 16, a
//! status: a written 1-to-0 change of bit 16 sets it after `ticks`, and a
//! write with bit 16 set clears it. A camera read of +0 while the display ran
//! returned 0, so +0 reads no stored bits. The delay is declared, not measured.
use fujimod_emulator_devices::graphics::scanout::Frame;
use fujimod_emulator_devices::{Cx, Device};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub struct FrameClock {
    irqs: Vec<u32>,
    period: u64,
    next: u64,
}
impl FrameClock {
    pub fn new(irqs: Vec<u32>, period: u64) -> Result<Self, String> {
        let mut unique = irqs.clone();
        unique.sort_unstable();
        unique.dedup();
        if irqs.is_empty()
            || unique.len() != irqs.len()
            || irqs.iter().any(|irq| !(32..512).contains(irq))
            || period == 0
        {
            return Err("Invalid X-Processor Pro display frame interrupts".into());
        }
        Ok(Self {
            irqs,
            period,
            next: period,
        })
    }
}
impl Device for FrameClock {
    fn name(&self) -> &'static str {
        "xpp-display-frames"
    }
    fn read(&mut self, cx: &mut Cx<'_>, _address: u64, _size: u8) -> u64 {
        cx.unmodeled();
        0
    }
    fn write(&mut self, cx: &mut Cx<'_>, _address: u64, _size: u8, _value: u64) {
        cx.unmodeled();
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        let now = cx.now();
        if now < self.next {
            return;
        }
        for &irq in &self.irqs {
            cx.set_irq(irq, true);
            cx.set_irq(irq, false);
        }
        let frames = (now - self.next) / self.period + 1;
        match frames
            .checked_mul(self.period)
            .and_then(|step| self.next.checked_add(step))
        {
            Some(next) => self.next = next,
            None => cx.fail("Display frame deadline overflow"),
        }
    }
    fn next_deadline(&self, _now: u64) -> Option<u64> {
        Some(self.next)
    }
    fn save(&self) -> Value {
        json!({ "next": self.next })
    }
    fn restore(&mut self, value: &Value) -> Result<(), String> {
        let next = value["next"]
            .as_u64()
            .filter(|next| next % self.period == 0 && *next > 0)
            .ok_or("Invalid display frame checkpoint")?;
        self.next = next;
        Ok(())
    }
}

const OUTPUT_SIZE: u64 = 0x014;
const LAYER_ENABLE: u64 = 0x200;
const LAYER_ORIGIN: u64 = 0x210;
const LAYER_SIZE: u64 = 0x220;
const LAYER_BUFFER: u64 = 0x230;

/// Host output index and capture period of a scanned-out display output.
#[derive(Clone, Copy, Debug)]
pub struct Scanout {
    pub output: usize,
    pub period: u64,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Capture {
    next: u64,
    sequence: u64,
    hash: Option<u64>,
}

pub struct Output {
    base: u64,
    size: u64,
    set: BTreeMap<u64, u32>,
    clear: BTreeMap<u64, u32>,
    words: BTreeMap<u64, u32>,
    scanout: Option<Scanout>,
    capture: Capture,
}
impl Output {
    /// `set` and `clear` map word offsets to the status bits that read as
    /// set or clear regardless of the last write.
    pub fn new(
        base: u64,
        size: u64,
        set: &[(u64, u32)],
        clear: &[(u64, u32)],
        scanout: Option<Scanout>,
    ) -> Result<Self, String> {
        let mut laws = [BTreeMap::new(), BTreeMap::new()];
        for (law, bits) in laws.iter_mut().zip([set, clear]) {
            for &(offset, mask) in bits {
                if offset & 3 != 0 || offset >= size || mask == 0 {
                    return Err("Invalid display output status bits".into());
                }
                *law.entry(offset).or_insert(0) |= mask;
            }
        }
        let [set, clear] = laws;
        if base & 0xfff != 0
            || size == 0
            || size & 0xfff != 0
            || set
                .iter()
                .any(|(offset, mask)| clear.get(offset).is_some_and(|c| c & mask != 0))
        {
            return Err("Invalid display output window".into());
        }
        if scanout.is_some_and(|s| s.period == 0 || size <= LAYER_BUFFER + 12) {
            return Err("Invalid display output scanout".into());
        }
        Ok(Self {
            base,
            size,
            set,
            clear,
            words: BTreeMap::new(),
            scanout,
            capture: Capture {
                next: scanout.map_or(0, |s| s.period),
                ..Capture::default()
            },
        })
    }
    fn word(&self, offset: u64) -> u32 {
        self.words.get(&offset).copied().unwrap_or(0)
    }
    /// `(width, height)` of a size word, or `None` for unsupported bits.
    fn extent(word: u32) -> Option<(u32, u32)> {
        (word & 0xf000_f000 == 0).then_some(((word >> 16) + 1, (word & 0xfff) + 1))
    }
    /// Composite the enabled OSD layers; `Ok(None)` when none is enabled.
    fn compose(&self, cx: &mut Cx<'_>) -> Result<Option<(u32, u32, Vec<u8>, u64)>, ()> {
        let enable = self.word(LAYER_ENABLE);
        if enable & !0xf != 0 {
            return Err(());
        }
        if enable == 0 {
            return Ok(None);
        }
        let (width, height) = Self::extent(self.word(OUTPUT_SIZE)).ok_or(())?;
        let mut rgba = vec![0; width as usize * height as usize * 4];
        for pixel in rgba.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        let mut top = 0;
        for layer in (0..4).filter(|n| enable & 1 << n != 0) {
            let at = |offset: u64| offset + 4 * layer as u64;
            let (w, h) = Self::extent(self.word(at(LAYER_SIZE))).ok_or(())?;
            let buffer = u64::from(self.word(at(LAYER_BUFFER)));
            if self.word(at(LAYER_ORIGIN)) != 0 || w > width || h > height || buffer & 3 != 0 {
                return Err(());
            }
            let pixels = cx
                .ram_bytes(buffer, w as usize * h as usize * 4)
                .map_err(|_| ())?;
            for (y, row) in pixels.chunks_exact(w as usize * 4).enumerate() {
                let line = &mut rgba[y * width as usize * 4..][..w as usize * 4];
                for (out, src) in line.chunks_exact_mut(4).zip(row.chunks_exact(4)) {
                    let keep = 255 - u32::from(src[3]);
                    for n in 0..3 {
                        let value = u32::from(src[n]) + (u32::from(out[n]) * keep + 127) / 255;
                        out[n] = value.min(255) as u8;
                    }
                }
            }
            top = buffer;
        }
        Ok(Some((width, height, rgba, top)))
    }
    fn scan(&mut self, cx: &mut Cx<'_>, scanout: Scanout) {
        let (width, height, rgba, source) = match self.compose(cx) {
            Ok(Some(frame)) => frame,
            Ok(None) => {
                let _ =
                    cx.output_status(scanout.output, json!({ "active": false, "tick": cx.now() }));
                return;
            }
            Err(()) => {
                cx.unmodeled();
                return;
            }
        };
        // FNV-1a: a stable digest for the image-change sequence.
        let hash = [width, height]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .chain(rgba.iter().copied())
            .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
            });
        if self.capture.hash != Some(hash) {
            self.capture.sequence += 1;
            self.capture.hash = Some(hash);
        }
        let sequence = self.capture.sequence;
        let status = json!({
            "active": true, "format": "rgba8888-premultiplied", "layers": self.word(LAYER_ENABLE),
            "source": format!("0x{source:x}"), "sequence": sequence, "width": width,
            "height": height, "tick": cx.now(), "capture": "osd-layers-over-black",
        });
        let delivered = cx.output_status(scanout.output, status).and_then(|()| {
            cx.scanout(Frame {
                output: scanout.output,
                sequence,
                width,
                height,
                rgba,
            })
        });
        if let Err(error) = delivered {
            cx.fail(error);
        }
    }
    fn offset(&self, cx: &mut Cx<'_>, address: u64, size: u8) -> Option<u64> {
        let offset = address.wrapping_sub(self.base);
        if size != 4 || offset & 3 != 0 || offset >= self.size {
            cx.unmodeled();
            return None;
        }
        Some(offset)
    }
}
impl Device for Output {
    fn name(&self) -> &'static str {
        "xpp-display-output"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        let Some(offset) = self.offset(cx, address, size) else {
            return 0;
        };
        let word = self.words.get(&offset).copied().unwrap_or(0);
        let set = self.set.get(&offset).copied().unwrap_or(0);
        let clear = self.clear.get(&offset).copied().unwrap_or(0);
        u64::from((word | set) & !clear)
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        let Some(offset) = self.offset(cx, address, size) else {
            return;
        };
        match u32::try_from(value) {
            Ok(0) => {
                self.words.remove(&offset);
            }
            Ok(value) => {
                self.words.insert(offset, value);
            }
            Err(_) => cx.unmodeled(),
        }
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        let Some(scanout) = self.scanout else {
            return;
        };
        let now = cx.now();
        if now < self.capture.next {
            return;
        }
        self.scan(cx, scanout);
        let frames = (now - self.capture.next) / scanout.period + 1;
        match frames
            .checked_mul(scanout.period)
            .and_then(|step| self.capture.next.checked_add(step))
        {
            Some(next) => self.capture.next = next,
            None => cx.fail("Display scanout deadline overflow"),
        }
    }
    fn next_deadline(&self, _now: u64) -> Option<u64> {
        self.scanout.map(|_| self.capture.next)
    }
    fn save(&self) -> Value {
        let words = self
            .words
            .iter()
            .map(|(o, v)| [*o, u64::from(*v)])
            .collect::<Vec<_>>();
        match self.scanout {
            Some(_) => json!({ "words": words, "scanout": self.capture }),
            None => json!({ "words": words }),
        }
    }
    fn restore(&mut self, value: &Value) -> Result<(), String> {
        let rows: Vec<(u64, u32)> = serde_json::from_value(value["words"].clone())
            .map_err(|e| format!("Invalid display output checkpoint: {e}"))?;
        if rows
            .iter()
            .any(|(offset, v)| offset & 3 != 0 || *offset >= self.size || *v == 0)
        {
            return Err("Invalid display output checkpoint".into());
        }
        let capture = match self.scanout {
            Some(scanout) => {
                let capture: Capture = serde_json::from_value(value["scanout"].clone())
                    .map_err(|e| format!("Invalid display output checkpoint: {e}"))?;
                if capture.next == 0 || capture.next % scanout.period != 0 {
                    return Err("Invalid display output checkpoint".into());
                }
                capture
            }
            None => Capture::default(),
        };
        self.words = rows.into_iter().collect();
        self.capture = capture;
        Ok(())
    }
}

const STATUS: u32 = 1 << 16;

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
struct Bank {
    words: BTreeMap<u64, u32>,
    status: bool,
    due: Option<u64>,
}

pub struct DisplayControl {
    bases: [u64; 2],
    ticks: u64,
    banks: [Bank; 2],
}

impl DisplayControl {
    pub fn new(bases: [u64; 2], ticks: u64) -> Result<Self, String> {
        if bases.iter().any(|b| b & 0xfff != 0 || b.checked_add(0x1000).is_none())
            || bases[0] == bases[1]
            || ticks == 0
        {
            return Err("Invalid X-Processor Pro display control wiring".into());
        }
        Ok(Self { bases, ticks, banks: Default::default() })
    }
    pub fn windows(&self) -> [(u64, u64); 2] {
        self.bases.map(|b| (b, b + 0x1000))
    }
    fn locate(&self, address: u64) -> (usize, u64) {
        let bank = if (self.bases[0]..self.bases[0] + 0x1000).contains(&address) { 0 } else { 1 };
        (bank, address - self.bases[bank])
    }
}

impl Device for DisplayControl {
    fn name(&self) -> &'static str {
        "xpp-display-control"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        let (bank, offset) = self.locate(address);
        if size != 4 || offset & 3 != 0 {
            cx.unmodeled();
            return 0;
        }
        let bank = &self.banks[bank];
        if offset == 0 {
            return if bank.status { STATUS.into() } else { 0 };
        }
        bank.words.get(&offset).copied().unwrap_or(0).into()
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        let (index, offset) = self.locate(address);
        if size != 4 || offset & 3 != 0 {
            cx.unmodeled();
            return;
        }
        let value = value as u32;
        let now = cx.now();
        let bank = &mut self.banks[index];
        if offset == 0 {
            let before = bank.words.get(&0).copied().unwrap_or(0);
            if value & STATUS != 0 {
                bank.status = false;
                bank.due = None;
            } else if before & STATUS != 0 {
                let Some(due) = now.checked_add(self.ticks) else {
                    cx.fail("X-Processor Pro display control deadline overflow");
                    return;
                };
                bank.due = Some(due);
            }
        }
        bank.words.insert(offset, value);
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        for bank in &mut self.banks {
            if bank.due.is_some_and(|due| due <= cx.now()) {
                bank.due = None;
                bank.status = true;
            }
        }
    }
    fn next_deadline(&self, _: u64) -> Option<u64> {
        self.banks.iter().filter_map(|b| b.due).min()
    }
    fn save(&self) -> Value {
        serde_json::to_value(&self.banks).unwrap()
    }
    fn restore(&mut self, value: &Value) -> Result<(), String> {
        let banks: [Bank; 2] = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        if banks.iter().any(|b| b.words.keys().any(|o| o & 3 != 0 || *o >= 0x1000)) {
            return Err("Invalid X-Processor Pro display control checkpoint".into());
        }
        self.banks = banks;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fujimod_emulator_devices::{Host, Record};

    #[derive(Default)]
    struct Clock {
        now: u64,
        edges: Vec<(u64, u32)>,
    }
    impl Host for Clock {
        fn now(&self) -> u64 {
            self.now
        }
        fn schedule(&mut self, _: usize, _: u64, _: u64) {
            unreachable!()
        }
        fn set_irq(&mut self, irq: u32, level: bool) {
            if level {
                self.edges.push((self.now, irq));
            }
        }
        fn raise_irq(&mut self, _: usize, _: u32) -> Result<(), String> {
            unreachable!()
        }
        fn ram_read(&self, _: u64, _: &mut [u8]) -> Result<(), String> {
            unreachable!()
        }
        fn ram_write(&mut self, _: u64, _: &[u8]) -> Result<(), String> {
            unreachable!()
        }
        fn record(&mut self, _: Record) {}
    }
    fn advance(clock: &mut FrameClock, host: &mut Clock, now: u64) {
        host.now = now;
        clock.advance(&mut Cx::new(host, 0, 0, 0));
    }

    #[test]
    fn each_period_raises_one_edge_per_line_and_skips_missed_frames() {
        assert!(FrameClock::new(vec![], 10).is_err());
        assert!(FrameClock::new(vec![112, 112], 10).is_err());
        assert!(FrameClock::new(vec![112], 0).is_err());
        let mut clock = FrameClock::new(vec![112, 117], 10).unwrap();
        assert_eq!(clock.next_deadline(0), Some(10));
        let mut host = Clock::default();
        advance(&mut clock, &mut host, 9);
        assert!(host.edges.is_empty());
        advance(&mut clock, &mut host, 35);
        assert_eq!(host.edges, vec![(35, 112), (35, 117)]);
        assert_eq!(clock.next_deadline(35), Some(40));
        let saved = clock.save();
        advance(&mut clock, &mut host, 40);
        assert_eq!(clock.next_deadline(40), Some(50));
        clock.restore(&saved).unwrap();
        assert_eq!(clock.next_deadline(40), Some(40));
        assert!(clock.restore(&json!({ "next": 41 })).is_err());
    }

    #[derive(Default)]
    struct Screen {
        now: u64,
        ram: Vec<u8>,
        frames: Vec<Frame>,
        active: Vec<bool>,
    }
    impl Host for Screen {
        fn now(&self) -> u64 {
            self.now
        }
        fn schedule(&mut self, _: usize, _: u64, _: u64) {
            unreachable!()
        }
        fn set_irq(&mut self, _: u32, _: bool) {}
        fn raise_irq(&mut self, _: usize, _: u32) -> Result<(), String> {
            unreachable!()
        }
        fn ram_read(&self, address: u64, out: &mut [u8]) -> Result<(), String> {
            let start = address as usize;
            out.copy_from_slice(
                self.ram
                    .get(start..start + out.len())
                    .ok_or("outside RAM")?,
            );
            Ok(())
        }
        fn ram_write(&mut self, _: u64, _: &[u8]) -> Result<(), String> {
            unreachable!()
        }
        fn scanout(&mut self, frame: Frame) -> Result<(), String> {
            self.frames.push(frame);
            Ok(())
        }
        fn output_status(&mut self, _: usize, state: Value) -> Result<(), String> {
            self.active.push(state["active"] == true);
            Ok(())
        }
        fn record(&mut self, _: Record) {}
    }

    #[test]
    fn scanout_composes_premultiplied_osd_layers_over_black() {
        const BASE: u64 = 0xfff64000;
        let scanout = Some(Scanout {
            output: 0,
            period: 10,
        });
        assert!(
            Output::new(
                BASE,
                0x1000,
                &[],
                &[],
                Some(Scanout {
                    output: 0,
                    period: 0
                })
            )
            .is_err()
        );
        let mut out = Output::new(BASE, 0x3000, &[], &[], scanout).unwrap();
        assert_eq!(out.next_deadline(0), Some(10));
        let mut host = Screen {
            ram: vec![0; 0x100],
            ..Screen::default()
        };
        // Two 2 x 1 layers: opaque red then half-covering premultiplied white.
        host.ram[0x40..0x48].copy_from_slice(&[255, 0, 0, 255, 0, 0, 0, 0]);
        host.ram[0x80..0x88].copy_from_slice(&[128, 128, 128, 128, 0, 0, 0, 0]);
        let write = |out: &mut Output, host: &mut Screen, offset: u64, value: u32| {
            out.write(
                &mut Cx::new(host, 0, 0, 0),
                BASE + offset,
                4,
                u64::from(value),
            );
        };
        host.now = 10;
        out.advance(&mut Cx::new(&mut host, 0, 0, 0));
        assert_eq!(
            (host.frames.len(), host.active.as_slice()),
            (0, &[false][..])
        );
        write(&mut out, &mut host, OUTPUT_SIZE, 0x1_0000);
        write(&mut out, &mut host, LAYER_ENABLE, 3);
        for (layer, buffer) in [(0, 0x40), (4, 0x80)] {
            write(&mut out, &mut host, LAYER_SIZE + layer, 0x1_0000);
            write(&mut out, &mut host, LAYER_BUFFER + layer, buffer);
        }
        host.now = 25;
        out.advance(&mut Cx::new(&mut host, 0, 0, 0));
        assert_eq!(out.next_deadline(25), Some(30));
        let frame = host.frames.last().unwrap();
        assert_eq!((frame.width, frame.height, frame.sequence), (2, 1, 1));
        assert_eq!(frame.rgba, [255, 128, 128, 255, 0, 0, 0, 255]);
        let saved = out.save();
        host.now = 30;
        out.advance(&mut Cx::new(&mut host, 0, 0, 0));
        assert_eq!(host.frames.last().unwrap().sequence, 1);
        out.restore(&saved).unwrap();
        assert_eq!(out.next_deadline(30), Some(30));
        // A layer origin away from the corner is not modeled.
        write(&mut out, &mut host, LAYER_ORIGIN, 1);
        let mut cx = Cx::new(&mut host, 0, 0, 0);
        out.advance(&mut cx);
        assert!(cx.is_unmodeled());
    }

    #[test]
    fn outputs_read_back_words_with_declared_status_bits() {
        use fujimod_emulator_devices::test_host::{TestHost, read, write};
        const BASE: u64 = 0xfff64000;
        assert!(Output::new(BASE, 0x3000, &[(0, 0x10000)], &[(0, 0x10000)], None).is_err());
        assert!(Output::new(BASE, 0x3000, &[(0x3000, 1)], &[], None).is_err());
        let mut out =
            Output::new(BASE, 0x3000, &[(0, 0x10000)], &[(0x2000, 1 << 28)], None).unwrap();
        let mut h = TestHost::new();
        assert_eq!(read(&mut out, &mut h, BASE), 0x10000);
        write(&mut out, &mut h, BASE, 0x10001);
        write(&mut out, &mut h, BASE, 1);
        assert_eq!(read(&mut out, &mut h, BASE), 0x10001);
        write(&mut out, &mut h, BASE + 0x2000, 0x1002_5000);
        assert_eq!(read(&mut out, &mut h, BASE + 0x2000), 0x25000);
        write(&mut out, &mut h, BASE + 0x1004, 0x1c1044b);
        let saved = out.save();
        write(&mut out, &mut h, BASE + 0x1004, 0);
        assert_eq!(read(&mut out, &mut h, BASE + 0x1004), 0);
        out.restore(&saved).unwrap();
        assert_eq!(read(&mut out, &mut h, BASE + 0x1004), 0x1c1044b);
        let mut cx = h.cx();
        out.read(&mut cx, BASE + 2, 2);
        assert!(cx.is_unmodeled());
    }

    #[test]
    fn clearing_bit16_reports_status_after_delay_until_set_again() {
        use fujimod_emulator_devices::test_host::{TestHost, advance, read, write};
        let mut h = TestHost::new();
        let mut d = DisplayControl::new([0xfff59000, 0xfff64000], 8).unwrap();
        write(&mut d, &mut h, 0xfff64000, 0x10000);
        write(&mut d, &mut h, 0xfff64000, 0);
        assert_eq!(read(&mut d, &mut h, 0xfff64000), 0);
        advance(&mut d, &mut h, 8);
        assert_eq!(read(&mut d, &mut h, 0xfff64000), 0x10000);
        assert_eq!(read(&mut d, &mut h, 0xfff59000), 0);
        write(&mut d, &mut h, 0xfff64010, 0x33);
        assert_eq!(read(&mut d, &mut h, 0xfff64010), 0x33);
        write(&mut d, &mut h, 0xfff64000, 0x10020);
        assert_eq!(read(&mut d, &mut h, 0xfff64000), 0);
    }
}

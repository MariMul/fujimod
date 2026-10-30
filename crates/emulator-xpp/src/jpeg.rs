//! X-Processor Pro JPEG encoder.
//!
//! X-T20 2.11 at 0xfffc8000, interrupt 134 (section 6 driver
//! 0x1557390..0x15575ac, section 5 0xc6dbfc..0xc6e568). A still encodes three
//! pictures (markers 0x06028b00/0x06028a00): the 160x120 EXIF thumbnail, a
//! 320x240 preview and the main picture from the second DRAM bank. The
//! firmware writes the quantization tables word by word to +0x8c (32 words:
//! luma then chroma, big-endian bytes in zigzag order), the restart interval
//! in MCUs to +0x80, height << 16 | width to +0x44, the packed 4:2:2 source
//! (Y0 Y1 Cb Cr) to +0x100, the destination to +0x10c and its size limit to
//! +0x110, and starts a run with bit 0 of +0x00 (bit 8 resets; both read
//! clear). +0x24 bit 16 reads set (ready); a finished run sets bit 4, which
//! writing 1 clears, and the level interrupt is high while it is set. The
//! handler 0x1557390 reads the status and the stream size at +0x94. The
//! stream is SOI, DHT, DQT, DRI, SOF0 (4:2:2), SOS, scan and EOI, as in the
//! camera's files; the firmware puts its EXIF/APP2 segments in front of the
//! main picture and checks that the file ends with EOI (0x16390cc). Other
//! words read back their last write.
use fujimod_emulator_devices::{Cx, Device};
use fujimod_emulator_image_codecs::jpeg::Codec;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Register window size in bytes.
pub const WINDOW: u64 = 0x1000;
const CONTROL: u64 = 0x00;
const START: u32 = 1;
const RESET: u32 = 1 << 8;
const STATUS: u64 = 0x24;
const READY: u32 = 1 << 16;
const DONE: u32 = 1 << 4;
const SIZE: u64 = 0x44;
const RESTART: u64 = 0x80;
const TABLES: u64 = 0x8c;
const STREAM: u64 = 0x94;
const SOURCE: u64 = 0x100;
const DESTINATION: u64 = 0x10c;
const LIMIT: u64 = 0x110;
const TABLE_WORDS: usize = 32;
const MAX_STREAM: usize = 64 * 1024 * 1024;

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    words: BTreeMap<u64, u32>,
    fifo: Vec<u32>,
    tables: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    due: Option<u64>,
    status: u32,
    stream: u32,
    runs: u64,
}
pub struct Encoder {
    base: u64,
    irq: u32,
    ticks: u64,
    state: State,
}
impl Encoder {
    pub fn new(base: u64, irq: u32, ticks: u64) -> Result<Self, String> {
        if base & 0xfff != 0 || !(32..512).contains(&irq) || ticks == 0 {
            return Err("Invalid X-Processor Pro JPEG encoder".into());
        }
        Ok(Self {
            base,
            irq,
            ticks,
            state: State::default(),
        })
    }
    fn word(&self, offset: u64) -> u32 {
        self.state.words.get(&offset).copied().unwrap_or(0)
    }
    fn encode(&mut self, cx: &mut Cx<'_>) -> Result<(), String> {
        let mut codec = Codec::default();
        codec.tables(&self.state.tables)?;
        let size = self.word(SIZE);
        let (width, height) = ((size & 0xffff) as usize, (size >> 16) as usize);
        let restart = self.word(RESTART) as u16;
        let limit = match self.word(LIMIT) as usize {
            0 => MAX_STREAM,
            n => n.min(MAX_STREAM),
        };
        let source = cx.ram_bytes(self.word(SOURCE).into(), width * height * 2)?;
        let mut body = Vec::new();
        codec.encode(&source, width, height, restart, &mut body, limit - 2)?;
        let mut stream = vec![0xff, 0xd8];
        stream.extend_from_slice(&body);
        cx.ram_write(self.word(DESTINATION).into(), &stream)?;
        self.state.stream = stream.len() as u32;
        Ok(())
    }
}
impl Device for Encoder {
    fn name(&self) -> &'static str {
        "xpp-jpeg-encoder"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        let offset = address.wrapping_sub(self.base);
        if size != 4 || offset & 3 != 0 || offset >= WINDOW {
            cx.unmodeled();
            return 0;
        }
        u64::from(match offset {
            STATUS => self.state.status | READY,
            STREAM => self.state.stream,
            CONTROL => self.word(CONTROL) & !(START | RESET),
            o => self.word(o),
        })
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        let offset = address.wrapping_sub(self.base);
        let Ok(value) = u32::try_from(value) else {
            return cx.unmodeled();
        };
        if size != 4 || offset & 3 != 0 || offset >= WINDOW {
            return cx.unmodeled();
        }
        match offset {
            STATUS => {
                self.state.status &= !value;
                if self.state.status & DONE == 0 {
                    cx.set_irq(self.irq, false);
                }
                return;
            }
            TABLES => {
                self.state.fifo.push(value);
                if self.state.fifo.len() > TABLE_WORDS {
                    self.state.fifo.remove(0);
                }
                return;
            }
            CONTROL if value & RESET != 0 => {
                self.state.fifo.clear();
            }
            CONTROL if value & START != 0 && self.state.due.is_none() => {
                if self.state.fifo.len() == TABLE_WORDS {
                    self.state.tables = self
                        .state
                        .fifo
                        .iter()
                        .flat_map(|w| w.to_be_bytes())
                        .collect();
                }
                self.state.fifo.clear();
                self.state.due = cx.now().checked_add(self.ticks);
                if self.state.due.is_none() {
                    return cx.fail("JPEG encoder deadline overflow");
                }
            }
            _ => {}
        }
        if value == 0 {
            self.state.words.remove(&offset);
        } else {
            self.state.words.insert(offset, value);
        }
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        if !self.state.due.is_some_and(|at| at <= cx.now()) {
            return;
        }
        self.state.due = None;
        if let Err(e) = self.encode(cx) {
            return cx.fail(format!("JPEG encoder run failed: {e}"));
        }
        self.state.runs += 1;
        self.state.status |= DONE;
        cx.set_irq(self.irq, true);
    }
    fn next_deadline(&self, _now: u64) -> Option<u64> {
        self.state.due
    }
    fn save(&self) -> Value {
        serde_json::to_value(&self.state).unwrap_or(Value::Null)
    }
    fn restore(&mut self, value: &Value) -> Result<(), String> {
        let state: State = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        if state
            .words
            .iter()
            .any(|(o, v)| o & 3 != 0 || *o >= WINDOW || *v == 0)
            || state.fifo.len() > TABLE_WORDS
            || !(state.tables.is_empty() || state.tables.len() == 4 * TABLE_WORDS)
            || state.status & !DONE != 0
        {
            return Err("Invalid JPEG encoder checkpoint".into());
        }
        self.state = state;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fujimod_emulator_devices::Host;
    use fujimod_emulator_devices::test_host::{TestHost, advance, read, write};

    const BASE: u64 = 0xfffc8000;

    #[test]
    fn a_run_encodes_the_source_with_the_uploaded_tables() {
        let mut encoder = Encoder::new(BASE, 134, 10).unwrap();
        let mut h = TestHost::new();
        assert_eq!(read(&mut encoder, &mut h, BASE + STATUS), u64::from(READY));
        write(&mut encoder, &mut h, BASE, RESET);
        assert_eq!(read(&mut encoder, &mut h, BASE), 0);
        for _ in 0..TABLE_WORDS {
            write(&mut encoder, &mut h, BASE + TABLES, 0x0303_0303);
        }
        h.ram_write(0x1000, &[128; 32 * 16 * 2]).unwrap();
        write(&mut encoder, &mut h, BASE + SIZE, 16 << 16 | 32);
        write(&mut encoder, &mut h, BASE + SOURCE, 0x1000);
        write(&mut encoder, &mut h, BASE + DESTINATION, 0x4000);
        write(&mut encoder, &mut h, BASE + LIMIT, 0x2000);
        write(&mut encoder, &mut h, BASE, START);
        assert_eq!(read(&mut encoder, &mut h, BASE), 0);
        assert_eq!(encoder.next_deadline(0), Some(10));
        advance(&mut encoder, &mut h, 10);
        assert_eq!(
            read(&mut encoder, &mut h, BASE + STATUS),
            u64::from(READY | DONE)
        );
        let size = read(&mut encoder, &mut h, BASE + STREAM) as usize;
        let mut stream = vec![0; size];
        h.ram_read(0x4000, &mut stream).unwrap();
        assert_eq!(&stream[..4], &[0xff, 0xd8, 0xff, 0xc4]);
        assert_eq!(&stream[size - 2..], &[0xff, 0xd9]);
        let saved = encoder.save();
        write(&mut encoder, &mut h, BASE + STATUS, DONE);
        assert_eq!(read(&mut encoder, &mut h, BASE + STATUS), u64::from(READY));
        let mut other = Encoder::new(BASE, 134, 10).unwrap();
        other.restore(&saved).unwrap();
        assert_eq!(read(&mut other, &mut h, BASE + STREAM) as usize, size);
    }

    #[test]
    fn a_run_without_tables_fails() {
        let mut encoder = Encoder::new(BASE, 134, 10).unwrap();
        let mut h = TestHost::new();
        write(&mut encoder, &mut h, BASE + SIZE, 16 << 16 | 32);
        write(&mut encoder, &mut h, BASE, START);
        h.now = 10;
        let mut cx = h.cx();
        encoder.advance(&mut cx);
        assert!(cx.refusal().is_some());
    }
}

//! FFF60000 H.264 sequence registers. The modeled configuration uses the
//! FFF604/60B/60C/60D/60E fields: C00 bit11 initializes, bits14/15 start,
//! and C24 bit9 reports a sequence-start notice on IRQ75 after two declared
//! virtual ticks. Frame compression, bitstream DMA and frame completion
//! remain unmodeled.
use super::{Irq, due, word};
use crate::{Cx, Device};
use serde::Deserialize;

#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {}

pub struct VideoEncoder {
    r: Vec<u32>,
    irq: Irq,
    pending: Option<u64>,
    started: bool,
}
impl VideoEncoder {
    pub fn new(_: Options, irq: Irq) -> Result<Self, String> {
        Ok(Self {
            r: vec![0; 0xf00 / 4],
            irq,
            pending: None,
            started: false,
        })
    }
    fn valid(o: usize) -> bool {
        matches!(o,0x400..=0x408|0x440|0x444|0x490..=0x4a4|0x4ac..=0x4b8|0x4c4|0x4c8|0x4fc|0xb00|0xb04|0xb08|0xb6c|0xb74..=0xb80|0xbd0..=0xbe0|0xc00|0xc20|0xc24|0xc80..=0xc9c|0xca8|0xcbc|0xd00..=0xd28|0xd44..=0xd7c|0xd88|0xd8c|0xdc8|0xdcc|0xde0|0xe00..=0xe24|0xe30..=0xe50|0xe58..=0xef0)
    }
}
impl Device for VideoEncoder {
    crate::checkpoint_fields!(r: Vec<u32>, pending: Option<u64>, started: bool);
    fn name(&self) -> &'static str {
        "exr2-isp-video-encoder"
    }
    fn read(&mut self, cx: &mut Cx<'_>, a: u64, s: u8) -> u64 {
        let Some(o) = word(cx, a, s, 0xfff60000, 0xf00) else {
            return 0;
        };
        if !Self::valid(o) {
            cx.unmodeled();
            return 0;
        }
        self.r[o / 4] as u64
    }
    fn write(&mut self, cx: &mut Cx<'_>, a: u64, s: u8, v: u64) {
        let Some(o) = word(cx, a, s, 0xfff60000, 0xf00) else {
            return;
        };
        if !Self::valid(o) || v > u32::MAX as u64 {
            cx.unmodeled();
            return;
        }
        if o == 0xc24 {
            self.r[o / 4] &= !(v as u32);
            return;
        }
        if o == 0xc00 {
            match v {
                0 | 0x800 => self.r[o / 4] = 0,
                0x4000 | 0x8000 => {
                    if self.started || self.pending.is_some() {
                        cx.fail("H.264 frame encoding is unmodeled");
                        return;
                    }
                    match due(cx.now(), 2) {
                        Ok(d) => self.pending = Some(d),
                        Err(e) => cx.fail(e),
                    }
                }
                _ => cx.fail("H.264 compression command is unmodeled"),
            }
        } else {
            self.r[o / 4] = v as u32;
        }
    }
    fn next_deadline(&self, _: u64) -> Option<u64> {
        self.pending
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        if self.pending.is_some_and(|d| cx.now() >= d) {
            self.r[0xc20 / 4] = 4;
            self.r[0xc24 / 4] |= 1 << 9;
            self.pending = None;
            self.started = true;
            if let Err(e) = self.irq.raise(cx) {
                cx.fail(e);
            }
        }
    }
    fn event(&mut self, cx: &mut Cx<'_>, _: u64) {
        self.advance(cx)
    }
    fn save(&self) -> serde_json::Value {
        serde_json::json!((&self.r, &self.pending, self.started))
    }
    fn restore(&mut self, v: &serde_json::Value) -> Result<(), String> {
        let (r, p, s): (Vec<u32>, Option<u64>, bool) =
            serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
        if r.len() != self.r.len() {
            return Err("video register bank size".into());
        }
        self.r = r;
        self.pending = p;
        self.started = s;
        Ok(())
    }
}

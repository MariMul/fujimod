//! FFF64000 addressed image transfer registers. Execution remains a strict
//! refusal until the programmed transfer layout is supported.
use fujimod_emulator_devices::{Cx, Device, exr2::irq_routes::Irq};
use fujimod_emulator_imaging::exr2_isp::{due, word};

pub struct ImageTransfer {
    r: [u32; 8],
    reset: Option<u64>,
    irq: Irq,
}
impl ImageTransfer {
    pub fn new(irq: Irq) -> Self {
        Self {
            r: [0x10000, 0, 0, 0, 0, 0, 0, 0],
            reset: None,
            irq,
        }
    }
}
impl Device for ImageTransfer {
    fujimod_emulator_devices::checkpoint_fields!(r: [u32; 8], reset: Option<u64>);
    fn fingerprint(&self) -> Result<serde_json::Value, String> {
        Ok(self.save())
    }
    fn name(&self) -> &'static str {
        "exr2-isp-image-transfer"
    }
    fn read(&mut self, cx: &mut Cx<'_>, a: u64, s: u8) -> u64 {
        self.advance(cx);
        let Some(o) = word(cx, a, s, 0xfff64000, 0x20) else {
            return 0;
        };
        self.r[o / 4] as u64
    }
    fn write(&mut self, cx: &mut Cx<'_>, a: u64, s: u8, v: u64) {
        self.advance(cx);
        let Some(o) = word(cx, a, s, 0xfff64000, 0x20) else {
            return;
        };
        if v > u32::MAX as u64 {
            cx.unmodeled();
            return;
        }
        let v = v as u32;
        if self.reset.is_some() {
            cx.fail("image transfer reconfiguration during reset");
            return;
        }
        let mask = match o {
            0 => 0x10131,
            4 | 8 => 0x3fffffff,
            0xc => 0x7fff,
            0x10 => 0x0fff0fff,
            0x18 | 0x1c => 0x31,
            _ => 0,
        };
        if (o == 0x14) || v & !mask != 0 {
            cx.fail("unknown image transfer register bits");
            return;
        }
        if o == 0 {
            if v == 0 {
                match due(cx.now(), 2) {
                    Ok(t) => {
                        self.reset = Some(t);
                        self.r[0] = 0;
                        self.r[5] = 0;
                    }
                    Err(e) => cx.fail(e),
                }
                return;
            }
            if v & 1 != 0 {
                // Preserve the actual submitted fields in diagnostics; no DMA,
                // completion or IRQ is published for an unsupported operation.
                self.r[0] = v;
                cx.fail("image transfer execution layout is unsupported");
                return;
            }
            if v & 0x10100 != 0x10000 {
                cx.fail("unknown image transfer control command");
                return;
            }
        }
        if o == 0x18 {
            self.r[5] &= !v;
            if let Err(e) =
                cx.set_core_irq(self.irq.core, self.irq.line, self.r[5] & self.r[7] != 0)
            {
                cx.fail(e)
            }
        } else {
            self.r[o / 4] = v;
        }
    }
    fn next_deadline(&self, _: u64) -> Option<u64> {
        self.reset
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        if self.reset.is_some_and(|t| cx.now() >= t) {
            self.reset = None;
            self.r[0] = 0x10000;
        }
    }
    fn event(&mut self, cx: &mut Cx<'_>, _: u64) {
        self.advance(cx)
    }
    fn save(&self) -> serde_json::Value {
        serde_json::json!({"registers":self.r,"reset":self.reset})
    }
}

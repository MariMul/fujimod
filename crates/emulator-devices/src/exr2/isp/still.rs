//! FFF40000 still processor and its addressed calibration banks. Register
//! geometry drives RAW reads and IRQ61 output. Nearest six-by-six CFA color
//! uses programmed black, WB/exposure, radial shading and the measured film
//! law. Spatial filtering remains unmodeled.
use super::dma::{Raster, Transfer};
use super::{Irq, word};
use crate::imaging::{
    film::{Film, still_offset},
    raw::{Cfa, Packing, RawView},
};
use crate::{Cx, Device};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub completion_ticks: u64,
    pub cfa: Vec<u8>,
}
pub struct StillProcessor {
    r: Vec<u32>,
    cfa: Cfa,
    film: Film,
    source: Arc<[u8]>,
    dma: Transfer,
    delay: u64,
    irq: Irq,
}
impl StillProcessor {
    pub fn new(options: Options, irq: Irq) -> Result<Self, String> {
        if options.completion_ticks == 0 {
            return Err("still processor delay must be positive".into());
        }
        let mut r = vec![0; 0x16100 / 4];
        r[0x10000 / 4] = 0x10001;
        Ok(Self {
            r,
            cfa: Cfa::new(options.cfa)?,
            film: Film::default(),
            source: Arc::from([]),
            dma: Transfer::default(),
            delay: options.completion_ticks,
            irq,
        })
    }
    fn valid(o: usize) -> bool {
        matches!(o,0|0xb4..=0xf8|0x1400..=0x4bfc|0x8000..=0x805c|0x10000..=0x100fc|0x11000..=0x11ffc|0x14000..=0x160fc)
    }
    fn start(&mut self, cx: &Cx<'_>) -> Result<(), String> {
        let r = |o: usize| self.r[(0x10000 + o) / 4];
        let packing = if r(8) & 0x100 != 0 {
            Packing::Packed12
        } else {
            Packing::U16
        };
        if r(0x44) > 16384 || r(0x48) > 16384 {
            return Err("still processor row pitch is unsupported".into());
        }
        let src = Raster {
            address: r(0x50) as u64,
            width: (r(0x64) >> 16) as usize + 1,
            height: (r(0x64) & 65535) as usize + 1,
            pitch: packing.row_bytes(r(0x44) as usize),
        };
        let dst = Raster {
            address: r(0x58) as u64,
            width: (r(0x38) >> 16) as usize,
            height: (r(0x38) & 65535) as usize,
            pitch: 2 * r(0x48) as usize,
        };
        src.validate(packing.row_bytes(src.width))?;
        dst.validate(dst.width * 2)?;
        if !src.width.is_multiple_of(2)
            || !dst.width.is_multiple_of(2)
            || src.pitch > 32768
            || dst.pitch > 32768
            || src.overlaps(dst)?
        {
            return Err("still processor RAW/output geometry overlaps or has an odd width".into());
        }
        let mut black = None;
        for o in (0x8004..0x8014).step_by(4) {
            for v in [self.r[o / 4] & 8191, self.r[o / 4] >> 16 & 8191] {
                if black.is_some_and(|b| b != v) {
                    return Err("unequal CFA black slots are unmodeled".into());
                }
                black = Some(v);
            }
        }
        let gains = [
            (self.r[0x8054 / 4] >> 16 & 32767) as u16,
            (self.r[0x8054 / 4] & 32767) as u16,
            (self.r[0x8058 / 4] >> 16 & 32767) as u16,
        ];
        let exposure = (self.r[0x8058 / 4] & 4095) as u16;
        let shading = super::shading::Shading::from_registers(|o| self.r[(0x8000 + o) / 4])?;
        self.film
            .update(|o| self.r[(0x14000 + still_offset(o).unwrap()) / 4])?;
        // The still filter consumes its surrounding RAW context. For
        // 4944x3296 -> 4896x3264, output (0,0) maps to RAW (24,16). These are
        // cropped pixel windows, rather than full-field resampling coordinates.
        let crop = match (
            src.width.checked_sub(dst.width),
            src.height.checked_sub(dst.height),
        ) {
            (Some(x), Some(y)) if x % 2 == 0 && y % 2 == 0 => [x / 2, y / 2, dst.width, dst.height],
            _ => return Err("still output must fit a centered RAW filter window".into()),
        };
        self.source = vec![0; src.pitch * src.height].into();
        cx.ram_read(src.address, Arc::get_mut(&mut self.source).unwrap())?;
        self.dma.prepare(cx.now(), self.delay, dst, dst.width * 2)?;
        let result = self.cfa.render(
            &RawView {
                bytes: &self.source,
                width: src.width,
                height: src.height,
                pitch: src.pitch,
                packing,
            },
            &self.film,
            black.unwrap() as u16,
            gains,
            exposure,
            |x, y| shading.as_ref().map_or(2048, |s| s.gain(x, y)),
            crop,
            dst.width,
            dst.height,
            &mut self.dma.pixels,
        );
        if result.is_err() {
            self.dma.cancel();
        }
        result
    }
}
impl Device for StillProcessor {
    crate::checkpoint_fields!(
        r: Vec<u32>,
        cfa: Cfa,
        film: Film,
        dma: Transfer,
        ; blobs: [source]
    );
    fn fingerprint(&self) -> std::result::Result<serde_json::Value, String> {
        Ok(
            serde_json::json!({"r":self.r,"cfa":self.cfa,"film":self.film,
            "source":crate::fingerprint::bytes(&self.source),"dma":self.dma.fingerprint()}),
        )
    }

    fn name(&self) -> &'static str {
        "exr2-isp-still-processor"
    }
    fn read(&mut self, cx: &mut Cx<'_>, a: u64, s: u8) -> u64 {
        let Some(o) = word(cx, a, s, 0xfff30000, 0x16100) else {
            return 0;
        };
        if !Self::valid(o) {
            cx.unmodeled();
            return 0;
        }
        self.r[o / 4] as u64
    }
    fn write(&mut self, cx: &mut Cx<'_>, a: u64, s: u8, v: u64) {
        let Some(o) = word(cx, a, s, 0xfff30000, 0x16100) else {
            return;
        };
        if !Self::valid(o) || v > u32::MAX as u64 {
            cx.unmodeled();
            return;
        }
        if o == 0x10084 {
            self.r[o / 4] &= !(v as u32);
            return;
        }
        if self.dma.deadline().is_some() {
            cx.fail("still processor reconfiguration during DMA");
            return;
        }
        if o == 0x10000 {
            self.r[o / 4] = v as u32 | 0x10001;
            return;
        }
        if o == 0x10004 {
            if v != 1 {
                cx.fail("unknown still processor START");
                return;
            }
            if let Err(e) = self.start(cx) {
                cx.fail(e);
                return;
            }
        }
        self.r[o / 4] = v as u32;
    }
    fn next_deadline(&self, _: u64) -> Option<u64> {
        self.dma.deadline()
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        match self.dma.write(cx) {
            Ok(true) => {
                self.r[0x10084 / 4] |= 1;
                Transfer::interrupt(cx, self.irq)
            }
            Err(e) => cx.fail(e),
            _ => {}
        }
    }
    fn event(&mut self, cx: &mut Cx<'_>, _: u64) {
        self.advance(cx)
    }
    fn save(&self) -> serde_json::Value {
        serde_json::json!((&self.r, &self.dma))
    }
    fn restore(&mut self, v: &serde_json::Value) -> Result<(), String> {
        let (r, d): (Vec<u32>, Transfer) =
            serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
        if r.len() != self.r.len() {
            return Err("still register bank size".into());
        }
        self.r = r;
        self.dma = d;
        self.film = Film::default();
        Ok(())
    }
}

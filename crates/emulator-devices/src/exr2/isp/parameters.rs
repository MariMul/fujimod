//! FFF19000 parameter retention. The bank supplies uploaded pixel parameters
//! through 96 canonical word masks. Implemented silicon bits remain unmeasured.
use super::word;
use crate::imaging::film::Film;
use crate::{Cx, Device};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {}
#[derive(Serialize, Deserialize)]
pub struct Parameters {
    pub words: Vec<u32>,
    pub film: Film,
}
impl Default for Parameters {
    fn default() -> Self {
        Self {
            words: vec![0; 0x1000 / 4],
            film: Film::default(),
        }
    }
}
pub type ParameterHandle = Arc<Mutex<Parameters>>;
pub struct ParameterBank {
    pub parameters: ParameterHandle,
}
impl ParameterBank {
    pub fn new(_: Options, parameters: ParameterHandle) -> Result<Self, String> {
        Ok(Self { parameters })
    }
}
pub fn mask(o: usize) -> Option<u32> {
    Some(match o {
        0 => 0x0111113f,
        4 => 0xd1511110,
        8 => 0x3131,
        0x100 => 0x10001,
        0x104 => 0xff007f,
        0x108 => 0xffff1fff,
        0x10c => 0x3fffffff,
        0x110 => 0x01ff01ff,
        0x114 => 0xff01ff,
        0x118 => 0x3fff3fff,
        0x11c..=0x128 => 0xffff3fff,
        0x12c => 0x7fff,
        0x200 => 0x7fff7fff,
        0x204 => 0x7fff,
        0x208 => 0xf0f0f,
        0x20c => u32::MAX,
        0x210 => 0xffff0707,
        0x300 => 0x3f3f3f3f,
        0x304..=0x32c => 0x1fff1fff,
        0x400 | 0x408 | 0x410 => 0x1fff1fff,
        0x404 | 0x40c | 0x414 => 0x1fff,
        0x500 => 0x77777777,
        0x504 => 7,
        0x508..=0x540 => 0x3fff3fff,
        0x600 => 0xf0f0f,
        0x604 | 0x608 => 0x1fff1fff,
        0x60c => u32::MAX,
        0x610..=0x620 => 0x03ff00ff,
        0x624 | 0x630 => 0x03ff03ff,
        0x628 | 0x634 => 0x7ff,
        0x62c | 0x638 => 0x1fff1fff,
        0x63c => 0x1f001f,
        0x640 => u32::MAX,
        0x644..=0x654 => 0x03ff07ff,
        0x700 => 0x3f3f3f3f,
        0x800..=0x814 => 0x03ff03ff,
        0x818 | 0x900 | 0x908 => 0xffffff,
        0x81c => 0x3f3f3f3f,
        0x820 | 0x828 => 0xffff,
        0x824 => u32::MAX,
        0x82c => 0xff,
        0x830 => 0x1f1f1f1f,
        0x834 => 0x1f1f,
        0x838 => 0x3f00ff,
        0x904 => 0x0f0f00ff,
        _ => return None,
    })
}
fn valid(o: usize, v: u32) -> bool {
    if mask(o).is_none_or(|m| v & !m != 0) {
        return false;
    }
    match o {
        0 => v & 63 <= 0x2d,
        8 => v >> 12 & 3 <= 2,
        0x104 => v & 127 <= 0x51,
        0x110 => v & 511 <= 256 && v >> 16 & 511 <= 256,
        0x114 => v & 511 <= 256,
        0x300 => (0..3).all(|i| v >> (i * 8) & 63 <= 32),
        0x900 => v & 255 <= 128 && v >> 8 & 255 <= 128,
        _ => true,
    }
}
impl Device for ParameterBank {
    fn checkpoint(&self) -> Result<crate::checkpoint::Checkpoint, String> {
        crate::checkpoint::Checkpoint::encode(
            self.name(),
            &*self
                .parameters
                .lock()
                .map_err(|_| "parameter state poisoned")?,
        )
    }
    fn validate_checkpoint(&self, saved: &crate::checkpoint::Checkpoint) -> Result<(), String> {
        let _: Parameters = saved.decode(self.name())?;
        Ok(())
    }
    fn restore_checkpoint(&mut self, saved: &crate::checkpoint::Checkpoint) -> Result<(), String> {
        *self
            .parameters
            .lock()
            .map_err(|_| "parameter state poisoned")? = saved.decode(self.name())?;
        Ok(())
    }

    fn fingerprint(&self) -> std::result::Result<serde_json::Value, String> {
        Ok(self.save())
    }

    fn name(&self) -> &'static str {
        "exr2-isp-parameters"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        let Some(o) = word(cx, address, size, 0xfff19000, 0x1000) else {
            return 0;
        };
        if mask(o).is_none() {
            cx.unmodeled();
            return 0;
        }
        self.parameters.lock().unwrap().words[o / 4] as u64
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        let Some(o) = word(cx, address, size, 0xfff19000, 0x1000) else {
            return;
        };
        if value > u32::MAX as u64 || !valid(o, value as u32) {
            cx.fail("image parameter word exceeds its mask/domain");
            return;
        }
        self.parameters.lock().unwrap().words[o / 4] = value as u32;
    }
    fn save(&self) -> serde_json::Value {
        serde_json::to_value(&*self.parameters.lock().unwrap()).unwrap()
    }
    fn restore(&mut self, v: &serde_json::Value) -> Result<(), String> {
        let p: Parameters = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
        if p.words.len() != 1024
            || p.words.iter().enumerate().any(|(i, &v)| {
                if mask(i * 4).is_some() {
                    !valid(i * 4, v)
                } else {
                    v != 0
                }
            })
        {
            return Err("invalid parameter bank state".into());
        }
        *self.parameters.lock().unwrap() = p;
        Ok(())
    }
}

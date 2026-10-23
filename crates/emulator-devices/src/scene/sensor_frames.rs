//! Immutable host RGB/RAW frames. Updating host input never changes a latched
//! exposure. Only the sensor's physical frame edge advances frame identity.
use super::FrameEdge;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
#[derive(Serialize, Deserialize)]
pub enum FramePixels {
    Rgb24(Arc<[u8]>),
    Raw16(Vec<u16>),
}
#[derive(Serialize, Deserialize)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub pixels: FramePixels,
    /// Scene radiance of an sRGB white pixel, in cd/m².
    #[serde(default = "daylight_white")]
    pub white_luminance: f64,
}
fn daylight_white() -> f64 {
    10000.
}
impl Frame {
    pub fn fingerprint(&self) -> serde_json::Value {
        let pixels = match &self.pixels {
            FramePixels::Rgb24(bytes) => {
                serde_json::json!({"kind":"rgb24","contents":crate::fingerprint::bytes(bytes)})
            }
            FramePixels::Raw16(samples) => {
                serde_json::json!({"kind":"raw16","contents":crate::fingerprint::words16(samples)})
            }
        };
        serde_json::json!({"width":self.width,"height":self.height,"white_luminance":self.white_luminance,"pixels":pixels})
    }
    pub fn checkpoint(&self) -> Result<crate::checkpoint::Checkpoint, String> {
        let raw = matches!(self.pixels, FramePixels::Raw16(_));
        let pixels = match &self.pixels {
            FramePixels::Rgb24(bytes) => bytes.clone(),
            FramePixels::Raw16(words) => words
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>()
                .into(),
        };
        Ok(crate::checkpoint::Checkpoint::encode(
            "sensor-frame",
            &(self.width, self.height, self.white_luminance, raw),
        )?
        .with_blobs(vec![pixels]))
    }
    pub fn from_checkpoint(saved: &crate::checkpoint::Checkpoint) -> Result<Self, String> {
        let (width, height, white, raw): (usize, usize, f64, bool) =
            saved.decode("sensor-frame")?;
        if width == 0
            || height == 0
            || width > 8192
            || height > 8192
            || width * height > 32_000_000
            || !white.is_finite()
            || white <= 0.
            || saved.blobs().len() != 1
            || !saved.children().is_empty()
            || saved.blobs()[0].len() != width * height * if raw { 2 } else { 3 }
        {
            return Err("sensor frame checkpoint raster or radiance differs".into());
        }
        let pixels = if raw {
            FramePixels::Raw16(
                saved.blobs()[0]
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect(),
            )
        } else {
            FramePixels::Rgb24(saved.blobs()[0].clone())
        };
        let mut frame = Self::new(width, height, pixels)?;
        frame.white_luminance = white;
        Ok(frame)
    }
    pub fn new(width: usize, height: usize, pixels: FramePixels) -> Result<Self, String> {
        if width == 0 || height == 0 || width > 8192 || height > 8192 || width * height > 32_000_000
        {
            return Err("sensor input dimensions are unbounded".into());
        }
        let len = match &pixels {
            FramePixels::Rgb24(p) => p.len() / 3,
            FramePixels::Raw16(p) => p.len(),
        };
        if len != width * height || matches!(&pixels,FramePixels::Rgb24(p) if p.len()%3!=0) {
            return Err("sensor input has an invalid raster length".into());
        }
        Ok(Self {
            width,
            height,
            pixels,
            white_luminance: daylight_white(),
        })
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct SensorFrames {
    pub current: Arc<Frame>,
    pub latched: Arc<Frame>,
    pub edge: Option<FrameEdge>,
}
pub type SensorFramesHandle = Arc<Mutex<SensorFrames>>;
impl SensorFrames {
    pub fn fingerprint(&self) -> serde_json::Value {
        serde_json::json!({"current":self.current.fingerprint(),"latched":self.latched.fingerprint(),"edge":self.edge})
    }
    pub fn new(frame: Frame) -> Self {
        let frame = Arc::new(frame);
        Self {
            current: frame.clone(),
            latched: frame,
            edge: None,
        }
    }
    pub fn set_frame(&mut self, frame: Frame) {
        self.current = Arc::new(frame);
    }
    pub fn latch(&mut self, tick: u64) -> Result<FrameEdge, String> {
        if self.edge.is_some_and(|s| tick < s.tick) {
            return Err("sensor frame clock moved backward".into());
        }
        let edge = FrameEdge {
            index: self
                .edge
                .map_or(Some(1), |s| s.index.checked_add(1))
                .ok_or("sensor frame index overflow")?,
            tick,
        };
        self.latched = self.current.clone();
        self.edge = Some(edge);
        Ok(edge)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_edge_freezes_external_input() {
        let mut s = SensorFrames::new(Frame::new(2, 1, FramePixels::Raw16(vec![1, 2])).unwrap());
        assert_eq!(s.latch(5).unwrap(), FrameEdge { index: 1, tick: 5 });
        s.set_frame(Frame::new(2, 1, FramePixels::Raw16(vec![3, 4])).unwrap());
        assert!(matches!(&s.latched.pixels,FramePixels::Raw16(p) if p==&[1,2]));
        assert_eq!(s.edge.unwrap().index, 1);
        s.latch(6).unwrap();
        assert!(matches!(&s.latched.pixels,FramePixels::Raw16(p) if p==&[3,4]));
    }
}

#[cfg(test)]
mod fingerprint_tests {
    use super::*;
    #[test]
    fn sensor_identity_hashes_complete_pixels_without_expanding_rasters() {
        let mut source = SensorFrames::new(
            Frame::new(
                1536,
                1024,
                FramePixels::Rgb24(vec![90; 1536 * 1024 * 3].into()),
            )
            .unwrap(),
        );
        source.latch(10).unwrap();
        let saved = source.clone();
        let before = source.fingerprint();
        assert!(serde_json::to_vec(&before).unwrap().len() < 1024);
        source.set_frame(Frame::new(1, 1, FramePixels::Rgb24(vec![4, 5, 6].into())).unwrap());
        assert_ne!(source.fingerprint(), before);
        assert_eq!(saved.fingerprint(), before);
        assert!(Arc::ptr_eq(&saved.latched, &source.latched));
        source.latch(20).unwrap();
        assert!(!Arc::ptr_eq(&saved.latched, &source.latched));
        source = saved;
        assert_eq!(source.fingerprint(), before);
    }
    #[test]
    fn raw_fingerprint_uses_every_word_in_little_endian_order() {
        let words = [0x1234, 0xff00, 0, 0xabcd];
        let identity = crate::fingerprint::words16(&words);
        let bytes: Vec<_> = words.iter().flat_map(|v| v.to_le_bytes()).collect();
        assert_eq!(
            identity["sha256"],
            crate::fingerprint::bytes(&bytes)["sha256"]
        );
        assert_ne!(
            identity,
            crate::fingerprint::words16(&[0x1234, 0xff00, 0, 0xabce])
        );
    }
}

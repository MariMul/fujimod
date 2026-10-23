//! Immutable host RGB/RAW/mosaic frames. Updating host input never changes a
//! latched exposure. Only the sensor's physical frame edge advances frame identity.
use super::FrameEdge;
use super::mosaic::{Cfa, MosaicSites};
use super::raf::RafScene;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, OnceLock};
#[derive(Serialize, Deserialize)]
pub enum FramePixels {
    Rgb24(Arc<[u8]>),
    Raw16(Vec<u16>),
    /// Per-site linear radiance of a CFA mosaic in a RAF's normalization:
    /// sensor counts = radiance × seconds / f_number² × analog_gain.
    Mosaic {
        cfa: Cfa,
        radiance: Arc<[f32]>,
    },
}
#[derive(Serialize, Deserialize)]
enum PixelKind {
    Rgb24,
    Raw16,
    Mosaic(Cfa),
}
#[derive(Serialize, Deserialize)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub pixels: FramePixels,
    /// Scene radiance of an sRGB white pixel, in cd/m².
    #[serde(default = "daylight_white")]
    pub white_luminance: f64,
    /// Mosaic cell means, derived once from the immutable pixels.
    #[serde(skip)]
    sites: OnceLock<MosaicSites>,
}
fn daylight_white() -> f64 {
    10000.
}
impl Frame {
    /// Whether two frames hold the same immutable pixels. Optical scenes and
    /// their caches depend only on these, so frames sharing a pixel buffer
    /// share projections.
    pub fn same_pixels(&self, other: &Frame) -> bool {
        self.width == other.width
            && self.height == other.height
            && self.white_luminance.to_bits() == other.white_luminance.to_bits()
            && match (&self.pixels, &other.pixels) {
                (FramePixels::Rgb24(a), FramePixels::Rgb24(b)) => Arc::ptr_eq(a, b),
                (
                    FramePixels::Mosaic {
                        cfa: a,
                        radiance: x,
                    },
                    FramePixels::Mosaic {
                        cfa: b,
                        radiance: y,
                    },
                ) => a == b && Arc::ptr_eq(x, y),
                _ => false,
            }
    }
    /// Per-site mosaic radiance for mosaic frames; `None` for RGB and RAW.
    pub(crate) fn mosaic(&self) -> Option<&MosaicSites> {
        match &self.pixels {
            FramePixels::Mosaic { cfa, radiance } => {
                Some(self.sites.get_or_init(|| {
                    MosaicSites::new(*cfa, self.width, self.height, radiance.clone())
                }))
            }
            FramePixels::Rgb24(_) | FramePixels::Raw16(_) => None,
        }
    }
    pub fn fingerprint(&self) -> serde_json::Value {
        let pixels = match &self.pixels {
            FramePixels::Rgb24(bytes) => {
                serde_json::json!({"kind":"rgb24","contents":crate::fingerprint::bytes(bytes)})
            }
            FramePixels::Raw16(samples) => {
                serde_json::json!({"kind":"raw16","contents":crate::fingerprint::words16(samples)})
            }
            FramePixels::Mosaic { cfa, radiance } => {
                serde_json::json!({"kind":"mosaic","cfa":cfa,"contents":crate::fingerprint::float32s(radiance)})
            }
        };
        serde_json::json!({"width":self.width,"height":self.height,"white_luminance":self.white_luminance,"pixels":pixels})
    }
    pub fn checkpoint(&self) -> Result<crate::checkpoint::Checkpoint, String> {
        let (kind, pixels): (PixelKind, Arc<[u8]>) = match &self.pixels {
            FramePixels::Rgb24(bytes) => (PixelKind::Rgb24, bytes.clone()),
            FramePixels::Raw16(words) => (
                PixelKind::Raw16,
                words
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect::<Vec<_>>()
                    .into(),
            ),
            FramePixels::Mosaic { cfa, radiance } => (
                PixelKind::Mosaic(*cfa),
                radiance
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect::<Vec<_>>()
                    .into(),
            ),
        };
        Ok(crate::checkpoint::Checkpoint::encode(
            "sensor-frame",
            &(self.width, self.height, self.white_luminance, kind),
        )?
        .with_blobs(vec![pixels]))
    }
    pub fn from_checkpoint(saved: &crate::checkpoint::Checkpoint) -> Result<Self, String> {
        let (width, height, white, kind): (usize, usize, f64, PixelKind) =
            saved.decode("sensor-frame")?;
        let bytes_per_site = match kind {
            PixelKind::Rgb24 => 3,
            PixelKind::Raw16 => 2,
            PixelKind::Mosaic(_) => 4,
        };
        if width == 0
            || height == 0
            || width > 8192
            || height > 8192
            || width * height > 32_000_000
            || !white.is_finite()
            || white <= 0.
            || saved.blobs().len() != 1
            || !saved.children().is_empty()
            || saved.blobs()[0].len() != width * height * bytes_per_site
        {
            return Err("sensor frame checkpoint raster or radiance differs".into());
        }
        let pixels = match kind {
            PixelKind::Rgb24 => FramePixels::Rgb24(saved.blobs()[0].clone()),
            PixelKind::Raw16 => FramePixels::Raw16(
                saved.blobs()[0]
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect(),
            ),
            PixelKind::Mosaic(cfa) => FramePixels::Mosaic {
                cfa,
                radiance: saved.blobs()[0]
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect::<Vec<_>>()
                    .into(),
            },
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
            FramePixels::Mosaic { cfa, radiance } => {
                if cfa.iter().flatten().any(|c| *c > 2) {
                    return Err("mosaic CFA colors must be 0, 1 or 2".into());
                }
                if radiance.iter().any(|v| !v.is_finite() || *v < 0.) {
                    return Err("mosaic radiance must be finite and nonnegative".into());
                }
                radiance.len()
            }
        };
        if len != width * height || matches!(&pixels,FramePixels::Rgb24(p) if p.len()%3!=0) {
            return Err("sensor input has an invalid raster length".into());
        }
        Ok(Self {
            width,
            height,
            pixels,
            white_luminance: daylight_white(),
            sites: OnceLock::new(),
        })
    }
    /// The RAF's visible crop as a mosaic frame. The CFA pattern is re-phased
    /// to the crop origin, so sites keep their physical colors.
    pub fn from_raf(scene: &RafScene) -> Result<Self, String> {
        let (full_width, full_height) = (scene.width as usize, scene.height as usize);
        let (left, top) = (scene.crop_left as usize, scene.crop_top as usize);
        let (width, height) = (scene.crop_width as usize, scene.crop_height as usize);
        if scene.radiance.len() != full_width * full_height
            || width == 0
            || height == 0
            || left + width > full_width
            || top + height > full_height
        {
            return Err("RAF crop lies outside its radiance raster".into());
        }
        let cfa = std::array::from_fn(|y| {
            std::array::from_fn(|x| scene.cfa[(y + top) % 6][(x + left) % 6])
        });
        let radiance: Arc<[f32]> = (top..top + height)
            .flat_map(|y| {
                let row = y * full_width + left;
                scene.radiance[row..row + width].iter().copied()
            })
            .collect::<Vec<_>>()
            .into();
        Self::new(width, height, FramePixels::Mosaic { cfa, radiance })
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
    #[test]
    fn mosaic_frames_checkpoint_and_fingerprint_every_site_and_phase() {
        let cfa = [
            [0, 2, 1, 2, 0, 1],
            [1, 1, 0, 1, 1, 2],
            [1, 1, 2, 1, 1, 0],
            [2, 0, 1, 0, 2, 1],
            [1, 1, 2, 1, 1, 0],
            [1, 1, 0, 1, 1, 2],
        ];
        let radiance: Arc<[f32]> = (0..21).map(|i| i as f32 * 0.5).collect::<Vec<_>>().into();
        let frame = Frame::new(
            7,
            3,
            FramePixels::Mosaic {
                cfa,
                radiance: radiance.clone(),
            },
        )
        .unwrap();
        let saved = frame.checkpoint().unwrap();
        let restored = Frame::from_checkpoint(&saved).unwrap();
        assert_eq!(restored.fingerprint(), frame.fingerprint());
        let mut shifted = cfa;
        shifted[0].swap(0, 1);
        let other = Frame::new(
            7,
            3,
            FramePixels::Mosaic {
                cfa: shifted,
                radiance,
            },
        )
        .unwrap();
        assert_ne!(other.fingerprint(), frame.fingerprint());
        let corrupt = saved.clone().with_blobs(vec![Arc::from(vec![0u8; 3])]);
        assert!(Frame::from_checkpoint(&corrupt).is_err());
        assert!(
            Frame::new(
                1,
                1,
                FramePixels::Mosaic {
                    cfa: [[3; 6]; 6],
                    radiance: vec![1.].into()
                }
            )
            .is_err()
        );
        assert!(
            Frame::new(
                1,
                1,
                FramePixels::Mosaic {
                    cfa: [[0; 6]; 6],
                    radiance: vec![f32::NAN].into()
                }
            )
            .is_err()
        );
        assert!(
            Frame::new(
                1,
                1,
                FramePixels::Mosaic {
                    cfa: [[0; 6]; 6],
                    radiance: vec![-1.].into()
                }
            )
            .is_err()
        );
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

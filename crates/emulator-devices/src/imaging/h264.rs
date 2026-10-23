//! Pure software H.264 compression. No guest memory, time or device effects.
//! OpenH264 is BSD-2-Clause: https://github.com/ralfbiedert/openh264-rs.
//! The 0.9.3 wrapper retains Rust 1.88 compatibility; newer wrappers require
//! Rust 1.89 through `wide`. The locked source backend is built by Cargo.
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub width: usize,
    pub height: usize,
    pub frames_per_second: u32,
    pub bitrate: u32,
    pub max_qp: u8,
    pub gop: usize,
}
impl Config {
    pub fn validate(self) -> Result<(), String> {
        if !(16..=1920).contains(&self.width)
            || !(16..=1088).contains(&self.height)
            || self.width % 16 != 0
            || self.height % 16 != 0
            || !(1..=60).contains(&self.frames_per_second)
            || !(10_000..=100_000_000).contains(&self.bitrate)
            || !(1..=51).contains(&self.max_qp)
            || !(1..=120).contains(&self.gop)
        {
            return Err(
                "H.264 requires bounded progressive macroblock-aligned 8-bit 4:2:0 configuration"
                    .into(),
            );
        }
        Ok(())
    }
    pub fn frame_bytes(self) -> usize {
        self.width * self.height * 3 / 2
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct Codec(openh264::encoder::Encoder);
#[cfg(target_arch = "wasm32")]
pub struct Codec;

impl Codec {
    pub fn new(config: Config, history: &[Arc<[u8]>]) -> Result<Self, String> {
        config.validate()?;
        #[cfg(target_arch = "wasm32")]
        {
            let _ = history;
            Err("software H.264 compression is unavailable on this target".into())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            use openh264::encoder::{
                BitRate, Complexity, Encoder, EncoderConfig, FrameRate, Profile, QpRange,
                RateControlMode,
            };
            let settings = EncoderConfig::new()
                .bitrate(BitRate::from_bps(config.bitrate))
                .max_frame_rate(FrameRate::from_hz(config.frames_per_second as f32))
                .rate_control_mode(RateControlMode::Bitrate)
                .profile(Profile::Baseline)
                .complexity(Complexity::Low)
                .qp(QpRange::new(1, config.max_qp))
                .skip_frames(false)
                .num_threads(1)
                .scene_change_detect(false)
                .adaptive_quantization(false)
                .background_detection(false);
            let mut codec = Self(
                Encoder::with_api_config(openh264::OpenH264API::from_source(), settings)
                    .map_err(|e| format!("H.264 encoder initialization: {e}"))?,
            );
            for frame in history {
                codec.encode(config, frame)?;
            }
            Ok(codec)
        }
    }
    pub fn encode(&mut self, config: Config, frame: &[u8]) -> Result<Vec<u8>, String> {
        if frame.len() != config.frame_bytes() {
            return Err("H.264 planar input size differs".into());
        }
        #[cfg(target_arch = "wasm32")]
        {
            Err("software H.264 compression is unavailable on this target".into())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let y = config.width * config.height;
            let input = openh264::formats::YUVSlices::new(
                (&frame[..y], &frame[y..y + y / 4], &frame[y + y / 4..]),
                (config.width, config.height),
                (config.width, config.width / 2, config.width / 2),
            );
            let stream = self
                .0
                .encode(&input)
                .map_err(|e| format!("H.264 encode: {e}"))?;
            let mut out = Vec::new();
            // Normalize start codes without changing any encoded NAL payload.
            // Only the supported single-layer AVC parameter sets and slices
            // may enter the DMA ring (no SVC, SEI, AUD or filler packets).
            for i in 0..stream.num_layers() {
                let layer = stream.layer(i).ok_or("H.264 missing layer")?;
                for j in 0..layer.nal_count() {
                    let nal = layer.nal_unit(j).ok_or("H.264 missing NAL")?;
                    let n = if nal.starts_with(&[0, 0, 0, 1]) {
                        4
                    } else if nal.starts_with(&[0, 0, 1]) {
                        3
                    } else {
                        return Err("H.264 backend omitted Annex B prefix".into());
                    };
                    let payload = nal
                        .get(n..)
                        .filter(|v| !v.is_empty())
                        .ok_or("empty H.264 NAL")?;
                    match payload[0] & 31 {
                        7 if payload.len() <= 64 => (),
                        8 if payload.len() <= 16 => (),
                        1 | 5 => (),
                        _ => return Err(
                            "H.264 backend produced unsupported NAL type or parameter-set length"
                                .into(),
                        ),
                    }
                    out.extend_from_slice(&[0, 0, 0, 1]);
                    out.extend_from_slice(payload);
                }
            }
            if out.is_empty() {
                return Err("H.264 backend skipped a frame".into());
            }
            Ok(out)
        }
    }
}

/// Packed Y0,Y1,Cb,Cr 4:2:2 to planar 4:2:0, averaging adjacent chroma rows.
pub fn planar_420(width: usize, height: usize, packed: &[u8]) -> Result<Vec<u8>, String> {
    if width == 0
        || height == 0
        || width % 2 != 0
        || height % 2 != 0
        || width > 1920
        || height > 1088
        || packed.len() != width * height * 2
    {
        return Err("H.264 packed input geometry differs".into());
    }
    let y = width * height;
    let mut out = vec![0; y * 3 / 2];
    for row in 0..height {
        for col in 0..width / 2 {
            let p = (row * width / 2 + col) * 4;
            out[row * width + col * 2..row * width + col * 2 + 2]
                .copy_from_slice(&packed[p..p + 2]);
            if row % 2 == 0 {
                let q = p + width * 2;
                let c = row / 2 * (width / 2) + col;
                out[y + c] = ((packed[p + 2] as u16 + packed[q + 2] as u16 + 1) / 2) as u8;
                out[y + y / 4 + c] = ((packed[p + 3] as u16 + packed[q + 3] as u16 + 1) / 2) as u8;
            }
        }
    }
    Ok(out)
}

//! EXR II image processing blocks. Register names and masks follow the
//! corresponding register interfaces; pixel approximations and
//! timing are declared explicitly by each model, independently of firmware.
pub mod block11000;
pub mod contrast;
pub mod dma;
pub mod face_detector;
pub mod face_results;
pub mod metering;
pub mod parameters;
mod pixel_statistics;
pub mod raw;
pub mod video;
pub use parameters::ParameterBank;
pub use raw::{Prefilter, RawConversion, RawStatistics};
pub use video::VideoEncoder;

/// Physical interrupt output. The board supplies the receiving core; the
/// runtime routes the physical line through that core's interrupt controller.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Irq {
    pub core: usize,
    pub line: u32,
}
impl Irq {
    pub fn raise(self, cx: &mut crate::Cx<'_>) -> Result<(), String> {
        cx.raise_irq(self.core, self.line)
    }
}

pub(crate) fn word(
    cx: &mut crate::Cx<'_>,
    address: u64,
    size: u8,
    base: u64,
    end: usize,
) -> Option<usize> {
    let Some(offset) = address.checked_sub(base) else {
        cx.unmodeled();
        return None;
    };
    if size != 4 || offset % 4 != 0 || offset >= end as u64 {
        cx.unmodeled();
        None
    } else {
        Some(offset as usize)
    }
}

pub(crate) fn due(now: u64, delay: u64) -> Result<u64, String> {
    if delay == 0 {
        return Err("image engine delay must be positive".into());
    }
    now.checked_add(delay)
        .ok_or_else(|| "image engine clock overflow".into())
}

#[cfg(test)]
pub(crate) mod tests;

mod shading;
pub mod still;
pub use still::StillProcessor;
pub mod scaler;
pub mod stream;
pub mod warp;
pub use scaler::Scaler;
pub use warp::Warp;
pub mod input;
pub mod readback;
pub use input::ImageInput;

// Output banks share their DMA batch and source/IRQ dependencies. The board
// wires ImageFrames/StatisticsFrames, and publishes register-derived owners
// through dma::IrqRoutes; window and phase blocks use local START deadlines.
pub mod output;
pub mod phase_detection;
pub mod producer;
pub mod window_analysis;
pub use output::OutputChannel;
pub use phase_detection::PhaseDetection;
pub use producer::Block18000;
pub use window_analysis::WindowAnalysis;

pub use block11000::Block11000;
pub use face_detector::FaceDetector;

pub mod combination;

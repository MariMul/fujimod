//! EXR II image processing blocks. Register names and masks follow the
//! corresponding register interfaces; pixel approximations and
//! timing are declared explicitly by each model, independently of firmware.
pub mod block11000;
pub mod face_detector;
pub mod face_results;
pub mod metering;
mod pixel_statistics;
pub mod raw;
pub mod video;
pub use raw::{Prefilter, RawConversion, RawStatistics};
pub use video::VideoEncoder;

#[cfg(test)]
pub(crate) mod tests;

mod shading;
pub mod still;
pub use still::StillProcessor;
pub mod scaler;
pub mod warp;
pub use scaler::Scaler;
pub use warp::Warp;
pub mod input;
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

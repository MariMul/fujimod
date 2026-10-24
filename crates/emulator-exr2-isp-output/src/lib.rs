//! EXR II combination, output channels, still processor, scaler, warp and video encoder.
pub mod construct;
pub mod combination;
pub mod output;
pub mod scaler;
mod shading;
pub mod still;
pub mod video;
pub mod warp;
pub use video::VideoEncoder;
pub use still::StillProcessor;
pub use scaler::Scaler;
pub use warp::Warp;
pub use output::OutputChannel;

#[cfg(test)]
mod tests;

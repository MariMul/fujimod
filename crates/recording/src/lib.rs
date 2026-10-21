//! Observational frame capture, screenshots and optional timestamped video.
//! Sources supply RGB or a declared framebuffer format; recording has no device
//! access or guest clock and cannot drive execution or a camera transport.
mod badges;
pub mod cache;
pub mod frame;
mod timeline;
mod video;
pub use frame::{Frame, Result};
pub use timeline::{Metrics, Recorder, seconds};
pub const CONTROLS: &str = include_str!("../assets/controls.json");

/// Annotate a host copy; clean captures and their source pixels stay immutable.
pub fn annotate(frame: &Frame, labels: &[(String, u8)]) -> Result<Frame> {
    for (name, _) in labels {
        badges::check(name)?;
    }
    Frame::rgb(frame.width(), frame.height(), badges::draw(frame, labels))
}

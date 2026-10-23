//! Declared external inputs. These laws do not inspect firmware state.

mod cadence;
pub mod clocked;
pub mod frames;
pub(crate) mod mosaic;
pub mod optics;
pub mod raf;
mod recorded;

pub use cadence::{FrameCadence, FrameEdge, FramePhases, StatisticsCadence};
pub use frames::{ImageFrames, StatisticsFrames, StatisticsSpan};
pub use raf::{RafRadianceRgb, RafScene, RecordedExposure, raf_to_radiance_rgb, read_raf};
pub use recorded::{Recorded, RecordedSpan};

pub mod sensor_frames;
pub use sensor_frames::{Frame, FramePixels, SensorFrames, SensorFramesHandle};

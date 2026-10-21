//! Declared external inputs. These laws do not inspect firmware state.

mod cadence;
pub mod clocked;
pub(crate) mod frames;
pub mod optics;
mod recorded;

pub use cadence::{FrameCadence, FrameEdge, FramePhases, StatisticsCadence};
pub use frames::{ImageFrames, StatisticsFrames, StatisticsSpan};
pub use recorded::{Recorded, RecordedSpan};

pub mod sensor_frames;
pub use sensor_frames::{Frame, FramePixels, SensorFrames, SensorFramesHandle};

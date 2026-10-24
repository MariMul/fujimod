//! The declared thin-lens optics a projection follows.
use serde::{Deserialize, Serialize};
/// Relative field illumination is explicit optical fixture data. Cosine-fourth
/// is the ideal thin-lens approximation, not a measured camera lens profile.
#[derive(Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Illumination {
    Uniform,
    CosineFourth,
}
#[derive(Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub focus: String,
    pub focal_length_mm: f64,
    pub subject_distance_mm: f64,
    pub sensor_width_mm: f64,
    pub relative_illumination: Illumination,
}

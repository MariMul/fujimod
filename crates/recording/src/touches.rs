//! Touch indicators as the iOS Simulator shows them: a translucent circle at
//! the finger while it is down, following it, fading after it lifts.
use crate::{
    CONTROLS, Result,
    canvas::{self, Canvas},
};
use serde::Deserialize;
use std::{str::FromStr, sync::OnceLock};
use tiny_skia::PathBuilder;

#[derive(Deserialize)]
struct Style {
    radius: f64,
    fill: String,
    border: String,
    border_width: f32,
    release_ms: u32,
}
#[derive(Deserialize)]
struct Spec {
    touch: Style,
}
fn style() -> &'static Style {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    &SPEC
        .get_or_init(|| serde_json::from_str(CONTROLS).expect("checked touch presentation"))
        .touch
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Start,
    Move,
    End,
}
impl FromStr for TouchPhase {
    type Err = String;
    fn from_str(value: &str) -> Result<Self> {
        match value {
            "start" => Ok(Self::Start),
            "move" => Ok(Self::Move),
            "end" => Ok(Self::End),
            _ => Err(format!("Unknown touch phase: {value}")),
        }
    }
}

/// One finger in source-frame coordinates; `lifted` is the output frame of
/// its lift.
pub(crate) struct Touch {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) lifted: Option<u64>,
}
/// A circle to draw: canvas center and radius in pixels (unrounded, pixel
/// centers at +0.5) and opacity.
pub(crate) type Mark = (f64, f64, f64, u8);

pub(crate) fn fade_frames(fps: u32) -> u64 {
    (f64::from(fps) * f64::from(style().release_ms) / 1000.)
        .round()
        .max(1.) as u64
}
/// Marks for fingers on a source of `source` size fitted centered into
/// `canvas`, as `Frame::fit` places it. Lifted fingers past their fade are
/// dropped from `touches`.
pub(crate) fn marks(
    touches: &mut Vec<Touch>,
    frames: u64,
    fps: u32,
    source: (u32, u32),
    canvas: (u32, u32),
) -> Vec<Mark> {
    let fade = fade_frames(fps);
    touches.retain(|t| t.lifted.is_none_or(|at| frames - at < fade));
    let (sw, sh) = (f64::from(source.0), f64::from(source.1));
    let scale = (f64::from(canvas.0) / sw).min(f64::from(canvas.1) / sh);
    let (w, h) = ((sw * scale).round(), (sh * scale).round());
    let (left, top) = (
        ((f64::from(canvas.0) - w) / 2.).floor(),
        ((f64::from(canvas.1) - h) / 2.).floor(),
    );
    let radius = style().radius * scale;
    touches
        .iter()
        .map(|t| {
            let alpha = t.lifted.map_or(255, |at| {
                (255. * (1. - (frames - at) as f64 / fade as f64)).round() as u8
            });
            (left + t.x * w / sw, top + t.y * h / sh, radius, alpha)
        })
        .collect()
}
/// A filled disc inside a ring, as an SVG circle with a stroke along its
/// inside edge: both anti-aliased, at any center and radius.
pub(crate) fn draw_on(canvas: &mut Canvas, marks: &[Mark]) {
    let style = style();
    for &(cx, cy, radius, alpha) in marks {
        let opacity = f32::from(alpha) / 255.;
        let (cx, cy, radius) = (cx as f32, cy as f32, radius as f32);
        let ring = style.border_width.min(radius);
        if let Some(disc) = PathBuilder::from_circle(cx, cy, radius - ring) {
            canvas.fill(&disc, canvas::color(&style.fill, opacity));
        }
        if let Some(edge) = PathBuilder::from_circle(cx, cy, radius - ring / 2.) {
            canvas.stroke(&edge, canvas::color(&style.border, opacity), ring);
        }
    }
}

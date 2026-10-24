//! Touch indicators as the iOS Simulator shows them: a translucent circle at
//! the finger while it is down, following it, fading after it lifts.
use crate::{CONTROLS, Result, badges};
use serde::Deserialize;
use std::{str::FromStr, sync::OnceLock};

#[derive(Deserialize)]
struct Style {
    radius: i32,
    fill: String,
    border: String,
    border_width: i32,
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
/// A circle to draw: canvas center, radius and opacity.
pub(crate) type Mark = (i32, i32, i32, u8);

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
    let radius = (f64::from(style().radius) * scale).round().max(1.) as i32;
    touches
        .iter()
        .map(|t| {
            let alpha = t.lifted.map_or(255, |at| {
                (255. * (1. - (frames - at) as f64 / fade as f64)).round() as u8
            });
            (
                (left + t.x * w / sw).round() as i32,
                (top + t.y * h / sh).round() as i32,
                radius,
                alpha,
            )
        })
        .collect()
}
pub(crate) fn draw_on(canvas: &mut badges::Canvas, marks: &[Mark]) {
    let style = style();
    for &(cx, cy, radius, alpha) in marks {
        let fill = badges::color(&style.fill, alpha);
        let border = badges::color(&style.border, alpha);
        let inner = (radius - style.border_width).max(0);
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let distance = dx * dx + dy * dy;
                if distance > radius * radius {
                    continue;
                }
                let color = if distance > inner * inner {
                    border
                } else {
                    fill
                };
                canvas.pixel(cx + dx, cy + dy, color);
            }
        }
    }
}

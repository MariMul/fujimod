//! Badge rasterization uses the same control identities and geometry as clients.
use crate::{CONTROLS, Frame, Result};
use font8x8::UnicodeFonts;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Deserialize)]
struct Control {
    label: String,
    text: Option<String>,
    icon: Option<String>,
}
#[derive(Deserialize)]
struct Shape {
    #[serde(default)]
    lines: Vec<Vec<[i32; 2]>>,
    circle: Option<[i32; 3]>,
}
#[derive(Deserialize)]
struct Style {
    icon_size: i32,
    padding_x: i32,
    padding_y: i32,
    border_width: i32,
    gap: i32,
    bottom: i32,
    radius: i32,
    foreground: String,
    background: String,
    border: String,
    release_ms: u32,
}
#[derive(Deserialize)]
struct Spec {
    controls: BTreeMap<String, Control>,
    icons: BTreeMap<String, Shape>,
    badge: Style,
}
fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| serde_json::from_str(CONTROLS).expect("checked control presentation"))
}
pub fn check(name: &str) -> Result<()> {
    if spec().controls.contains_key(name) {
        Ok(())
    } else {
        Err(format!("Missing input presentation: {name}"))
    }
}
pub fn fade_frames(fps: u32) -> u64 {
    (f64::from(fps) * f64::from(spec().badge.release_ms) / 1000.)
        .round()
        .max(1.) as u64
}
pub(crate) fn color(value: &str, alpha: u8) -> [u8; 4] {
    let number = u32::from_str_radix(&value[1..], 16).expect("checked badge color");
    let rgba = if value.len() == 7 {
        (number << 8) | 255
    } else {
        number
    };
    [
        (rgba >> 24) as u8,
        (rgba >> 16) as u8,
        (rgba >> 8) as u8,
        ((rgba & 255) * u32::from(alpha) / 255) as u8,
    ]
}
pub(crate) struct Canvas {
    pub(crate) pixels: Vec<u8>,
    pub(crate) width: i32,
    pub(crate) height: i32,
}
impl Canvas {
    pub(crate) fn pixel(&mut self, x: i32, y: i32, c: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return;
        }
        let start = ((y * self.width + x) * 3) as usize;
        for (value, over) in self.pixels[start..start + 3].iter_mut().zip(c) {
            *value = ((u32::from(over) * u32::from(c[3])
                + u32::from(*value) * (255 - u32::from(c[3]))
                + 127)
                / 255) as u8;
        }
    }
    fn line(&mut self, mut x: i32, mut y: i32, end_x: i32, end_y: i32, c: [u8; 4]) {
        let (dx, dy) = ((end_x - x).abs(), -(end_y - y).abs());
        let (sx, sy) = (
            if x < end_x { 1 } else { -1 },
            if y < end_y { 1 } else { -1 },
        );
        let mut error = dx + dy;
        loop {
            self.pixel(x, y, c);
            self.pixel(x + 1, y, c);
            if x == end_x && y == end_y {
                break;
            }
            let twice = 2 * error;
            if twice >= dy {
                error += dy;
                x += sx;
            }
            if twice <= dx {
                error += dx;
                y += sy;
            }
        }
    }
    fn rounded(&mut self, bounds: [i32; 4], r: i32, fill: [u8; 4], border: [u8; 4], stroke: i32) {
        let [x, y, w, h] = bounds;
        for py in 0..h {
            for px in 0..w {
                let cx = px.clamp(r, w - r - 1);
                let cy = py.clamp(r, h - r - 1);
                let distance = (px - cx).pow(2) + (py - cy).pow(2);
                if distance > r * r {
                    continue;
                }
                let edge = px < stroke
                    || py < stroke
                    || px >= w - stroke
                    || py >= h - stroke
                    || distance > (r - stroke).max(0).pow(2);
                self.pixel(x + px, y + py, if edge { border } else { fill });
            }
        }
    }
}
pub fn draw(frame: &Frame, labels: &[(String, u8)]) -> Vec<u8> {
    let mut canvas = Canvas {
        pixels: frame.pixels().to_vec(),
        width: frame.width() as i32,
        height: frame.height() as i32,
    };
    draw_on(&mut canvas, labels);
    canvas.pixels
}
/// The badge stack, newest at the bottom right, over existing pixels.
pub(crate) fn draw_on(canvas: &mut Canvas, labels: &[(String, u8)]) {
    if labels.is_empty() {
        return;
    }
    let spec = spec();
    let style = &spec.badge;
    let height = style.icon_size + 2 * style.padding_y + 2 * style.border_width;
    let mut y = canvas.height
        - style.bottom
        - height * labels.len() as i32
        - style.gap * (labels.len() as i32 - 1);
    for (name, alpha) in labels {
        let item = &spec.controls[name];
        let text = item.text.as_ref().unwrap_or(&item.label);
        let content = if item.icon.is_some() {
            style.icon_size
        } else {
            text.chars().count() as i32 * 16
        };
        let width = content + 2 * style.padding_x + 2 * style.border_width;
        let x = canvas.width - width - 12;
        canvas.rounded(
            [x, y, width, height],
            style.radius,
            color(&style.background, *alpha),
            color(&style.border, *alpha),
            style.border_width,
        );
        let left = x + style.padding_x + style.border_width;
        let top = y + style.padding_y + style.border_width;
        let foreground = color(&style.foreground, *alpha);
        if let Some(icon) = &item.icon {
            let shape = &spec.icons[icon];
            let scale = |v: i32| v * style.icon_size / 24;
            for line in &shape.lines {
                for pair in line.windows(2) {
                    canvas.line(
                        left + scale(pair[0][0]),
                        top + scale(pair[0][1]),
                        left + scale(pair[1][0]),
                        top + scale(pair[1][1]),
                        foreground,
                    );
                }
            }
            if let Some([cx, cy, r]) = shape.circle {
                for py in -scale(r)..=scale(r) {
                    for px in -scale(r)..=scale(r) {
                        if px * px + py * py <= scale(r).pow(2) {
                            canvas.pixel(left + scale(cx) + px, top + scale(cy) + py, foreground);
                        }
                    }
                }
            }
        } else {
            for (i, ch) in text.chars().enumerate() {
                if let Some(glyph) = font8x8::BASIC_FONTS.get(ch) {
                    for (row, bits) in glyph.iter().enumerate() {
                        for col in 0..8 {
                            if bits & (1 << col) != 0 {
                                for dy in 0..2 {
                                    for dx in 0..2 {
                                        canvas.pixel(
                                            left + i as i32 * 16 + col * 2 + dx,
                                            top + row as i32 * 2 + dy,
                                            foreground,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        y += height + style.gap;
    }
}

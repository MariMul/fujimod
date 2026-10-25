//! Optional video presentation of a caller-supplied SD directory snapshot.
//! Captured frames and documentation screenshots retain their original pixels.
use crate::{
    Frame, Result,
    canvas::{Canvas, rounded_rect},
    text,
};
use serde::Deserialize;
use tiny_skia::PathBuilder;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardRow {
    pub name: String,
    pub path: String,
    pub depth: u32,
    pub directory: bool,
    pub bytes: u64,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardPanel {
    pub label: String,
    pub rows: Vec<CardRow>,
    pub highlight: Option<String>,
}

impl CardPanel {
    pub fn parse(json: &str) -> Result<Self> {
        let panel: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if panel.rows.len() > 4096
            || panel.label.chars().count() > 64
            || panel.rows.iter().any(|r| {
                r.depth > 32
                    || r.name.is_empty()
                    || r.name.chars().count() > 255
                    || r.name.chars().any(char::is_control)
            })
        {
            return Err("Invalid SD card presentation".into());
        }
        if panel
            .highlight
            .as_ref()
            .is_some_and(|path| !panel.rows.iter().any(|r| &r.path == path))
        {
            return Err("Highlighted file is absent from the SD snapshot".into());
        }
        Ok(panel)
    }
}

/// Reserve a side column without scaling, covering or changing the source raster.
pub(crate) fn reserve(frame: &Frame) -> Result<Frame> {
    let width = frame.width() + (frame.height() * 3 / 4);
    let mut rgb = vec![14; width as usize * frame.height() as usize * 3];
    for (source, target) in frame
        .pixels()
        .chunks_exact(frame.width() as usize * 3)
        .zip(rgb.chunks_exact_mut(width as usize * 3))
    {
        target[..source.len()].copy_from_slice(source);
    }
    Frame::rgb(width, frame.height(), rgb)
}

fn line(canvas: &mut Canvas, text: &str, x: f32, y: f32, size: f32, width: f32, color: [u8; 4]) {
    let mut shown = text.to_owned();
    while text::layout(&shown, x, y, size, size * 1.5).width > width && !shown.is_empty() {
        shown.pop();
    }
    if shown != text && !shown.is_empty() {
        shown.pop();
        shown.push('…');
    }
    if let Some(path) = text::layout(&shown, x, y, size, size * 1.5).path {
        canvas.fill(&path, color);
    }
}
fn bytes(value: u64) -> String {
    if value < 1024 {
        format!("{value} B")
    } else if value < 1024 * 1024 {
        format!("{:.1} KB", value as f64 / 1024.)
    } else {
        format!("{:.1} MB", value as f64 / (1024. * 1024.))
    }
}

pub(crate) fn draw(canvas: &mut Canvas, panel: &CardPanel) {
    let scale = canvas.height() / 480.;
    let (x, y, w, h) = (
        canvas.width() - 340. * scale,
        24. * scale,
        316. * scale,
        432. * scale,
    );
    let mut body = PathBuilder::new();
    body.move_to(x + 10. * scale, y);
    body.line_to(x + w - 42. * scale, y);
    body.line_to(x + w, y + 42. * scale);
    body.line_to(x + w, y + h - 10. * scale);
    body.quad_to(x + w, y + h, x + w - 10. * scale, y + h);
    body.line_to(x + 10. * scale, y + h);
    body.quad_to(x, y + h, x, y + h - 10. * scale);
    body.line_to(x, y + 10. * scale);
    body.quad_to(x, y, x + 10. * scale, y);
    body.close();
    let body = body.finish().expect("SD outline");
    canvas.fill(&body, [25, 25, 28, 255]);
    canvas.stroke(&body, [75, 75, 82, 255], scale);
    for index in 0..7 {
        if let Some(pad) = rounded_rect(
            x + (20. + index as f32 * 29.) * scale,
            y + 12. * scale,
            17. * scale,
            20. * scale,
            2. * scale,
        ) {
            canvas.fill(&pad, [145, 122, 67, 255]);
        }
    }
    line(
        canvas,
        "SD CARD",
        x + 18. * scale,
        y + 44. * scale,
        11. * scale,
        w - 36. * scale,
        [142, 142, 147, 255],
    );
    if !panel.label.is_empty() {
        line(
            canvas,
            &panel.label,
            x + 18. * scale,
            y + 63. * scale,
            17. * scale,
            w - 36. * scale,
            [238, 238, 242, 255],
        );
    }
    let rows = ((h / scale - 110.) / 27.) as usize;
    let selected = panel
        .rows
        .iter()
        .position(|r| Some(&r.path) == panel.highlight.as_ref());
    let start = selected
        .map_or(0, |i| (i + 1).saturating_sub(rows))
        .min(panel.rows.len().saturating_sub(rows));
    for (index, row) in panel.rows.iter().skip(start).take(rows).enumerate() {
        let top = y + (101. + index as f32 * 27.) * scale;
        let indent = row.depth.min(8) as f32 * 14. * scale;
        if Some(&row.path) == panel.highlight.as_ref() {
            if let Some(band) = rounded_rect(
                x + 10. * scale,
                top,
                w - 20. * scale,
                26. * scale,
                4. * scale,
            ) {
                canvas.fill(&band, [35, 55, 40, 255]);
            }
        }
        let left = x + 18. * scale + indent;
        if row.directory {
            let mut folder = PathBuilder::new();
            folder.move_to(left, top + 7. * scale);
            folder.line_to(left + 5. * scale, top + 7. * scale);
            folder.line_to(left + 7. * scale, top + 10. * scale);
            folder.line_to(left + 12. * scale, top + 10. * scale);
            folder.line_to(left + 12. * scale, top + 18. * scale);
            folder.line_to(left, top + 18. * scale);
            folder.close();
            canvas.stroke(
                &folder.finish().expect("folder"),
                [174, 174, 181, 255],
                scale,
            );
        } else if let Some(icon) = rounded_rect(
            left + 2. * scale,
            top + 7. * scale,
            9. * scale,
            12. * scale,
            scale,
        ) {
            canvas.stroke(&icon, [142, 142, 147, 255], scale);
        }
        let amount = if row.directory {
            String::new()
        } else {
            bytes(row.bytes)
        };
        let amount_width = text::layout(&amount, 0., 0., 10. * scale, 15. * scale).width;
        let right = x + w - 17. * scale;
        line(
            canvas,
            &row.name,
            left + 19. * scale,
            top + 3. * scale,
            12. * scale,
            right - left - 27. * scale - amount_width,
            [222, 222, 228, 255],
        );
        line(
            canvas,
            &amount,
            right - amount_width,
            top + 5. * scale,
            10. * scale,
            amount_width + scale,
            [142, 142, 147, 255],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panel_preserves_camera_pixels_and_requires_a_real_highlight() {
        let frame = Frame::rgb(720, 480, vec![60; 720 * 480 * 3]).unwrap();
        let padded = reserve(&frame).unwrap();
        let json = r#"{"label":"2026-10-25","rows":[{"name":"DCIM","path":"/DCIM","depth":0,"directory":true,"bytes":0}],"highlight":"/DCIM"}"#;
        let panel = CardPanel::parse(json).unwrap();
        let mut canvas = Canvas::from_rgb(padded.pixels(), padded.width(), padded.height());
        draw(&mut canvas, &panel);
        let shown = canvas.rgb();
        for row in shown.chunks_exact(padded.width() as usize * 3) {
            assert_eq!(&row[..720 * 3], &vec![60; 720 * 3]);
        }
        assert_ne!(shown, padded.pixels());
        assert!(CardPanel::parse(&json.replace("/DCIM\"}", "/MISSING\"}")).is_err());
    }
}

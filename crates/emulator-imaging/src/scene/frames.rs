use super::{FrameEdge, Recorded};
use crate::analysis::ImageLayout;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum ImageSource {
    External,
    Analytic,
    Sensor,
}
pub enum Pixels {
    Packed(Arc<[u8]>),
    Sensor(Arc<crate::radiometric::Image>),
}
impl Pixels {
    pub fn render(&self, layout: ImageLayout, workers: usize) -> Result<Arc<[u8]>, String> {
        match self {
            Self::Packed(p) => Ok(p.clone()),
            Self::Sensor(image) => image.render(layout, workers),
        }
    }
    pub fn packed_mut(
        &mut self,
        layout: ImageLayout,
        workers: usize,
    ) -> Result<&mut Arc<[u8]>, String> {
        if matches!(self, Self::Sensor(_)) {
            *self = Self::Packed(self.render(layout, workers)?);
        }
        match self {
            Self::Packed(p) => Ok(p),
            _ => unreachable!(),
        }
    }
    fn checkpoint(&self) -> Result<fujimod_emulator_devices::checkpoint::Checkpoint, String> {
        match self {
            Self::Packed(p) => Ok(
                fujimod_emulator_devices::checkpoint::Checkpoint::encode("image-pixels", &false)?
                    .with_blobs(vec![p.clone()]),
            ),
            Self::Sensor(image) => Ok(fujimod_emulator_devices::checkpoint::Checkpoint::encode(
                "image-pixels",
                &true,
            )?
            .with_children(vec![image.checkpoint()?])),
        }
    }
    fn from_checkpoint(
        saved: &fujimod_emulator_devices::checkpoint::Checkpoint,
        layout: ImageLayout,
    ) -> Result<Self, String> {
        let physical: bool = saved.decode("image-pixels")?;
        if physical {
            if saved.children().len() != 1 || !saved.blobs().is_empty() {
                return Err("invalid physical image pixels checkpoint".into());
            }
            Ok(Self::Sensor(Arc::new(
                crate::radiometric::Image::from_checkpoint(&saved.children()[0])?,
            )))
        } else {
            if !saved.children().is_empty()
                || saved.blobs().len() != 1
                || saved.blobs()[0].len() != layout.bytes()?
            {
                return Err("invalid packed image pixels checkpoint".into());
            }
            Ok(Self::Packed(saved.blobs()[0].clone()))
        }
    }
    fn fingerprint(&self) -> serde_json::Value {
        match self {
            Self::Packed(p) => serde_json::json!({"packed":fujimod_emulator_devices::fingerprint::bytes(p)}),
            Self::Sensor(image) => serde_json::json!({"sensor":image.fingerprint()}),
        }
    }
}
pub struct ImageFrame {
    pub edge: Option<FrameEdge>,
    pub layout: ImageLayout,
    pub pixels: Pixels,
    pub prepared: Option<Arc<[u8]>>,
    pub source: ImageSource,
    pub radiance: Option<crate::exposure::ExposedFrame>,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct ImageState {
    edge: Option<FrameEdge>,
    layout: ImageLayout,
    prepared: bool,
    source: ImageSource,
    radiance: bool,
}
/// Shared output from a modeled image source. Retained keyframes share immutable
/// bytes; publication copies only when an older image is still retained.
#[derive(Clone)]
pub struct ImageFrames(pub Arc<Mutex<ImageFrame>>);
impl ImageFrames {
    pub fn checkpoint(&self) -> Result<fujimod_emulator_devices::checkpoint::Checkpoint, String> {
        let frame = self.0.lock().map_err(|_| "image-frame poisoned")?;
        let state = ImageState {
            edge: frame.edge,
            layout: frame.layout,
            prepared: frame.prepared.is_some(),
            source: frame.source,
            radiance: frame.radiance.is_some(),
        };
        let mut blobs = Vec::new();
        if let Some(prepared) = &frame.prepared {
            blobs.push(prepared.clone());
        }
        let mut children: Vec<_> = frame
            .radiance
            .as_ref()
            .map(|r| r.checkpoint())
            .transpose()?
            .into_iter()
            .collect();
        children.insert(0, frame.pixels.checkpoint()?);
        Ok(
            fujimod_emulator_devices::checkpoint::Checkpoint::encode("image-frame", &state)?
                .with_blobs(blobs)
                .with_children(children),
        )
    }
    fn from_checkpoint(saved: &fujimod_emulator_devices::checkpoint::Checkpoint) -> Result<ImageFrame, String> {
        let state: ImageState = saved.decode("image-frame")?;
        let bytes = state.layout.bytes()?;
        if saved.blobs().len() != usize::from(state.prepared)
            || saved.blobs().iter().any(|b| b.len() != bytes)
            || saved.children().len() != 1 + usize::from(state.radiance)
        {
            return Err("image-frame snapshot extent differs".into());
        }
        Ok(ImageFrame {
            edge: state.edge,
            layout: state.layout,
            pixels: Pixels::from_checkpoint(&saved.children()[0], state.layout)?,
            prepared: state.prepared.then(|| saved.blobs()[0].clone()),
            source: state.source,
            radiance: if state.radiance {
                Some(crate::exposure::ExposedFrame::from_checkpoint(
                    &saved.children()[1],
                )?)
            } else {
                None
            },
        })
    }
    pub fn validate_checkpoint(&self, saved: &fujimod_emulator_devices::checkpoint::Checkpoint) -> Result<(), String> {
        Self::from_checkpoint(saved).map(|_| ())
    }
    pub fn restore_checkpoint(&self, saved: &fujimod_emulator_devices::checkpoint::Checkpoint) -> Result<(), String> {
        let frame = Self::from_checkpoint(saved)?;
        *self.0.lock().map_err(|_| "image-frame poisoned")? = frame;
        Ok(())
    }

    pub fn fingerprint(&self) -> serde_json::Value {
        let frame = self.0.lock().unwrap();
        serde_json::json!({"edge":frame.edge,"layout":[frame.layout.width,frame.layout.height,frame.layout.stride],
            "pixels":frame.pixels.fingerprint(),"prepared":frame.prepared.as_ref().map(|b|fujimod_emulator_devices::fingerprint::bytes(b)),"source":frame.source,
            "radiance":frame.radiance.as_ref().map(|f|f.fingerprint())})
    }
    pub fn new(layout: ImageLayout) -> Result<Self, String> {
        Self::from_pixels(layout, vec![0; layout.bytes()?])
    }
    /// Prepared external pixels with no frame edge yet. A producer publishes
    /// the first electrical edge during guest execution.
    pub fn from_pixels(layout: ImageLayout, bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() != layout.bytes()? {
            return Err("prepared image extent does not match its layout".into());
        }
        let result = Self(Arc::new(Mutex::new(ImageFrame {
            edge: None,
            layout,
            pixels: Pixels::Packed(bytes.into()),
            prepared: None,
            source: ImageSource::External,
            radiance: None,
        })));
        // Initialize host mutex storage before the first frame/access.
        drop(result.0.lock().unwrap());
        Ok(result)
    }
    /// Select external RGB input for the next physical frame edge. The
    /// currently published image remains immutable until that edge.
    pub fn prepare_rgb(&self, width: usize, height: usize, rgb: &[u8]) -> Result<(), String> {
        if width == 0
            || height == 0
            || width > 8192
            || height > 8192
            || width.checked_mul(height).and_then(|n| n.checked_mul(3)) != Some(rgb.len())
        {
            return Err("invalid external RGB raster".into());
        }
        let mut frame = self.0.lock().unwrap();
        let layout = frame.layout;
        let mut pixels = vec![0; layout.bytes()?];
        let sample = |x: usize, y: usize| {
            let i = (y * height / layout.height * width + x * width / layout.width) * 3;
            let (r, g, b) = (rgb[i] as i32, rgb[i + 1] as i32, rgb[i + 2] as i32);
            (
                (77 * r + 150 * g + 29 * b + 128) >> 8,
                ((-43 * r - 85 * g + 128 * b + 128) >> 8) + 128,
                ((128 * r - 107 * g - 21 * b + 128) >> 8) + 128,
            )
        };
        for y in 0..layout.height {
            for x in (0..layout.width).step_by(2) {
                let a = sample(x, y);
                let b = sample(x + 1, y);
                let i = y * layout.stride + x * 2;
                pixels[i..i + 4].copy_from_slice(&[
                    a.0.clamp(0, 255) as u8,
                    b.0.clamp(0, 255) as u8,
                    ((a.1 + b.1) / 2).clamp(0, 255) as u8,
                    ((a.2 + b.2) / 2).clamp(0, 255) as u8,
                ]);
            }
        }
        frame.prepared = Some(pixels.into());
        frame.source = ImageSource::External;
        Ok(())
    }
    pub fn use_analytic(&self) {
        let mut frame = self.0.lock().unwrap();
        frame.source = ImageSource::Analytic;
        frame.prepared = None;
    }
    pub fn use_sensor(&self) {
        let mut frame = self.0.lock().unwrap();
        frame.source = ImageSource::Sensor;
        frame.prepared = None;
    }
    pub fn publish(&self, edge: FrameEdge, bytes: &[u8]) -> Result<(), String> {
        let mut frame = self.0.lock().unwrap();
        if edge.index == 0
            || frame
                .edge
                .is_some_and(|p| edge.index <= p.index || edge.tick < p.tick)
            || bytes.len() != frame.layout.bytes()?
        {
            return Err("invalid image frame edge or extent".into());
        }
        match &mut frame.pixels {
            Pixels::Packed(pixels) => Arc::make_mut(pixels).copy_from_slice(bytes),
            Pixels::Sensor(_) => frame.pixels = Pixels::Packed(Arc::from(bytes)),
        }
        frame.edge = Some(edge);
        Ok(())
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StatisticsSpan {
    pub destination: usize,
    pub offset: u32,
    pub bytes: Vec<u8>,
}
#[derive(serde::Serialize, serde::Deserialize)]
pub struct StatisticsFrame {
    pub edge: Option<FrameEdge>,
    pub spans: Vec<StatisticsSpan>,
}
/// A fixed-layout bank input, including optional destination 9 (histogram).
/// External producers update bytes in place before publishing the next edge.
#[derive(Clone)]
pub struct StatisticsFrames(pub Arc<Mutex<StatisticsFrame>>);
impl StatisticsFrames {
    pub fn checkpoint(&self) -> Result<fujimod_emulator_devices::checkpoint::Checkpoint, String> {
        fujimod_emulator_devices::checkpoint::Checkpoint::encode(
            "statistics-frame",
            &*self.0.lock().map_err(|_| "statistics-frame poisoned")?,
        )
    }
    pub fn validate_checkpoint(&self, saved: &fujimod_emulator_devices::checkpoint::Checkpoint) -> Result<(), String> {
        let _: StatisticsFrame = saved.decode("statistics-frame")?;
        Ok(())
    }
    pub fn restore_checkpoint(&self, saved: &fujimod_emulator_devices::checkpoint::Checkpoint) -> Result<(), String> {
        *self.0.lock().map_err(|_| "statistics-frame poisoned")? =
            saved.decode::<StatisticsFrame>("statistics-frame")?;
        Ok(())
    }

    pub fn fingerprint(&self) -> serde_json::Value {
        let frame = self.0.lock().unwrap();
        serde_json::json!({"edge":frame.edge,"spans":frame.spans.iter().map(|s|serde_json::json!({
            "destination":s.destination,"offset":s.offset,"bytes":fujimod_emulator_devices::fingerprint::bytes(&s.bytes)})).collect::<Vec<_>>()})
    }
    pub fn new(spans: Vec<StatisticsSpan>) -> Result<Self, String> {
        let mut total = 0usize;
        if spans.len() > 256 {
            return Err("statistics span count exceeds its bound".into());
        }
        for (i, s) in spans.iter().enumerate() {
            total = total.saturating_add(s.bytes.len());
            if s.destination > 9
                || s.bytes.is_empty()
                || s.offset & 3 != 0
                || s.bytes.len() & 3 != 0
                || s.offset as u64 + s.bytes.len() as u64 > 64 << 20
                || total > 64 << 20
                || spans[..i].iter().any(|p| {
                    p.destination == s.destination
                        && (p.offset as u64) < s.offset as u64 + s.bytes.len() as u64
                        && (s.offset as u64) < p.offset as u64 + p.bytes.len() as u64
                })
            {
                return Err("invalid statistics span layout".into());
            }
        }
        let result = Self(Arc::new(Mutex::new(StatisticsFrame { edge: None, spans })));
        drop(result.0.lock().unwrap());
        Ok(result)
    }
    pub fn recorded(recorded: &Recorded) -> Result<Self, String> {
        Self::new(
            recorded
                .spans()
                .map(|(s, b)| StatisticsSpan {
                    destination: s.destination,
                    offset: s.destination_offset,
                    bytes: b.to_vec(),
                })
                .collect(),
        )
    }
    pub fn publish(&self, edge: FrameEdge) -> Result<(), String> {
        let mut frame = self.0.lock().unwrap();
        if edge.index == 0
            || frame
                .edge
                .is_some_and(|p| edge.index <= p.index || edge.tick < p.tick)
        {
            return Err("statistics frame edge must be monotonic".into());
        }
        frame.edge = Some(edge);
        Ok(())
    }
    pub fn update(&self, index: usize, bytes: &[u8]) -> Result<(), String> {
        let mut frame = self.0.lock().unwrap();
        let span = frame
            .spans
            .get_mut(index)
            .ok_or("unknown statistics span")?;
        if span.bytes.len() != bytes.len() {
            return Err("statistics input extent changed".into());
        }
        span.bytes.copy_from_slice(bytes);
        Ok(())
    }
}

#[cfg(test)]
mod checkpoint_tests {
    use super::*;

    #[test]
    fn image_keyframes_share_pixels_and_preserve_pending_input_across_publication() {
        let layout = ImageLayout {
            width: 2,
            height: 1,
            stride: 4,
        };
        let image = ImageFrames::from_pixels(layout, vec![1, 2, 128, 128]).unwrap();
        let initial = image.checkpoint().unwrap();
        assert!(Arc::ptr_eq(
            &initial.children()[0].blobs()[0],
            &image.0.lock().unwrap().pixels.render(layout, 0).unwrap()
        ));
        image.prepare_rgb(2, 1, &[255, 0, 0, 0, 255, 0]).unwrap();
        let before = image.fingerprint();
        let pending = image.checkpoint().unwrap();
        assert_eq!(pending.blobs().len(), 1);
        assert!(Arc::ptr_eq(
            &pending.blobs()[0],
            image.0.lock().unwrap().prepared.as_ref().unwrap()
        ));
        image
            .publish(FrameEdge { index: 1, tick: 10 }, &[50, 60, 128, 128])
            .unwrap();
        assert_eq!(&*initial.children()[0].blobs()[0], &[1, 2, 128, 128]);
        assert!(!Arc::ptr_eq(
            &initial.children()[0].blobs()[0],
            &image.0.lock().unwrap().pixels.render(layout, 0).unwrap()
        ));
        image.use_analytic();
        image.restore_checkpoint(&pending).unwrap();
        assert_eq!(image.fingerprint(), before);
        assert!(Arc::ptr_eq(
            &pending.children()[0].blobs()[0],
            &image.0.lock().unwrap().pixels.render(layout, 0).unwrap()
        ));
        let invalid = fujimod_emulator_devices::checkpoint::Checkpoint::encode(
            "image-frame",
            &ImageState {
                edge: None,
                layout,
                prepared: true,
                source: ImageSource::External,
                radiance: false,
            },
        )
        .unwrap()
        .with_blobs(vec![Arc::from([0u8; 4])]);
        assert!(image.restore_checkpoint(&invalid).is_err());
        assert_eq!(image.fingerprint(), before);
    }
}

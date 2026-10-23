//! Bounded host input preparation, outside device execution.
use fujimod_emulator_exr2_board::Inputs;
use fujimod_emulator_devices::{
    imaging::analysis::ImageLayout,
    scene::{Frame, FramePixels, ImageFrames, SensorFrames, StatisticsFrames, StatisticsSpan},
};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RgbSource {
    width: usize,
    height: usize,
    asset: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Image {
    width: usize,
    height: usize,
    stride: usize,
    asset: String,
    rgb_source: Option<RgbSource>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Span {
    destination: usize,
    offset: u32,
    asset: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Pixels {
    Rgb24,
    Raw16,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sensor {
    width: usize,
    height: usize,
    pixels: Pixels,
    asset: String,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    #[serde(default)]
    nands: BTreeMap<String, fujimod_emulator_devices::storage::media::NandInput>,
    #[serde(default)]
    cards: BTreeMap<String, fujimod_emulator_devices::storage::media::CardInput>,
    #[serde(default)]
    images: BTreeMap<String, Image>,
    #[serde(default)]
    statistics: BTreeMap<String, Vec<Span>>,
    #[serde(default)]
    sensors: BTreeMap<String, Sensor>,
    #[serde(default)]
    files: BTreeMap<String, BTreeMap<String, String>>,
}
impl Spec {
    pub fn build(self, assets: &BTreeMap<String, Vec<u8>>) -> Result<Inputs, String> {
        let asset = |name: &str| {
            assets
                .get(name)
                .ok_or_else(|| format!("missing scene asset {name}"))
        };
        let mut inputs = Inputs::default();
        for (name, card) in self.cards {
            inputs.cards.insert(name, card.build(assets)?);
        }
        for (name, nand) in self.nands {
            inputs.nands.insert(name, nand.build(assets)?);
        }
        for (name, image) in self.images {
            let layout = ImageLayout {
                width: image.width,
                height: image.height,
                stride: image.stride,
            };
            let bytes = asset(&image.asset)?;
            if bytes.len() != layout.bytes()? {
                return Err("image asset extent does not match its declared raster".into());
            }
            let frames = ImageFrames::from_pixels(layout, bytes.clone())?;
            if let Some(source) = image.rgb_source {
                frames.prepare_rgb(source.width, source.height, asset(&source.asset)?)?;
            }
            inputs.images.insert(name, frames);
        }
        for (name, spans) in self.statistics {
            if spans.len() > 256 {
                return Err("statistics input has too many spans".into());
            }
            let mut total = 0usize;
            let spans = spans
                .into_iter()
                .map(|s| {
                    let bytes = asset(&s.asset)?;
                    total = total.saturating_add(bytes.len());
                    if total > 64 << 20 {
                        return Err("statistics input exceeds 64 MiB".into());
                    }
                    Ok(StatisticsSpan {
                        destination: s.destination,
                        offset: s.offset,
                        bytes: bytes.clone(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            inputs
                .statistics
                .insert(name, StatisticsFrames::new(spans)?);
        }
        for (name, files) in self.files {
            let mut total = 0usize;
            if files.len() > 4096 {
                return Err("card input has too many files".into());
            }
            let files = files
                .into_iter()
                .map(|(path, source)| {
                    let bytes = asset(&source)?;
                    total = total.saturating_add(bytes.len());
                    if total > 64 << 20 {
                        return Err("card input exceeds 64 MiB".into());
                    }
                    Ok((path, bytes.clone()))
                })
                .collect::<Result<Vec<_>, String>>()?;
            inputs.files.insert(name, files);
        }
        for (name, sensor) in self.sensors {
            let bytes = asset(&sensor.asset)?;
            let pixels = match sensor.pixels {
                Pixels::Rgb24 => FramePixels::Rgb24(bytes.clone().into()),
                Pixels::Raw16 => {
                    if bytes.len() % 2 != 0 {
                        return Err("RAW16 scene asset has an incomplete sample".into());
                    }
                    FramePixels::Raw16(
                        bytes
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|b| u16::from_le_bytes(*b))
                            .collect(),
                    )
                }
            };
            inputs.sensors.insert(
                name,
                Arc::new(Mutex::new(SensorFrames::new(Frame::new(
                    sensor.width,
                    sensor.height,
                    pixels,
                )?))),
            );
        }
        Ok(inputs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initial_rgb_waits_for_a_real_frame_edge_and_validates_extent() {
        let assets = BTreeMap::from([
            ("packed".into(), vec![1, 2, 128, 128]),
            ("rgb".into(), vec![255, 0, 0]),
        ]);
        let spec = serde_json::json!({"images":{"live":{"width":2,"height":1,"stride":4,
            "asset":"packed","rgb_source":{"width":1,"height":1,"asset":"rgb"}}}});
        let inputs = serde_json::from_value::<Spec>(spec.clone())
            .unwrap()
            .build(&assets)
            .unwrap();
        let image = &inputs.images["live"];
        let baseline = ImageFrames::from_pixels(
            ImageLayout {
                width: 2,
                height: 1,
                stride: 4,
            },
            vec![1, 2, 128, 128],
        )
        .unwrap();
        assert_eq!(image.fingerprint()["edge"], serde_json::Value::Null);
        assert_eq!(
            image.fingerprint()["bytes"],
            baseline.fingerprint()["bytes"]
        );
        assert_ne!(image.fingerprint()["prepared"], serde_json::Value::Null);
        let mut invalid = spec;
        invalid["images"]["live"]["rgb_source"]["width"] = serde_json::json!(2);
        assert!(
            serde_json::from_value::<Spec>(invalid)
                .unwrap()
                .build(&assets)
                .is_err()
        );
    }
}

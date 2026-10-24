//! A resource archive includes external pixels and media. Shared wires are
//! rebound by the receiving graph's existing checkpoint validator/restorer.
use super::*;
use fujimod_emulator_imaging::scene::{Frame, FrameEdge, SensorFrames};
use serde::{Deserialize, Serialize};

const KIND: &str = "session-resources-v1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    wires: Vec<String>,
    images: Vec<String>,
    statistics: Vec<String>,
    nands: Vec<String>,
    sensors: Vec<Sensor>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sensor {
    name: String,
    edge: Option<FrameEdge>,
    shared: bool,
}

impl ResourceSnapshot {
    /// Encode captured values only, including both current and latched sensor
    /// inputs. The live graph is never inspected or advanced by this export.
    pub fn export(&self) -> Result<Checkpoint, String> {
        let metadata = Metadata {
            wires: self.wires.keys().cloned().collect(),
            images: self.images.keys().cloned().collect(),
            statistics: self.statistics.keys().cloned().collect(),
            nands: self.nands.keys().cloned().collect(),
            sensors: self
                .sensors
                .iter()
                .map(|(name, frames)| Sensor {
                    name: name.clone(),
                    edge: frames.edge,
                    shared: Arc::ptr_eq(&frames.current, &frames.latched),
                })
                .collect(),
        };
        let mut children: Vec<_> = self
            .wires
            .values()
            .chain(self.images.values())
            .chain(self.statistics.values())
            .chain(self.nands.values())
            .chain(std::iter::once(&self.media))
            .cloned()
            .collect();
        for (frames, sensor) in self.sensors.values().zip(&metadata.sensors) {
            children.push(frames.current.checkpoint()?);
            if !sensor.shared {
                children.push(frames.latched.checkpoint()?);
            }
        }
        Ok(Checkpoint::encode(KIND, &metadata)?.with_children(children))
    }

    /// Decode detached data. Call `Resources::validate_checkpoint` against the
    /// fresh graph before installing any decoded resource values.
    pub fn from_checkpoint(encoded: &Checkpoint) -> Result<Self, String> {
        let metadata: Metadata = encoded.decode(KIND)?;
        if !encoded.blobs().is_empty() {
            return Err("resource snapshot has unexpected blobs".into());
        }
        let ordered = |names: Vec<&str>| names.windows(2).all(|v| v[0] < v[1]);
        if [
            &metadata.wires,
            &metadata.images,
            &metadata.statistics,
            &metadata.nands,
        ]
        .iter()
        .any(|names| !ordered(names.iter().map(String::as_str).collect()))
            || !ordered(metadata.sensors.iter().map(|s| s.name.as_str()).collect())
        {
            return Err("resource snapshot names must be unique and ordered".into());
        }
        let mut children = encoded.children().iter();
        let mut next = || {
            children
                .next()
                .ok_or("resource snapshot is missing a child")
        };
        let mut group = |names: Vec<String>| -> Result<BTreeMap<String, Checkpoint>, String> {
            names
                .into_iter()
                .map(|name| Ok((name, next()?.clone())))
                .collect()
        };
        let wires = group(metadata.wires)?;
        let images = group(metadata.images)?;
        let statistics = group(metadata.statistics)?;
        let nands = group(metadata.nands)?;
        let media = next()?.clone();
        let sensors = metadata
            .sensors
            .into_iter()
            .map(|sensor| {
                if sensor.edge.is_some_and(|edge| edge.index == 0) {
                    return Err("resource snapshot sensor edge index is zero".into());
                }
                let current = Arc::new(Frame::from_checkpoint(next()?)?);
                let latched = if sensor.shared {
                    current.clone()
                } else {
                    Arc::new(Frame::from_checkpoint(next()?)?)
                };
                Ok((
                    sensor.name,
                    SensorFrames {
                        current,
                        latched,
                        edge: sensor.edge,
                    },
                ))
            })
            .collect::<Result<_, String>>()?;
        if children.next().is_some() {
            return Err("resource snapshot has extra children".into());
        }
        Ok(Self {
            wires,
            images,
            statistics,
            sensors,
            media,
            nands,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fujimod_emulator_devices::parts::sd_card;
    use fujimod_emulator_imaging::scene::FramePixels;

    fn resources() -> Resources {
        let mut resources = Resources::default();
        resources
            .wires
            .insert("cable".into(), Wire::logic_level(false));
        let sensor = SensorFrames::new(Frame::new(2, 1, FramePixels::Raw16(vec![0; 2])).unwrap());
        resources
            .inputs
            .sensors
            .insert("sensor".into(), Arc::new(Mutex::new(sensor)));
        resources.inputs.cards.insert(
            "sd".into(),
            Card::new(sd_card::Options {
                sectors: 0x400000,
                max_dirty_sectors: 16,
            })
            .unwrap(),
        );
        resources
    }

    #[test]
    fn resource_archive_restores_into_fresh_wires_media_and_sensor_handles() {
        let (encoded, expected) = {
            let mut source = resources();
            let Wire::LogicLevel(level) = &source.wires["cable"] else {
                unreachable!()
            };
            level.drive(true);
            source
                .inputs
                .cards
                .get_mut("sd")
                .unwrap()
                .write_bytes(3, &[0x42; 700])
                .unwrap();
            let mut sensor = source.inputs.sensors["sensor"].lock().unwrap();
            sensor.latch(42).unwrap();
            sensor.set_frame(
                Frame::new(2, 1, FramePixels::Rgb24(vec![1, 2, 3, 4, 5, 6].into())).unwrap(),
            );
            drop(sensor);
            let exported = source.checkpoint().unwrap().export().unwrap();
            (exported.to_bytes().unwrap(), source.fingerprint().unwrap())
        };
        let archive = Checkpoint::from_bytes(&encoded).unwrap();
        let saved = ResourceSnapshot::from_checkpoint(&archive).unwrap();
        let mut target = resources();
        let handle = target.inputs.sensors["sensor"].clone();
        let before = target.fingerprint().unwrap();
        target.validate_checkpoint(&saved).unwrap();
        assert_eq!(target.fingerprint().unwrap(), before);
        target.restore_checkpoint(&saved).unwrap();
        assert_eq!(target.fingerprint().unwrap(), expected);
        assert!(Arc::ptr_eq(&target.inputs.sensors["sensor"], &handle));
        let sensor = handle.lock().unwrap();
        assert!(!Arc::ptr_eq(&sensor.current, &sensor.latched));
        assert_eq!(sensor.edge, Some(FrameEdge { tick: 42, index: 1 }));
        drop(sensor);
        let shared = resources().checkpoint().unwrap().export().unwrap();
        let shared = ResourceSnapshot::from_checkpoint(&shared).unwrap();
        assert!(Arc::ptr_eq(
            &shared.sensors["sensor"].current,
            &shared.sensors["sensor"].latched
        ));
    }

    #[test]
    fn invalid_resource_archives_are_refused_before_mutating_the_graph() {
        let target = resources();
        let saved = target.checkpoint().unwrap();
        let archive = saved.export().unwrap();
        let before = target.fingerprint().unwrap();
        for bad in [
            archive.clone().with_children(vec![]),
            archive.clone().with_blobs(vec![vec![1].into()]),
        ] {
            assert!(ResourceSnapshot::from_checkpoint(&bad).is_err());
        }
        let mut metadata: Metadata = archive.decode(KIND).unwrap();
        metadata.wires.push("cable".into());
        let bad = Checkpoint::encode(KIND, &metadata)
            .unwrap()
            .with_children(archive.children().to_vec());
        assert!(ResourceSnapshot::from_checkpoint(&bad).is_err());
        let mut metadata: Metadata = archive.decode(KIND).unwrap();
        metadata.sensors[0].edge = Some(FrameEdge { tick: 42, index: 0 });
        let bad = Checkpoint::encode(KIND, &metadata)
            .unwrap()
            .with_children(archive.children().to_vec());
        assert!(ResourceSnapshot::from_checkpoint(&bad).is_err());
        let mut corrupt = saved;
        let card: Card = serde_json::from_value(serde_json::json!({
            "sectors":1,"max_dirty":1,"revision":0,"data":{}
        }))
        .unwrap();
        corrupt.media = Checkpoint::encode(
            "resource-media",
            &(
                BTreeMap::from([("sd".to_owned(), card)]),
                BTreeMap::<String, Vec<(String, Vec<u8>)>>::new(),
            ),
        )
        .unwrap();
        let imported = ResourceSnapshot::from_checkpoint(&corrupt.export().unwrap()).unwrap();
        assert!(target.validate_checkpoint(&imported).is_err());
        assert_eq!(target.fingerprint().unwrap(), before);
    }
}

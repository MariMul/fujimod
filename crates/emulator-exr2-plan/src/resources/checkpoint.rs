//! The graph owns shared wire identities. Restore their values in place.
mod portable;
use super::*;
use fujimod_emulator_devices::checkpoint::Checkpoint;

#[derive(Clone)]
pub struct ResourceSnapshot {
    wires: BTreeMap<String, Checkpoint>,
    images: BTreeMap<String, Checkpoint>,
    statistics: BTreeMap<String, Checkpoint>,
    sensors: BTreeMap<String, fujimod_emulator_imaging::scene::SensorFrames>,
    media: Checkpoint,
    nands: BTreeMap<String, Checkpoint>,
}
impl ResourceSnapshot {
    /// Encoded mutable resource state; shared sensor rasters remain external inputs.
    pub fn encoded_bytes(&self) -> usize {
        self.wires
            .values()
            .chain(self.images.values())
            .chain(self.statistics.values())
            .chain(self.nands.values())
            .map(Checkpoint::bytes)
            .sum::<usize>()
            + self.media.bytes()
    }
}
impl Wire {
    fn checkpoint(&self) -> Result<Checkpoint, String> {
        Ok(match self {
            Self::FramePeriod(v) => Checkpoint::encode(
                "sensor-frame-period",
                &*v.lock().map_err(|_| "frame period poisoned")?,
            )?,
            Self::ImageGrid(v) => Checkpoint::encode(
                "sensor-image-grid",
                &*v.lock().map_err(|_| "image grid poisoned")?,
            )?,
            Self::Focus(v) => {
                Checkpoint::encode("physical-focus", &*v.lock().map_err(|_| "focus poisoned")?)?
            }
            Self::Exposure(v) => Checkpoint::encode(
                "physical-exposure",
                &*v.lock().map_err(|_| "exposure poisoned")?,
            )?,
            Self::Aperture(v) => Checkpoint::encode(
                "physical-aperture",
                &*v.lock().map_err(|_| "aperture poisoned")?,
            )?,
            Self::SoftwarePulse(v) => Checkpoint::encode("software-pulse", &v.asserted())?,
            Self::LogicLevel(v) => Checkpoint::encode("logic-level", &v.high())?,
            Self::OpticalPosition(v) => Checkpoint::encode("optical-position", &v.value())?,
            Self::Parameters(v) => {
                Checkpoint::encode("parameters", &*v.lock().map_err(|_| "parameters poisoned")?)?
            }
            Self::MemoryOutputs(v) => Checkpoint::encode(
                "memory-outputs",
                &*v.lock().map_err(|_| "memory outputs poisoned")?,
            )?,
            Self::Stream(v) => {
                Checkpoint::encode("stream", &*v.lock().map_err(|_| "stream poisoned")?)?
            }
            Self::PhaseEdges(v) => Checkpoint::encode(
                "phase-edges",
                &*v.lock().map_err(|_| "phase edges poisoned")?,
            )?,
            Self::OutputChannels(v) => v.checkpoint()?,
            Self::IrqRoutes(v) => v.checkpoint()?,
            Self::BridgeRegisters(v) => Checkpoint::encode(
                "bridge-registers",
                &(0..4)
                    .map(|i| v.read_register(i))
                    .collect::<Result<Vec<_>, _>>()?,
            )?,
        })
    }
    fn validate_checkpoint(&self, saved: &Checkpoint) -> Result<(), String> {
        match self {
            Self::FramePeriod(_) => {
                let _: u64 = saved.decode("sensor-frame-period")?;
            }
            Self::ImageGrid(_) => {
                let grid: [usize; 2] = saved.decode("sensor-image-grid")?;
                if grid.iter().any(|n| !(1..=16384).contains(n)) {
                    return Err("invalid sensor image grid checkpoint".into());
                }
            }
            Self::Focus(_) => {
                let position: f64 = saved.decode("physical-focus")?;
                if !position.is_finite() {
                    return Err("invalid physical focus checkpoint".into());
                }
            }
            Self::Exposure(_) => {
                let value: fujimod_emulator_imaging::exposure::Exposure =
                    saved.decode("physical-exposure")?;
                if !value.seconds.is_finite()
                    || value.seconds < 0.
                    || !value.analog_gain.is_finite()
                    || value.analog_gain < 0.
                    || value.seconds > 0. && value.analog_gain == 0.
                {
                    return Err("invalid physical exposure checkpoint".into());
                }
            }
            Self::Aperture(_) => {
                let value: f64 = saved.decode("physical-aperture")?;
                if !value.is_finite() || value <= 0. {
                    return Err("invalid physical aperture checkpoint".into());
                }
            }
            Self::SoftwarePulse(_) => {
                let _: bool = saved.decode("software-pulse")?;
            }
            Self::LogicLevel(_) => {
                let _: bool = saved.decode("logic-level")?;
            }
            Self::OpticalPosition(_) => {
                let _: i64 = saved.decode("optical-position")?;
            }
            Self::Parameters(_) => {
                let _: isp::parameters::Parameters = saved.decode("parameters")?;
            }
            Self::MemoryOutputs(_) => {
                let _: fujimod_emulator_imaging::exr2_isp::memory_outputs::MemoryOutputs =
                    saved.decode("memory-outputs")?;
            }
            Self::Stream(_) => {
                let _: isp::stream::Stream = saved.decode("stream")?;
            }
            Self::PhaseEdges(_) => {
                let _: [Option<fujimod_emulator_imaging::scene::FrameEdge>; 2] =
                    saved.decode("phase-edges")?;
            }
            Self::OutputChannels(v) => v.validate_checkpoint(saved)?,
            Self::IrqRoutes(v) => v.validate_checkpoint(saved)?,
            Self::BridgeRegisters(_) => {
                let bytes: Vec<u8> = saved.decode("bridge-registers")?;
                if bytes.len() != 4 {
                    return Err("bridge snapshot extent differs".into());
                }
            }
        }
        Ok(())
    }
    fn restore_checkpoint(&self, saved: &Checkpoint) -> Result<(), String> {
        match self {
            Self::FramePeriod(v) => {
                *v.lock().map_err(|_| "frame period poisoned")? =
                    saved.decode("sensor-frame-period")?;
            }
            Self::ImageGrid(v) => {
                *v.lock().map_err(|_| "image grid poisoned")? =
                    saved.decode("sensor-image-grid")?;
            }
            Self::Focus(v) => {
                *v.lock().map_err(|_| "focus poisoned")? = saved.decode("physical-focus")?;
            }
            Self::Exposure(v) => {
                *v.lock().map_err(|_| "exposure poisoned")? = saved.decode("physical-exposure")?
            }
            Self::Aperture(v) => {
                *v.lock().map_err(|_| "aperture poisoned")? = saved.decode("physical-aperture")?
            }
            Self::SoftwarePulse(v) => v.restore_level(saved.decode("software-pulse")?),
            Self::LogicLevel(v) => v.drive(saved.decode("logic-level")?),
            Self::OpticalPosition(v) => v.publish(saved.decode("optical-position")?),
            Self::Parameters(v) => {
                *v.lock().map_err(|_| "parameters poisoned")? = saved.decode("parameters")?
            }
            Self::MemoryOutputs(v) => {
                *v.lock().map_err(|_| "memory outputs poisoned")? =
                    saved.decode("memory-outputs")?
            }
            Self::Stream(v) => {
                *v.lock().map_err(|_| "stream poisoned")? = saved.decode("stream")?
            }
            Self::PhaseEdges(v) => {
                *v.lock().map_err(|_| "phase edges poisoned")? = saved.decode("phase-edges")?
            }
            Self::OutputChannels(v) => v.restore_checkpoint(saved)?,
            Self::IrqRoutes(v) => v.restore_checkpoint(saved)?,
            Self::BridgeRegisters(v) => {
                for (i, b) in saved
                    .decode::<Vec<u8>>("bridge-registers")?
                    .into_iter()
                    .enumerate()
                {
                    v.write_register(i as u8, b)?;
                }
            }
        }
        Ok(())
    }
}
fn same_keys<A, B>(a: &BTreeMap<String, A>, b: &BTreeMap<String, B>) -> Result<(), String> {
    if a.keys().ne(b.keys()) {
        Err("snapshot graph resource names differ".into())
    } else {
        Ok(())
    }
}
impl Resources {
    pub fn checkpoint(&self) -> Result<ResourceSnapshot, String> {
        Ok(ResourceSnapshot {
            wires: self
                .wires
                .iter()
                .map(|(k, v)| Ok((k.clone(), v.checkpoint()?)))
                .collect::<Result<_, String>>()?,
            images: self
                .inputs
                .images
                .iter()
                .map(|(k, v)| Ok((k.clone(), v.checkpoint()?)))
                .collect::<Result<_, String>>()?,
            statistics: self
                .inputs
                .statistics
                .iter()
                .map(|(k, v)| Ok((k.clone(), v.checkpoint()?)))
                .collect::<Result<_, String>>()?,
            sensors: self
                .inputs
                .sensors
                .iter()
                .map(|(k, v)| {
                    Ok((
                        k.clone(),
                        v.lock().map_err(|_| "sensor frames poisoned")?.clone(),
                    ))
                })
                .collect::<Result<_, String>>()?,
            media: Checkpoint::encode("resource-media", &(&self.inputs.cards, &self.inputs.files))?,
            nands: self
                .inputs
                .nands
                .iter()
                .map(|(k, v)| Ok((k.clone(), v.checkpoint()?)))
                .collect::<Result<_, String>>()?,
        })
    }
    pub fn validate_checkpoint(&self, saved: &ResourceSnapshot) -> Result<(), String> {
        same_keys(&self.wires, &saved.wires)?;
        same_keys(&self.inputs.images, &saved.images)?;
        same_keys(&self.inputs.statistics, &saved.statistics)?;
        same_keys(&self.inputs.sensors, &saved.sensors)?;
        same_keys(&self.inputs.nands, &saved.nands)?;
        for (k, v) in &self.wires {
            v.validate_checkpoint(&saved.wires[k])?;
        }
        for (k, v) in &self.inputs.images {
            v.validate_checkpoint(&saved.images[k])?;
        }
        for (k, v) in &self.inputs.statistics {
            v.validate_checkpoint(&saved.statistics[k])?;
        }
        let (cards, files): (
            BTreeMap<String, Card>,
            BTreeMap<String, Vec<(String, Vec<u8>)>>,
        ) = saved.media.decode("resource-media")?;
        same_keys(&self.inputs.cards, &cards)?;
        same_keys(&self.inputs.files, &files)?;
        for card in cards.values() {
            card.validate()?;
        }
        for v in saved.nands.values() {
            fujimod_emulator_devices::parts::nand::Nand::from_checkpoint(v)?;
        }
        Ok(())
    }
    pub fn restore_checkpoint(&mut self, saved: &ResourceSnapshot) -> Result<(), String> {
        self.validate_checkpoint(saved)?;
        for (k, v) in &self.wires {
            v.restore_checkpoint(&saved.wires[k])?;
        }
        for (k, v) in &self.inputs.images {
            v.restore_checkpoint(&saved.images[k])?;
        }
        for (k, v) in &self.inputs.statistics {
            v.restore_checkpoint(&saved.statistics[k])?;
        }
        for (k, v) in &self.inputs.sensors {
            *v.lock().map_err(|_| "sensor frames poisoned")? = saved.sensors[k].clone();
        }
        let (cards, files) = saved.media.decode("resource-media")?;
        self.inputs.cards = cards;
        self.inputs.files = files;
        self.inputs.nands = saved
            .nands
            .iter()
            .map(|(k, v)| {
                Ok((
                    k.clone(),
                    fujimod_emulator_devices::parts::nand::Nand::from_checkpoint(v)?,
                ))
            })
            .collect::<Result<_, String>>()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fujimod_emulator_imaging::scene::{Frame, FrameEdge, FramePixels, SensorFrames};
    #[test]
    fn checkpoint_graph_resources_restore_existing_stream_scene_and_signal_handles() {
        let mut resources = Resources::default();
        let logic = fujimod_emulator_devices::signals::Level::new(false);
        let position = fujimod_emulator_imaging::scene::optics::Position::default();
        position.publish(23);
        resources
            .wires
            .insert("focus".into(), Wire::OpticalPosition(position.clone()));
        let stream: isp::stream::StreamHandle = Default::default();
        let phases: fujimod_emulator_imaging::scene::FramePhases = Default::default();
        resources
            .wires
            .insert("cable".into(), Wire::LogicLevel(logic.clone()));
        resources
            .wires
            .insert("stream".into(), Wire::Stream(stream.clone()));
        resources
            .wires
            .insert("phases".into(), Wire::PhaseEdges(phases.clone()));
        let image = ImageFrames::new(fujimod_emulator_imaging::analysis::ImageLayout {
            width: 8,
            height: 2,
            stride: 16,
        })
        .unwrap();
        image.prepare_rgb(2, 1, &[255, 0, 0, 0, 255, 0]).unwrap();
        resources
            .inputs
            .images
            .insert("preview".into(), image.clone());
        let sensor = Arc::new(Mutex::new(SensorFrames::new(
            Frame::new(2, 1, FramePixels::Raw16(vec![1, 2])).unwrap(),
        )));
        sensor.lock().unwrap().latch(5).unwrap();
        sensor
            .lock()
            .unwrap()
            .set_frame(Frame::new(2, 1, FramePixels::Raw16(vec![3, 4])).unwrap());
        resources
            .inputs
            .sensors
            .insert("sensor".into(), sensor.clone());
        *phases.lock().unwrap() = [Some(FrameEdge { tick: 5, index: 1 }), None];
        stream.lock().unwrap().pixels = vec![9, 8, 7, 6].into();
        let before = resources.fingerprint().unwrap();
        let saved = resources.checkpoint().unwrap();
        assert!(Arc::ptr_eq(
            &saved.sensors["sensor"].current,
            &sensor.lock().unwrap().current
        ));
        assert!(Arc::ptr_eq(
            &saved.sensors["sensor"].latched,
            &sensor.lock().unwrap().latched
        ));
        logic.drive(true);
        position.publish(-42);
        image.use_analytic();
        stream.lock().unwrap().pixels.fill(0);
        sensor.lock().unwrap().latch(6).unwrap();
        *phases.lock().unwrap() = [None; 2];
        resources.restore_checkpoint(&saved).unwrap();
        assert_eq!(resources.fingerprint().unwrap(), before);
        assert!(!logic.high());
        assert_eq!(position.value(), 23);
        assert_eq!(stream.lock().unwrap().pixels.as_slice(), [9, 8, 7, 6]);
        assert_eq!(sensor.lock().unwrap().edge.unwrap().tick, 5);
    }
}

#[cfg(test)]
mod optical_wire_tests {
    use super::*;
    #[test]
    fn optical_position_restores_the_existing_shared_wire() {
        let position = fujimod_emulator_imaging::scene::optics::Position::default();
        let reader = position.clone();
        position.publish(-62);
        let wire = Wire::OpticalPosition(position.clone());
        let saved = wire.checkpoint().unwrap();
        assert_eq!(reader.value(), -62);
        position.publish(7);
        wire.validate_checkpoint(&saved).unwrap();
        wire.restore_checkpoint(&saved).unwrap();
        assert_eq!(reader.value(), -62);
        assert_eq!(wire.fingerprint().unwrap(), serde_json::json!(-62));
    }
}

#[cfg(test)]
mod exposure_wire_tests {
    use super::*;
    use fujimod_emulator_imaging::exposure::Exposure;
    #[test]
    fn physical_wires_restore_shared_values_and_refuse_bad_iris_before_mutation() {
        let mut resources = Resources::default();
        let exposure = Arc::new(Mutex::new(Exposure {
            seconds: 0.008,
            analog_gain: 2.,
        }));
        let aperture = Arc::new(Mutex::new(16.));
        resources
            .wires
            .insert("exposure".into(), Wire::Exposure(exposure.clone()));
        resources
            .wires
            .insert("iris".into(), Wire::Aperture(aperture.clone()));
        let initial = resources.fingerprint().unwrap();
        let saved = resources.checkpoint().unwrap();
        *exposure.lock().unwrap() = Exposure {
            seconds: 0.1,
            analog_gain: 4.,
        };
        *aperture.lock().unwrap() = 64.;
        let changed = resources.fingerprint().unwrap();
        let mut corrupt = saved.clone();
        corrupt.wires.insert(
            "iris".into(),
            Checkpoint::encode("physical-aperture", &0.0f64).unwrap(),
        );
        assert!(resources.restore_checkpoint(&corrupt).is_err());
        assert_eq!(resources.fingerprint().unwrap(), changed);
        resources.restore_checkpoint(&saved).unwrap();
        assert_eq!(
            *exposure.lock().unwrap(),
            Exposure {
                seconds: 0.008,
                analog_gain: 2.
            }
        );
        assert_eq!(*aperture.lock().unwrap(), 16.);
        assert_eq!(resources.fingerprint().unwrap(), initial);
    }
}

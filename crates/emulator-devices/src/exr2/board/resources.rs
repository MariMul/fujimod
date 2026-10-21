//! Explicit peer wires and external scene/media inputs, never CPU state.
use super::super::{clock, gpio, irq, isp, pulse};
use crate::{
    parts::{hdmi_sink::HdmiSink, lens::bridge, sd_card::Card},
    scene::{ImageFrames, SensorFramesHandle, StatisticsFrames},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

/// Prepared external inputs. Firmware/file verification belongs to the
/// frontend; these values contain bytes or physical input handles only.
#[derive(Default)]
pub struct Inputs {
    pub images: BTreeMap<String, ImageFrames>,
    pub statistics: BTreeMap<String, StatisticsFrames>,
    pub sensors: BTreeMap<String, SensorFramesHandle>,
    pub cards: BTreeMap<String, Card>,
    pub nands: BTreeMap<String, crate::parts::nand::Nand>,
    pub files: BTreeMap<String, Vec<(String, Vec<u8>)>>,
}

/// A shared wire is declared once and has one concrete type. Every consumer
/// is recorded during construction so source accesses wake its connected peers.
pub enum Wire {
    FramePeriod(crate::imaging::timing::FramePeriod),
    ImageGrid(crate::imaging::analysis::GridHandle),
    Focus(crate::imaging::optics::FocusHandle),
    Exposure(crate::imaging::exposure::ExposureHandle),
    Aperture(crate::imaging::exposure::ApertureHandle),
    SoftwarePulse(irq::SoftwarePulse),
    Parameters(isp::parameters::ParameterHandle),
    MemoryOutputs(isp::readback::MemoryOutputHandle),
    Stream(isp::stream::StreamHandle),
    OutputChannels(isp::output::OutputChannels),
    IrqRoutes(isp::dma::IrqRoutes),
    PhaseEdges(crate::scene::FramePhases),
    BridgeRegisters(bridge::Registers),
    LogicLevel(crate::signals::Level),
    OpticalPosition(crate::scene::optics::Position),
}
impl Wire {
    pub fn fingerprint(&self) -> Result<serde_json::Value, String> {
        use serde_json::json;
        Ok(match self {
            Self::FramePeriod(v) => json!(*v.lock().map_err(|_| "frame clock poisoned")?),
            Self::ImageGrid(v) => json!(*v.lock().map_err(|_| "image grid poisoned")?),
            Self::Focus(v) => json!(*v.lock().map_err(|_| "focus poisoned")?),
            Self::Exposure(v) => json!(*v.lock().map_err(|_| "exposure poisoned")?),
            Self::Aperture(v) => json!(*v.lock().map_err(|_| "aperture poisoned")?),
            Self::SoftwarePulse(v) => json!(v.asserted()),
            Self::LogicLevel(v) => json!(v.high()),
            Self::OpticalPosition(v) => json!(v.value()),
            Self::Parameters(v) => json!(*v.lock().map_err(|_| "parameters poisoned")?),
            Self::MemoryOutputs(v) => v
                .lock()
                .map_err(|_| "memory outputs poisoned")?
                .fingerprint(),
            Self::Stream(v) => json!(*v.lock().map_err(|_| "stream poisoned")?),
            Self::OutputChannels(v) => v.fingerprint(),
            Self::IrqRoutes(v) => v.fingerprint(),
            Self::PhaseEdges(v) => json!(*v.lock().map_err(|_| "phase edges poisoned")?),
            Self::BridgeRegisters(v) => json!(
                (0..4)
                    .map(|i| v.read_register(i))
                    .collect::<Result<Vec<_>, _>>()?
            ),
        })
    }

    pub fn logic_level(high: bool) -> Self {
        Self::LogicLevel(crate::signals::Level::new(high))
    }
    pub fn optical_position() -> Self {
        Self::OpticalPosition(Default::default())
    }
    pub fn software_pulse() -> Self {
        Self::SoftwarePulse(Default::default())
    }
    pub fn exposure() -> Self {
        Self::Exposure(Default::default())
    }
    pub fn focus() -> Self {
        Self::Focus(Default::default())
    }
    pub fn frame_period() -> Self {
        Self::FramePeriod(Default::default())
    }
    pub fn image_grid() -> Self {
        Self::ImageGrid(Arc::new(Mutex::new([1, 1])))
    }
    pub fn aperture() -> Self {
        Self::Aperture(Arc::new(Mutex::new(1.)))
    }
    pub fn parameters() -> Self {
        Self::Parameters(Arc::new(Mutex::new(Default::default())))
    }
    pub fn memory_outputs() -> Self {
        Self::MemoryOutputs(Arc::new(Mutex::new(Default::default())))
    }
    pub fn stream() -> Self {
        Self::Stream(Arc::new(Mutex::new(Default::default())))
    }
    pub fn output_channels() -> Self {
        Self::OutputChannels(Default::default())
    }
    pub fn irq_routes() -> Self {
        Self::IrqRoutes(Default::default())
    }
    pub fn phase_edges() -> Self {
        Self::PhaseEdges(Arc::new(Mutex::new([None; 2])))
    }
    pub fn bridge_registers() -> Self {
        Self::BridgeRegisters(Default::default())
    }
}

pub struct CardPins {
    pub gpio: usize,
    pub pads: gpio::Pads,
    pub detect: crate::parts::pins::Pin,
    pub present_high: bool,
    pub protect: crate::parts::pins::Pin,
    pub protected_high: bool,
}
#[derive(Default)]
pub struct Resources {
    pub inputs: Inputs,
    pub card_pins: BTreeMap<String, CardPins>,
    pub wires: BTreeMap<String, Wire>,
    pub(super) users: BTreeMap<String, BTreeSet<usize>>,
    pub(super) image_users: BTreeMap<String, BTreeSet<usize>>,
    pub(super) gpio: BTreeMap<String, gpio::Pads>,
    pub(super) pulses: BTreeMap<String, pulse::PulsePins>,
    pub(super) encoders: BTreeMap<String, crate::parts::encoder::EncoderInput>,
    pub(super) hdmi_sinks: BTreeMap<String, HdmiSink>,
    pub(super) controllers: BTreeMap<String, irq::Inputs>,
    pub(super) clocks: BTreeMap<String, clock::Output>,
    pub(super) software_sources: BTreeSet<String>,
}
impl Resources {
    pub fn optical_position(&self, name: &str) -> Result<crate::scene::optics::Position, String> {
        match self.wires.get(name) {
            Some(Wire::OpticalPosition(v)) => Ok(v.clone()),
            _ => Err(format!("{name} is not an optical position wire")),
        }
    }
    pub fn fingerprint(&self) -> Result<serde_json::Value, String> {
        use serde_json::json;
        let wires = self
            .wires
            .iter()
            .map(|(name, v)| Ok((name.clone(), v.fingerprint()?)))
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let sensors = self
            .inputs
            .sensors
            .iter()
            .map(|(name, v)| {
                Ok((
                    name.clone(),
                    v.lock().map_err(|_| "sensor input poisoned")?.fingerprint(),
                ))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        Ok(
            json!({"wires":wires,"users":self.users,"image_users":self.image_users,"software_sources":self.software_sources,
                "images":self.inputs.images.iter().map(|(k,v)|(k,v.fingerprint())).collect::<BTreeMap<_,_>>(),
                "statistics":self.inputs.statistics.iter().map(|(k,v)|(k,v.fingerprint())).collect::<BTreeMap<_,_>>(),
                "sensors":sensors,"cards":self.inputs.cards.iter().map(|(k,v)|(k,v.fingerprint())).collect::<BTreeMap<_,_>>(),
                "nands":self.inputs.nands.iter().map(|(k,v)|(k,v.fingerprint())).collect::<BTreeMap<_,_>>(),
                "files":self.inputs.files.iter().map(|(k,v)|(k,v.iter().map(|(path,data)|json!({"path":path,"contents":crate::fingerprint::bytes(data)})).collect::<Vec<_>>())).collect::<BTreeMap<_,_>>()
            }),
        )
    }

    pub fn logic_level(&self, name: &str) -> Result<crate::signals::Level, String> {
        match self.wires.get(name) {
            Some(Wire::LogicLevel(level)) => Ok(level.clone()),
            _ => Err(format!("{name} is not a logic-level input")),
        }
    }
    pub fn wire_users(&self, name: &str) -> Result<Vec<usize>, String> {
        self.users
            .get(name)
            .map(|users| users.iter().copied().collect())
            .ok_or_else(|| format!("{name} has no connected models"))
    }
    pub fn image_users(&self, name: &str) -> Result<Vec<usize>, String> {
        if !self.inputs.images.contains_key(name) {
            return Err(format!("no such image input {name}"));
        }
        Ok(self
            .image_users
            .get(name)
            .map(|users| users.iter().copied().collect())
            .unwrap_or_default())
    }
    pub(super) fn image_input(&mut self, name: &str, node: usize) -> Result<ImageFrames, String> {
        let image = self
            .inputs
            .images
            .get(name)
            .cloned()
            .ok_or_else(|| format!("missing external image input {name}"))?;
        self.image_users
            .entry(name.to_owned())
            .or_default()
            .insert(node);
        Ok(image)
    }
    pub fn encoder(&self, name: &str) -> Result<crate::parts::encoder::EncoderInput, String> {
        self.encoders
            .get(name)
            .cloned()
            .ok_or_else(|| format!("{name} is not an encoder input"))
    }
    pub fn bridge_registers(&self, name: &str) -> Result<bridge::Registers, String> {
        match self.wires.get(name) {
            Some(Wire::BridgeRegisters(registers)) => Ok(registers.clone()),
            _ => Err(format!("{name} is not a lens bridge register connection")),
        }
    }
    pub fn hdmi_sink(&self, name: &str) -> Result<HdmiSink, String> {
        self.hdmi_sinks
            .get(name)
            .cloned()
            .ok_or_else(|| format!("{name} is not a constructed HDMI sink input"))
    }
    pub(super) fn wire(&mut self, name: &str, node: usize) -> Result<&Wire, String> {
        let wire = self
            .wires
            .get(name)
            .ok_or_else(|| format!("unknown peer wire {name}"))?;
        self.users.entry(name.to_owned()).or_default().insert(node);
        Ok(wire)
    }
    pub fn pads(&self, name: &str) -> Result<gpio::Pads, String> {
        self.gpio
            .get(name)
            .cloned()
            .ok_or_else(|| format!("{name} is not a constructed GPIO block"))
    }
    pub fn pulses(&self, name: &str) -> Result<pulse::PulsePins, String> {
        self.pulses
            .get(name)
            .cloned()
            .ok_or_else(|| format!("{name} is not a constructed pulse unit"))
    }
    pub fn controller(&self, name: &str) -> Result<irq::Inputs, String> {
        self.controllers
            .get(name)
            .cloned()
            .ok_or_else(|| format!("{name} is not an interrupt controller"))
    }
    pub fn clock(&self, name: &str) -> Result<clock::Output, String> {
        self.clocks
            .get(name)
            .cloned()
            .ok_or_else(|| format!("{name} is not a clock controller"))
    }
}
pub struct CoreInterrupts {
    pub core: usize,
    pub device: usize,
    pub inputs: irq::Inputs,
}

pub mod checkpoint;

use super::super::{
    adc, audio, boot, clock, companion, core_release, lz,
    display::{device as scanout, formatter, timing},
    dram, gpio, gpu, hdmi, i2c, irq, isp, jpeg, dma, nand, pulse, reset, sdhc,
    sdio, spi, timer, usb,
};
use crate::{
    bus::InterruptLine,
    graph::Mapping,
    parts::{
        audio_sink, command_data_output, contact_actuator, encoder, gpio_contact_response,
        hdmi_sink, lens, sd_card,
        sensor, spi as spi_parts, stabilizer, usb_endpoint, wifi_module,
    },
    storage::fat,
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootEntry {
    Rom,
    Loader,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: u32,
    pub boot_entry: BootEntry,
    pub wires: BTreeMap<String, WireKind>,
    pub nodes: Vec<Node>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardPins {
    pub gpio: String,
    pub detect: crate::parts::pins::Pin,
    pub present_high: bool,
    pub protect: crate::parts::pins::Pin,
    pub protected_high: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WireKind {
    LogicLevel(bool),
    FramePeriod,
    ImageGrid,
    Focus,
    Exposure,
    Aperture,
    SoftwarePulse,
    Parameters,
    MemoryOutputs,
    Stream,
    OutputChannels,
    IrqRoutes,
    PhaseEdges,
    BridgeRegisters,
    OpticalPosition,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub name: String,
    pub boot_entries: Vec<BootEntry>,
    pub windows: Vec<Mapping>,
    pub dependencies: Vec<String>,
    pub model: Model,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adapter {
    pub implementation: String,
    pub options: serde_json::Value,
    pub references: BTreeMap<String, String>,
    pub wires: Vec<String>,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Card {
    Blank {
        options: sd_card::Options,
    },
    Formatted {
        options: fat::Options,
        files: String,
    },
    Retained {
        input: String,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sensor {
    pub options: sensor::Options,
    pub frames: String,
    pub controls: Option<SensorControls>,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SensorControls {
    Xtrans16 {
        options: crate::parts::xtrans16::Options,
        calibration: crate::imaging::exposure::Calibration,
        exposure: String,
        aperture: String,
        projection: Option<crate::imaging::optics::Spec>,
        frame_clock: Option<crate::imaging::timing::ClockOutput>,
    },
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SerialEndpoint {
    #[serde(rename = "lens-unit")]
    LensUnit {
        options: Box<lens::Options>,
        pulses: String,
        gpio: String,
        completion: [InterruptLine; 2],
        position: Option<String>,
        optics: Option<Iris>,
        focus: Option<FocusAxis>,
    },
    Stabilizer {
        options: stabilizer::Options,
    },
    Registers {
        options: spi_parts::RegisterOptions,
    },
    Panel {
        options: spi_parts::PanelOptions,
    },
    CommandDataOutput {
        options: command_data_output::Options,
    },
    Fixed {
        options: spi_parts::FixedOptions,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Iris {
    pub output: String,
    pub positions: Vec<(i64, f64)>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FocusAxis {
    pub output: String,
    pub infinity_position: i64,
    pub steps_per_mm: f64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SerialRoute {
    pub options: spi::channel::Options,
    pub gpio: String,
    pub endpoint: SerialEndpoint,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoftwareOutput {
    pub line: u32,
    pub pulse: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputSignal {
    pub gpio: String,
    pub selection: crate::parts::pins::Selection,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Model {
    Power {
        options: super::serial::Options,
        gpio: String,
    },
    Lz {
        options: lz::Options,
        base: u64,
        controllers: Vec<String>,
    },
    MemoryDma {
        options: dma::memory::Options,
        base: u64,
        routes: String,
    },
    SdhciClocks {
        words: Vec<(u64, u64)>,
    },
    Usb {
        options: usb::Options,
        cable: String,
        endpoints: Option<usb_endpoint::Wiring>,
    },
    Scanout {
        options: scanout::Options,
    },
    SerialAudioTransmitter {
        options: audio::transmit::Options,
        sinks: [crate::parts::audio_sink::Options; 2],
        routes: String,
    },
    SerialDmaLink {
        options: super::serial_dma::Options,
        input: super::serial_dma::Input,
    },
    XMountLens {
        transport: super::serial_dma::Options,
        configuration: crate::parts::x_mount_link::Configuration,
        startup_ticks: u64,
        pins: super::super::x_mount_connector::Options,
        gpio: String,
        controllers: Vec<String>,
    },
    SerialDma {
        options: dma::audio::Options,
    },
    ImageCombination {
        options: isp::combination::Options,
    },
    I2c {
        options: i2c::Options,
        controllers: Vec<String>,
        cable: Option<String>,
        bridge: Option<String>,
    },
    VectorGpu {
        options: gpu::Options,
        controllers: Vec<String>,
    },
    Nand {
        options: nand::Options,
        medium: String,
    },
    ExternalFrames {
        options: crate::scene::clocked::Options,
        sensor_clock: Option<String>,
        images: String,
        statistics: String,
        activity: Option<InputSignal>,
        position: Option<String>,
        radiometric: Option<crate::imaging::radiometric::Spec>,
        phase_edges: Option<String>,
        frame_period: Option<String>,
    },
    DisplayTiming {
        options: timing::Options,
        clock: String,
        controllers: Vec<String>,
    },
    Dram {
        options: dram::Options,
    },
    DramPhy {
        options: dram::Options,
    },
    Reset {
        options: reset::Options,
    },
    #[serde(rename = "block-98000")]
    Block98000 {
        options: boot::Options,
        wiring: boot::Wiring,
    },
    Clocks {
        options: clock::Options,
    },
    #[serde(rename = "exr2-interrupt-controller")]
    InterruptController {
        options: irq::Options,
        core: usize,
        base: u64,
        routes: Option<String>,
        software: Vec<SoftwareOutput>,
    },
    CoreRelease {
        options: core_release::Options,
        core: usize,
        request: String,
    },
    #[serde(rename = "exr2-system-timers")]
    Timers {
        options: timer::Options,
        controllers: Vec<String>,
    },
    Adc {
        options: adc::controller::Options,
    },
    Gpio {
        options: gpio::Options,
        irqs: Vec<gpio::InputIrq>,
    },
    ContactActuator {
        options: contact_actuator::Options,
        gpio: String,
    },
    #[serde(rename = "exr2-pulse")]
    PulseUnit {
        options: pulse::Options,
        irqs: [Option<InterruptLine>; 12],
    },
    SerialChannel {
        options: spi::channel::Options,
        gpio: String,
        endpoint: SerialEndpoint,
    },
    SpiController {
        routes: Vec<SerialRoute>,
    },
    GpioContactResponse {
        options: gpio_contact_response::Options,
        gpio: String,
    },
    Encoder {
        options: encoder::Options,
        gpio: String,
        counter: Option<String>,
    },
    #[serde(rename = "i2c-motor-bridge")]
    I2cMotorBridge {
        options: lens::bridge::Options,
        registers: String,
        pulses: String,
        gpio: String,
        clock_irq: InterruptLine,
    },
    Block86000 {
        options: pulse::block86000::Options,
    },
    Companion {
        options: companion::Options,
    },
    Fe80Processing {
        transport: companion::Options,
        options: super::super::companion_processing::Options,
        frames: String,
        routes: String,
    },
    Formatter {
        options: formatter::Options,
    },
    Hdmi {
        options: hdmi::transmitter::Options,
        sink: String,
    },
    HdmiSink {
        options: hdmi_sink::Options,
    },
    SerialAudio {
        options: audio::Options,
        sinks: [audio_sink::Options; 2],
    },
    SdHost {
        options: sdhc::Options,
        card: Card,
        pins: Option<CardPins>,
        present: bool,
        protected: bool,
        irq: InterruptLine,
    },
    SdioHost {
        options: sdio::Options,
        module: wifi_module::Options,
        irq: InterruptLine,
    },
    UsbEndpoints {
        options: usb_endpoint::Options,
        irq: InterruptLine,
        dma_irq: InterruptLine,
        ram_bytes: u64,
    },
    ImageInput {
        options: isp::input::Options,
        phase_irqs: [isp::Irq; 2],
        raw_irq: isp::Irq,
        readback_irq: isp::Irq,
        shutter_output: Option<usize>,
        sensor: Option<Sensor>,
        parameters: Option<String>,
        memory_outputs: Option<String>,
        statistics_grid: Option<String>,
        frame_period: Option<String>,
        edges: String,
    },
    OutputChannel {
        options: isp::output::Options,
        base: u64,
        irq: u32,
        channel: usize,
        channels: String,
        routes: String,
        frames: String,
        memory_outputs: Option<String>,
    },
    Statistics {
        options: isp::producer::Options,
        routes: String,
        frames: String,
        images: String,
        grid: Option<String>,
    },
    PhaseDetection {
        options: isp::phase_detection::Options,
        routes: String,
    },
    WindowAnalysis {
        options: isp::window_analysis::Options,
        routes: String,
    },
    Parameters {
        options: isp::parameters::Options,
        bank: String,
    },
    RawConversion {
        options: isp::raw::RegisterOptions,
    },
    Prefilter {
        options: isp::raw::Options,
        irq: isp::Irq,
    },
    RawStatistics {
        options: isp::raw::Options,
        irq: isp::Irq,
    },
    StillProcessor {
        options: isp::still::Options,
        irq: isp::Irq,
    },
    Scaler {
        options: isp::scaler::Options,
        irq: isp::Irq,
    },
    Warp {
        options: isp::warp::Options,
        irq: isp::Irq,
        stream: String,
    },
    VideoEncoder {
        options: isp::video::Options,
        irq: isp::Irq,
    },
    JpegEncoder {
        options: jpeg::encoder::Options,
        irq: isp::Irq,
        stream: String,
    },
    JpegDecoder {
        options: jpeg::decoder::Options,
        irq: isp::Irq,
    },
    Block11000 {
        options: isp::block11000::Options,
    },
    FaceDetector {
        options: isp::face_detector::Options,
    },
    FaceResults {
        profile: isp::face_results::Profile,
        routes: String,
    },
    Adapter {
        adapter: Adapter,
    },
}
impl Model {
    pub(super) fn node_references(&self) -> Vec<&str> {
        let mut refs = vec![];
        match self {
            Self::Power { gpio, .. } | Self::ContactActuator { gpio, .. } => {
                refs.push(gpio.as_str())
            }
            Self::SdHost {
                pins: Some(pins), ..
            } => refs.push(pins.gpio.as_str()),
            Self::ExternalFrames {
                activity: Some(signal),
                ..
            } => refs.push(signal.gpio.as_str()),
            Self::DisplayTiming {
                clock, controllers, ..
            } => {
                refs.push(clock.as_str());
                refs.extend(controllers.iter().map(String::as_str));
            }
            Self::Timers { controllers, .. }
            | Self::Lz { controllers, .. }
            | Self::VectorGpu { controllers, .. }
            | Self::I2c { controllers, .. } => {
                refs.extend(controllers.iter().map(String::as_str));
            }
            Self::SerialChannel { gpio, endpoint, .. } => {
                refs.push(gpio.as_str());
                if let SerialEndpoint::LensUnit { pulses, gpio, .. } = endpoint {
                    refs.extend([pulses.as_str(), gpio.as_str()]);
                }
            }
            Self::SpiController { routes } => {
                for route in routes {
                    refs.push(route.gpio.as_str());
                    if let SerialEndpoint::LensUnit { pulses, gpio, .. } = &route.endpoint {
                        refs.extend([pulses.as_str(), gpio.as_str()]);
                    }
                }
            }
            Self::XMountLens {
                gpio, controllers, ..
            } => {
                refs.push(gpio.as_str());
                refs.extend(controllers.iter().map(String::as_str));
            }
            Self::GpioContactResponse { gpio, .. } => refs.push(gpio.as_str()),
            Self::Encoder { gpio, counter, .. } => {
                refs.push(gpio.as_str());
                refs.extend(counter.as_deref());
            }
            Self::I2cMotorBridge { pulses, gpio, .. } => {
                refs.extend([pulses.as_str(), gpio.as_str()])
            }
            Self::Hdmi { sink, .. } => refs.push(sink.as_str()),
            Self::Adapter { adapter } => {
                refs.extend(adapter.references.values().map(String::as_str))
            }
            _ => (),
        }
        refs
    }
}

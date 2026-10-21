//! Typed identity-based hardware and firmware reference records.
//!
//! The records contain device facts and named references. They never contain
//! another record's filename or an input asset's location.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const SOC_SCHEMA_URI: &str = "https://fujimod.com/schemas/soc.schema.json";
pub const PART_SCHEMA_URI: &str = "https://fujimod.com/schemas/part.schema.json";
pub const BODY_SCHEMA_URI: &str = "https://fujimod.com/schemas/body.schema.json";
pub const FIRMWARE_SCHEMA_URI: &str = "https://fujimod.com/schemas/firmware.schema.json";

/// A processor architecture described by a system-on-chip record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProcessorArchitecture {
    /// 32-bit ARM processor architecture.
    Arm32,
    /// 64-bit ARM processor architecture.
    Aarch64,
}

/// A named system-on-chip record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct Soc {
    /// Stable schema URI for this SoC record.
    #[serde(rename = "$schema")]
    pub schema_uri: String,
    /// SoC record version.
    pub schema: u32,
    /// Stable SoC identity.
    pub id: String,
    /// CPU execution architecture implemented by the SoC.
    pub architecture: ProcessorArchitecture,
    /// Processor cores and their hardware reset configuration.
    pub processors: Vec<ProcessorCore>,
    /// Addressable memory regions owned by this SoC.
    pub memory_regions: Vec<MemoryRegion>,
    /// Address and register blocks exposed by the SoC.
    pub blocks: Vec<SocBlock>,
    /// Interrupt routes between SoC blocks and processor cores.
    pub interrupts: Vec<InterruptRoute>,
    /// Clock domains exposed by the SoC.
    pub clocks: Vec<ClockDomain>,
    /// CPU address-space mappings used to construct a processor graph.
    pub mappings: Vec<AddressMapping>,
    /// DMA address-space mappings used to construct a processor graph.
    pub dma_mappings: Vec<DmaMapping>,
    /// Named hardware-decoder states and supported selections.
    pub decoders: Vec<AddressDecoder>,
    /// Per-core peripheral mappings into named SoC blocks.
    pub peripherals: Vec<PeripheralMapping>,
    /// Hardware reset and coprocessor setup facts.
    pub reset: SocReset,
}

impl Soc {
    /// Validate the record's schema identity and internal references.
    pub fn validate(&self, selected_id: &str) -> Result<(), String> {
        if self.schema != 1 || self.schema_uri != SOC_SCHEMA_URI || self.id != selected_id {
            return Err("SoC schema or identity differs".into());
        }
        validate_id(&self.id)?;
        let mut processors = BTreeSet::new();
        for core in &self.processors {
            validate_id(&core.id)?;
            if !processors.insert(core.id.as_str()) {
                return Err(format!("SoC {} repeats processor {}", self.id, core.id));
            }
        }
        let mut blocks = BTreeSet::new();
        for block in &self.blocks {
            validate_id(&block.id)?;
            if !blocks.insert(block.id.as_str()) {
                return Err(format!("SoC {} repeats block {}", self.id, block.id));
            }
        }
        for mapping in &self.peripherals {
            if !blocks.contains(mapping.block_id.as_str()) {
                return Err(format!(
                    "SoC {} refers to missing block {}",
                    self.id, mapping.block_id
                ));
            }
        }
        for clock in &self.clocks {
            validate_id(&clock.id)?;
            if let Some(parent) = &clock.parent_id
                && !self.clocks.iter().any(|candidate| candidate.id == *parent)
            {
                return Err(format!("SoC {} refers to missing clock {parent}", self.id));
            }
        }
        Ok(())
    }
}

/// A processor core and its hardware reset state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ProcessorCore {
    /// Core identity within its SoC.
    pub id: String,
    /// Implemented processor model.
    pub model: ProcessorModel,
    /// Whether this core leaves reset when the SoC starts.
    pub released: bool,
    /// Initial architectural register values.
    pub registers: Vec<RegisterValue>,
    /// Optional M-profile hardware state.
    pub mprofile: Option<MProfileState>,
    /// Optional R-profile hardware state.
    pub rprofile: Option<RProfileState>,
}

/// A processor core model supported by the current catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProcessorModel {
    /// Cortex-R5F family processor.
    CortexR5f,
    /// Cortex-A7 family processor.
    CortexA7,
    /// Cortex-A53 family processor.
    CortexA53,
}

/// An initial value for an architectural register.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RegisterValue {
    /// Register selected for initialization.
    pub register: ProcessorRegister,
    /// Initial register value.
    pub value: u64,
}

/// Architectural registers explicitly initialized by a catalog record.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProcessorRegister {
    /// Program counter.
    Pc,
    /// Stack pointer.
    Sp,
    /// Program status register.
    Pstate,
    /// Current program status register.
    Cpsr,
}

/// M-profile processor identification state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct MProfileState {
    /// CPUID register value presented by the processor.
    pub cpuid: u32,
    /// Vector-table address after reset.
    pub vector_base: u64,
}

/// R-profile processor identification and TCM state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RProfileState {
    /// CP14 processor identity register value.
    pub cp14_identity: u32,
    /// ATCM state after reset.
    pub atcm: TcmState,
    /// BTCM state after reset.
    pub btcm: TcmState,
}

/// A tightly coupled memory reset state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct TcmState {
    /// TCM capacity in bytes.
    pub bytes: u64,
    /// Address selected by the processor at reset.
    pub reset_address: u64,
}

/// A memory region provided by a SoC.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct MemoryRegion {
    /// Region identity within the SoC.
    pub id: String,
    /// Region's physical base address.
    pub base: u64,
    /// Region capacity or a reference to body DRAM capacity.
    pub size: RegionSize,
    /// Hardware memory type.
    pub kind: MemoryKind,
}

/// A memory extent with no repeated body-specific capacity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub enum RegionSize {
    /// A fixed byte length owned by the SoC.
    Bytes { bytes: u64 },
    /// The selected body's declared DRAM capacity.
    BodyDram,
}

/// Memory technologies exposed by the SoC.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    /// Dynamic random-access memory.
    Dram,
    /// Read-only boot memory.
    Rom,
    /// Static random-access memory.
    Sram,
    /// Tightly coupled processor memory.
    Tcm,
}

/// A named register or address block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SocBlock {
    /// Block identity used by bodies and run inputs.
    pub id: String,
    /// Physical base address of the block.
    pub base: u64,
    /// Addressable block size in bytes.
    pub size: u64,
    /// Block's modeled hardware category.
    pub kind: SocBlockKind,
    /// Clock domain supplying this block, when specified.
    pub clock_id: Option<String>,
    /// Interrupt lines emitted by this block.
    pub interrupt_lines: Vec<u32>,
}

/// Hardware block categories represented in the catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SocBlockKind {
    /// General-purpose input/output controller.
    Gpio,
    /// Analog-to-digital converter.
    Adc,
    /// Interrupt controller.
    InterruptController,
    /// Timer controller.
    Timer,
    /// Direct-memory-access controller.
    Dma,
    /// Serial control-bus controller.
    ControlBus,
    /// General-purpose serial controller.
    SerialController,
    /// Nonvolatile memory controller.
    NandController,
    /// USB controller.
    UsbController,
    /// Display controller.
    DisplayController,
    /// Imaging controller.
    ImagingController,
    /// System reset controller.
    ResetController,
    /// Clock controller.
    ClockController,
}

/// A routed interrupt line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct InterruptRoute {
    /// Processor core receiving the interrupt.
    pub core: String,
    /// Interrupt line number on the processor core.
    pub line: u32,
    /// SoC block producing the interrupt.
    pub source_block: String,
}

/// A SoC clock domain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ClockDomain {
    /// Clock-domain identity.
    pub id: String,
    /// Clock frequency in hertz, when known.
    pub frequency_hz: Option<u64>,
    /// Parent clock-domain identity, when present.
    pub parent_id: Option<String>,
}

/// A processor address-space mapping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct AddressMapping {
    /// Processor core using this mapping.
    pub core: String,
    /// Address visible to the processor.
    pub address: u64,
    /// Physical target address.
    pub physical: u64,
    /// Mapping extent, including a body DRAM reference when applicable.
    pub size: RegionSize,
    /// Read, write and execute permission bits.
    pub permissions: u8,
}

/// A DMA address-space mapping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct DmaMapping {
    /// DMA-visible address.
    pub address: u64,
    /// Physical target address.
    pub physical: u64,
    /// Mapping extent, including a body DRAM reference when applicable.
    pub size: RegionSize,
    /// Read, write and execute permission bits.
    pub permissions: u8,
}

/// A selectable SoC address decoder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct AddressDecoder {
    /// Decoder identity.
    pub id: String,
    /// Initially selected target IDs.
    pub initial: DecoderSelection,
    /// Other supported named decoder states.
    pub choices: Vec<DecoderChoice>,
}

/// The processor and DMA routes selected by a decoder state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct DecoderSelection {
    /// Processor cores selected by this state.
    pub processors: Vec<String>,
    /// DMA mapping identities selected by this state.
    pub dma_mappings: Vec<String>,
}

/// A named alternate decoder state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct DecoderChoice {
    /// Choice identity selected by a hardware strap or control.
    pub id: String,
    /// Routes enabled by this choice.
    pub selection: DecoderSelection,
}

/// A per-core view into a named SoC block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct PeripheralMapping {
    /// Processor core using this peripheral window.
    pub core: String,
    /// SoC block identity exposed through this window.
    pub block_id: String,
    /// Processor-visible address.
    pub address: u64,
    /// Physical target address.
    pub physical: u64,
    /// Window size in bytes.
    pub size: u64,
}

/// Hardware state applied when a SoC leaves reset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SocReset {
    /// Optional secondary-core release register.
    pub release: Option<CoreRelease>,
    /// Coprocessor register writes applied during reset.
    pub coprocessor_writes: Vec<CoprocessorWrite>,
}

/// A secondary-core release condition.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct CoreRelease {
    /// Release register address.
    pub register: u64,
    /// Register bit set when the core is released.
    pub mask: u32,
    /// Interrupt or reset source used to release the core.
    pub source: u32,
    /// Reset vector address selected on release.
    pub target: u64,
}

/// A CP15 or CP14 coprocessor register write during reset.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct CoprocessorWrite {
    /// Processor core receiving the write.
    pub core: String,
    /// Coprocessor selector.
    pub cp: u8,
    /// Register width selector.
    pub wide: bool,
    /// Secure-state selector.
    pub secure: u8,
    /// CRn selector.
    pub crn: u8,
    /// CRm selector.
    pub crm: u8,
    /// Opcode selector 1.
    pub opc1: u8,
    /// Opcode selector 2.
    pub opc2: u8,
    /// Bits set in the selected register.
    pub set_bits: u64,
}

/// A named camera body and its physical components.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct Body {
    /// Stable schema URI for this body record.
    #[serde(rename = "$schema")]
    pub schema_uri: String,
    /// Body record version.
    pub schema: u32,
    /// Stable body identity.
    pub id: String,
    /// SoC identity implemented by this body.
    pub soc_id: String,
    /// Installed DRAM capacity in bytes.
    pub dram_bytes: u64,
    /// Optional installed NAND geometry.
    pub nand: Option<NandGeometry>,
    /// Optional sensor assembly and its physical characteristics.
    pub sensor: Option<SensorAssembly>,
    /// Optional lens assembly and its physical characteristics.
    pub lens: Option<LensAssembly>,
    /// Optional display assembly and its physical characteristics.
    pub display: Option<DisplayAssembly>,
    /// Optional MCU endpoint and its body-specific serial wiring.
    pub microcontroller: Option<MicrocontrollerAssembly>,
    /// Optional power-controller endpoint and its body-specific wiring.
    pub power_control: Option<PowerControlAssembly>,
    /// Optional removable-card slot and its body-specific contacts.
    pub card_slot: Option<CardSlotAssembly>,
    /// Physical control-bus devices installed in this body.
    pub control_bus_chips: Vec<ControlBusChip>,
    /// Declared physical control and peripheral connections.
    pub connections: Vec<BodyConnection>,
    /// Reusable part identities installed in this body.
    pub parts: Vec<String>,
    /// Physical links between named body connections and SoC blocks or parts.
    pub wiring: Vec<BodyWiring>,
}

impl Body {
    /// Validate the record's schema identity and component references.
    pub fn validate(&self, selected_id: &str) -> Result<(), String> {
        if self.schema != 1 || self.schema_uri != BODY_SCHEMA_URI || self.id != selected_id {
            return Err("body schema or identity differs".into());
        }
        validate_id(&self.id)?;
        validate_id(&self.soc_id)?;
        if self.dram_bytes == 0 {
            return Err(format!("body {} has no DRAM capacity", self.id));
        }
        unique_ids(self.parts.iter().map(String::as_str), "body part")?;
        unique_ids(
            self.connections.iter().map(|item| item.id.as_str()),
            "body connection",
        )?;
        for connection_id in [
            self.sensor.as_ref().map(|item| item.connection_id.as_str()),
            self.lens.as_ref().map(|item| item.connection_id.as_str()),
            self.display
                .as_ref()
                .map(|item| item.connection_id.as_str()),
            self.microcontroller
                .as_ref()
                .map(|item| item.connection_id.as_str()),
            self.power_control
                .as_ref()
                .map(|item| item.connection_id.as_str()),
            self.card_slot
                .as_ref()
                .map(|item| item.connection_id.as_str()),
        ]
        .into_iter()
        .flatten()
        {
            if !self.connections.iter().any(|item| item.id == connection_id) {
                return Err(format!(
                    "body {} component refers to missing connection {connection_id}",
                    self.id
                ));
            }
        }
        for link in &self.wiring {
            validate_id(&link.source_id)?;
            validate_id(&link.target_id)?;
        }
        Ok(())
    }
}

/// NAND geometry installed in a camera body or storage part.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct NandGeometry {
    /// Total main-area capacity in bytes.
    pub bytes: u64,
    /// Main-area page size in bytes.
    pub page_bytes: u32,
    /// Pages per erase block.
    pub pages_per_block: u32,
    /// Spare-area size per page in bytes.
    pub spare_bytes: u32,
}

/// A camera sensor assembly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SensorAssembly {
    /// Reusable sensor part identity.
    pub part_id: String,
    /// Body connection identity carrying sensor control and data signals.
    pub connection_id: String,
    /// SoC control-bus block identity wired to the sensor.
    pub controller_id: String,
    /// Control-bus channel connected to the sensor.
    pub channel: u8,
    /// Sensor's device-specific control protocol.
    pub control: SensorControlInterface,
}

/// Sensor image and timing geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SensorRaster {
    /// Sensor output width in pixels.
    pub width: u32,
    /// Sensor output height in pixels.
    pub height: u32,
    /// Active image area as `[x, y, width, height]`.
    pub image_area: [u32; 4],
    /// Sensor clock frequency in hertz when known.
    pub pixel_clock_hz: Option<u64>,
}

/// A sensor's physical control-bus connection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SensorControlInterface {
    /// Seven-bit control-bus address.
    pub address: u8,
    /// Register-address width in bits.
    pub register_address_bits: u8,
    /// Data width in bits.
    pub data_bits: u8,
}

/// A body lens assembly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct LensAssembly {
    /// Reusable lens part identity.
    pub part_id: String,
    /// Body connection identity carrying lens control signals.
    pub connection_id: String,
    /// Body-specific pulse-controller wiring for this lens.
    pub pulse: PulseInterface,
}

/// Aperture positions and their optical output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct IrisOptics {
    /// Named optical quantity produced by the aperture.
    pub output: OpticalQuantity,
    /// Aperture positions and their f-number values.
    pub positions: Vec<AperturePosition>,
    /// Minimum and maximum mechanical positions.
    pub travel_limits: [i32; 2],
}

/// An aperture mechanism position and f-number.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct AperturePosition {
    /// Mechanical position value.
    pub position: i32,
    /// Optical f-number at this position.
    pub f_number: f64,
}

/// Named optical quantities exposed by a lens assembly.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OpticalQuantity {
    /// Lens focus position.
    LensFocus,
    /// Lens aperture value.
    LensAperture,
}

/// A focus-axis model.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct FocusAxis {
    /// Named optical quantity produced by focus motion.
    pub output: OpticalQuantity,
    /// Focus position representing infinity.
    pub infinity_position: i32,
    /// Focus steps per millimeter.
    pub steps_per_mm: f64,
}

/// A body display assembly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct DisplayAssembly {
    /// Reusable panel or display-controller part identity.
    pub part_id: String,
    /// Body connection identity carrying display data.
    pub connection_id: String,
    /// Body-specific serial wiring for this panel.
    pub route: Option<SerialRoute>,
}

/// A body-specific attachment for a microcontroller part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct MicrocontrollerAssembly {
    /// Reusable microcontroller part identity.
    pub part_id: String,
    /// Body connection identity carrying MCU signals.
    pub connection_id: String,
    /// SoC serial controller block identity.
    pub controller_id: String,
    /// Channel selected on the controller.
    pub channel: u8,
    /// Body connection identity controlling chip select.
    pub select_connection_id: String,
}

/// A body-specific attachment for a power-control part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct PowerControlAssembly {
    /// Reusable power-control part identity.
    pub part_id: String,
    /// Body connection identity carrying power-control signals.
    pub connection_id: String,
    /// SoC serial controller block identity.
    pub controller_id: String,
    /// SoC ADC block identity wired to the controller.
    pub adc_block_id: String,
    /// ADC channel wired to the controller.
    pub adc_channel: u8,
}

/// A body-specific attachment for a removable-card slot part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct CardSlotAssembly {
    /// Reusable card-slot part identity.
    pub part_id: String,
    /// Body connection identity for the card data interface.
    pub connection_id: String,
    /// Body connection identity carrying card-detect state.
    pub detect_connection_id: String,
    /// Body connection identity carrying write-protect state.
    pub write_protect_connection_id: String,
}

/// Display orientation relative to the camera body.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DisplayOrientation {
    /// Fixed landscape panel.
    Landscape,
    /// Fixed portrait panel.
    Portrait,
    /// Panel that can tilt relative to the body.
    Tilting,
}

/// A physical body connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct BodyConnection {
    /// Connection identity referenced by body wiring and run inputs.
    pub id: String,
    /// Physical connection category.
    pub kind: ConnectionKind,
    /// Electrical or physical signals carried by the connection.
    pub signals: Vec<ConnectionSignal>,
}

/// Physical connection categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ConnectionKind {
    /// User button or control switch.
    Control,
    /// USB cable or host connection.
    Usb,
    /// HDMI cable or display connection.
    Hdmi,
    /// SD card slot.
    SdCard,
    /// Wired remote or microphone connector.
    Remote,
    /// Hot-shoe accessory connector.
    HotShoe,
    /// External power connector.
    ExternalPower,
    /// Battery-door interlock.
    BatteryDoor,
    /// Sensor data or control connection.
    Sensor,
    /// Lens or actuator connection.
    Lens,
    /// Display data or control connection.
    Display,
    /// Microcontroller connection.
    Microcontroller,
}

/// One electrical signal exposed by a body connection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub enum ConnectionSignal {
    /// A GPIO pin with an active logic level.
    Gpio {
        port: u8,
        pin: u8,
        active_level: bool,
    },
    /// An ADC channel and threshold.
    Adc { channel: u8, threshold: u16 },
    /// A named SoC block signal.
    Block {
        block_id: String,
        signal: BlockSignal,
    },
    /// A routed serial control channel.
    Serial {
        block_id: String,
        channel: u8,
        address: u8,
    },
}

/// Named control signals carried by SoC blocks.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlockSignal {
    /// Presence or cable-detect signal.
    Detect,
    /// Write-protect signal.
    WriteProtect,
    /// Data ready signal.
    DataReady,
    /// Clock output.
    Clock,
}

/// A physical wire between a body connection, part, or SoC block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct BodyWiring {
    /// Source component identity.
    pub source_id: String,
    /// Target component identity.
    pub target_id: String,
    /// Named signal carried by the wire.
    pub signal: WireSignal,
}

/// Named physical wire types.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WireSignal {
    /// GPIO line.
    Gpio,
    /// ADC input.
    Adc,
    /// Serial control data.
    SerialData,
    /// Serial control clock.
    SerialClock,
    /// Sensor image data.
    ImageData,
    /// Display output data.
    DisplayData,
    /// MCU command and response.
    Microcontroller,
    /// Memory card data and control.
    Storage,
    /// Power or reset state.
    Power,
}

/// A physical control-bus device installed in a camera body.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ControlBusChip {
    /// Control-bus channel index.
    pub channel: u8,
    /// Seven-bit device address.
    pub address: u8,
    /// Polarity selector for this device.
    pub polarity_selector: u8,
}

/// A reusable physical or protocol component referenced by a body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct Part {
    /// Stable schema URI for this part record.
    #[serde(rename = "$schema")]
    pub schema_uri: String,
    /// Part record version.
    pub schema: u32,
    /// Stable reusable part identity.
    pub id: String,
    /// Typed physical or protocol model for this part.
    pub model: PartModel,
}

impl Part {
    /// Validate the record's schema identity.
    pub fn validate(&self, selected_id: &str) -> Result<(), String> {
        if self.schema != 1 || self.schema_uri != PART_SCHEMA_URI || self.id != selected_id {
            return Err("part schema or identity differs".into());
        }
        validate_id(&self.id)
    }
}

/// A closed set of reusable physical part models.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub enum PartModel {
    /// A serially controlled optical lens mechanism.
    LensMechanism {
        /// Lens protocol and electrical timing.
        protocol: LensProtocol,
        /// Lens motors and their travel/response mechanics.
        motors: Vec<LensMotor>,
        /// Optional physical encoder input.
        encoder: Option<PartEncoder>,
        /// Optional zoom mechanism.
        zoom: Option<ZoomMechanism>,
    },
    /// A multi-pin contact actuator.
    ContactActuator {
        /// GPIO pins that drive the actuator.
        drive_pins: Vec<Pin>,
        /// GPIO pins sensed as contacts.
        contact_pins: Vec<Pin>,
        /// Supported drive values and resulting contact state.
        positions: Vec<ContactPosition>,
        /// Time required for a contact transition to settle.
        settle_ticks: u64,
    },
    /// A serial output endpoint.
    SerialOutput {
        /// Serial output endpoint category.
        endpoint: SerialOutputKind,
    },
    /// A microcontroller packet endpoint.
    Microcontroller {
        /// Register address range used by the endpoint.
        registers: RegisterRange,
    },
    /// A power-control endpoint with named startup inputs.
    PowerControl {
        /// SPI status registers exposed by the power controller.
        status_registers: Vec<StatusRegister>,
    },
    /// An SD card slot and its physical contacts.
    SdCardSlot,
    /// An image sensor protocol and pixel-array component.
    ImageSensor {
        /// Sensor raster geometry.
        raster: SensorRaster,
        /// Sensor register and control-bus interface.
        control: SensorControlInterface,
        /// Color-filter array pattern.
        color_filter: Vec<u8>,
        /// Black-level value.
        black_level: u32,
        /// Full-scale value.
        full_scale: u32,
        /// Per-channel reference-white values.
        reference_white: Vec<u32>,
    },
    /// A display panel endpoint.
    DisplayPanel {
        /// Display raster width in pixels.
        width: u32,
        /// Display raster height in pixels.
        height: u32,
        /// Display orientation relative to the body.
        orientation: DisplayOrientation,
        /// Display endpoint serial protocol.
        serial: SerialOutputKind,
        /// Number of panel register bits used by the endpoint.
        used_mask: u32,
    },
}

/// A supported lens serial protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LensProtocol {
    /// Banked lens protocol.
    Banked,
    /// Three-byte lens protocol.
    ThreeByte,
    /// Mixed-width lens protocol.
    MixedWidth,
    /// Direct-current zoom lens protocol.
    DcZoom,
}

/// A pulse interface required by an optical mechanism.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct PulseInterface {
    /// Counter channels sampled by the mechanism.
    pub counter_channels: Vec<u8>,
    /// Timer channels sampled by the mechanism.
    pub timer_channels: Vec<u8>,
    /// Whether quadrature signals are active.
    pub quadrature: bool,
    /// Initial pulse phase.
    pub initial_phase: u32,
}

/// One lens motor and its physical response limits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct LensMotor {
    /// Motor's initial position.
    pub initial_position: i32,
    /// Minimum and maximum travel positions.
    pub travel_limits: [i32; 2],
    /// Guest ticks per motor step.
    pub ticks_per_step: u64,
    /// Settling time after motion stops.
    pub settle_ticks: u64,
    /// Optional home-switch pin and threshold.
    pub home: Option<HomeSwitch>,
}

/// A lens home-switch input.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct HomeSwitch {
    /// GPIO pin used by the home switch.
    pub pin: Pin,
    /// Position threshold used to detect home.
    pub threshold: i32,
    /// GPIO level indicating the home position.
    pub active_high: bool,
}

/// A two-dimensional GPIO pin identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct Pin {
    /// GPIO port number.
    pub port: u8,
    /// Pin number within the GPIO port.
    pub pin: u8,
}

/// An optional encoder attached to a physical part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct PartEncoder {
    /// Encoder GPIO input pins.
    pub pins: Vec<Pin>,
    /// Guest ticks per encoder phase transition.
    pub phase_ticks: u64,
    /// Initial encoder phase.
    pub rest_phase: u8,
}

/// A zoom mechanism's travel and pulse settings.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ZoomMechanism {
    /// Minimum zoom position.
    pub minimum_position: i32,
    /// Maximum zoom position.
    pub maximum_position: i32,
    /// Guest ticks per zoom pulse.
    pub pulse_ticks: u64,
}

/// A pin drive position and its resulting contact states.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ContactPosition {
    /// Drive value applied to the actuator.
    pub drive: u32,
    /// Resulting contact states.
    pub contacts: Vec<bool>,
}

/// A serial bus route to a physical endpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SerialRoute {
    /// SoC serial controller block identity.
    pub controller_id: String,
    /// Controller channel identity.
    pub channel: u8,
    /// Endpoint identity on the serial bus.
    pub endpoint_id: String,
    /// Chip-select GPIO pin.
    pub select_pin: Pin,
    /// Active logic level for chip select.
    pub select_active_high: bool,
    /// Serial clock frequency in hertz, when known.
    pub clock_hz: Option<u64>,
    /// Word width in bits.
    pub word_bits: u8,
}

/// Serial output endpoint categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SerialOutputKind {
    /// Display panel register endpoint.
    Panel,
    /// Control-bus register endpoint.
    Registers,
    /// Command and data serial endpoint.
    CommandData,
    /// Fixed serial register endpoint.
    Fixed,
}

/// A range of registers exposed by a physical component.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RegisterRange {
    /// First register address.
    pub first: u32,
    /// Number of consecutive registers.
    pub count: u32,
    /// Register width in bytes.
    pub width_bytes: u8,
}

/// An SPI status register initialized by a physical control part.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct StatusRegister {
    /// Register address.
    pub address: u8,
    /// Initial status byte.
    pub value: u8,
}

/// A firmware content identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ContentIdentity {
    /// Content size in bytes.
    pub size: u64,
    /// Lowercase SHA-256 digest.
    pub sha256: String,
}

impl ContentIdentity {
    /// Validate content size and lowercase SHA-256 syntax.
    pub fn validate(&self) -> Result<(), String> {
        if self.size == 0
            || self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("content identity requires a positive size and lowercase SHA-256".into());
        }
        Ok(())
    }
}

/// A named firmware version and the content identities needed to run it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct Firmware {
    /// Stable schema URI for this firmware record.
    #[serde(rename = "$schema")]
    pub schema_uri: String,
    /// Firmware record version.
    pub schema: u32,
    /// Stable firmware identity.
    pub id: String,
    /// Body identity that carries this firmware.
    pub body_id: String,
    /// Human-readable firmware version string.
    pub version: String,
    /// Firmware update content, when distributed for this version.
    pub update: Option<ContentIdentity>,
    /// Boot ROM content and its physical load address, when applicable.
    pub boot_rom: Option<BootRomIdentity>,
    /// Firmware installation and loader facts consumed by the emulator.
    pub installation: Option<FirmwareInstallation>,
    /// Generated observation facts consumed by supported runs.
    pub generated_facts: Option<FirmwareGeneratedFacts>,
}

impl Firmware {
    /// Validate the record's schema identity and content digests.
    pub fn validate(&self, selected_id: &str) -> Result<(), String> {
        if self.schema != 1 || self.schema_uri != FIRMWARE_SCHEMA_URI || self.id != selected_id {
            return Err("firmware schema or identity differs".into());
        }
        validate_id(&self.id)?;
        validate_id(&self.body_id)?;
        if let Some(content) = &self.update {
            content.validate()?;
        }
        if let Some(boot_rom) = &self.boot_rom {
            boot_rom.content.validate()?;
        }
        Ok(())
    }
}

/// A verified boot ROM image and its load address.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct BootRomIdentity {
    /// Boot ROM content identity.
    pub content: ContentIdentity,
    /// Address where the ROM image is loaded.
    pub address: u64,
}

/// The flash layout used to install a firmware update.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct FirmwareInstallation {
    /// Offset from the update input to the flash image.
    pub flash_offset: u64,
    /// Address used to validate the first firmware image.
    pub loader_entry: Option<u64>,
}

/// Generated firmware facts that inform state observation and initial setup.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct FirmwareGeneratedFacts {
    /// Addresses and values used to observe firmware readiness.
    pub observation: Option<FirmwareObservation>,
    /// Calibration and input requirements for the first start sequence.
    pub first_start: Option<FirstStart>,
}

/// Firmware addresses used by a supported run observer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct FirmwareObservation {
    /// Firmware address containing the UI mode.
    pub ui_mode: u64,
    /// Firmware address containing the system mode.
    pub system_mode: u64,
    /// Firmware address containing the menu-active flag.
    pub menu_active: u64,
    /// Firmware address containing the pending UI mode.
    pub pending_mode: u64,
    /// Firmware address containing the current menu node.
    pub menu_node: u64,
    /// REC mode value.
    pub rec_mode: u32,
    /// Setup mode value.
    pub setup_mode: u32,
    /// Readiness observer addresses.
    pub readiness: ReadinessAddresses,
}

/// Addresses used to check REC readiness.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ReadinessAddresses {
    /// Firmware address for readiness category one.
    pub category_one: u64,
    /// Firmware address for readiness category two.
    pub category_two: u64,
    /// Firmware address for pending input state.
    pub input_pending: u64,
    /// Mask of readiness bits excluded from the check.
    pub excluded_mask: u32,
    /// Firmware address for the operation-alert timer.
    pub operation_alert_timer: u64,
    /// Firmware address used to override readiness.
    pub override_address: u64,
}

/// First-start ADC calibration and press limits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct FirstStart {
    /// SoC ADC block identity.
    pub adc_block_id: String,
    /// ADC channel used by the first-start input.
    pub channel: u8,
    /// Firmware address of the calibration table pointer.
    pub calibration_pointer: u64,
    /// Lower calibration-table byte offset.
    pub lower_offset: u16,
    /// Upper calibration-table byte offset.
    pub upper_offset: u16,
    /// Press-and-hold time in guest ticks.
    pub hold_ticks: u64,
    /// Input settle time in guest ticks.
    pub settle_ticks: u64,
    /// Maximum first-start input presses.
    pub max_presses: u8,
}

/// All resolved reference records for one body and firmware selection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ResolvedSelection {
    /// Resolved SoC record selected by the body.
    pub soc: Soc,
    /// Resolved body record.
    pub body: Body,
    /// Resolved firmware record.
    pub firmware: Firmware,
    /// Resolved reusable parts referenced by the body.
    pub parts: Vec<Part>,
}

/// A checked, ID-indexed catalog of processor, part, body and firmware facts.
#[derive(Clone, Debug, Default)]
pub struct ReferenceCatalog {
    socs: BTreeMap<String, Soc>,
    parts: BTreeMap<String, Part>,
    bodies: BTreeMap<String, Body>,
    firmwares: BTreeMap<String, Firmware>,
}

impl ReferenceCatalog {
    /// Load and validate reference records from a catalog root directory.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, String> {
        let root = root.as_ref();
        let mut catalog = Self::default();
        for (path, bytes) in read_json_files(&root.join("soc"))? {
            let value: Soc = decode(&bytes, &path)?;
            value.validate(&value.id)?;
            insert_unique(&mut catalog.socs, value.id.clone(), value, &path)?;
        }
        for (path, bytes) in read_json_files(&root.join("parts"))? {
            let value: Part = decode(&bytes, &path)?;
            value.validate(&value.id)?;
            insert_unique(&mut catalog.parts, value.id.clone(), value, &path)?;
        }
        for (path, bytes) in read_json_files(&root.join("bodies"))? {
            let value: Body = decode(&bytes, &path)?;
            value.validate(&value.id)?;
            insert_unique(&mut catalog.bodies, value.id.clone(), value, &path)?;
        }
        for (path, bytes) in read_json_files(&root.join("firmware"))? {
            let value: Firmware = decode(&bytes, &path)?;
            value.validate(&value.id)?;
            insert_unique(&mut catalog.firmwares, value.id.clone(), value, &path)?;
        }
        catalog.validate_references()?;
        Ok(catalog)
    }

    /// Load the normalized reference catalog compiled into this library.
    pub fn bundled() -> Result<Self, String> {
        let mut catalog = Self::default();
        for (kind, path, bytes) in bundled_documents() {
            match kind {
                ReferenceKind::Soc => {
                    let value: Soc = decode(bytes.as_bytes(), Path::new(path))?;
                    value.validate(&value.id)?;
                    insert_unique(&mut catalog.socs, value.id.clone(), value, Path::new(path))?;
                }
                ReferenceKind::Part => {
                    let value: Part = decode(bytes.as_bytes(), Path::new(path))?;
                    value.validate(&value.id)?;
                    insert_unique(&mut catalog.parts, value.id.clone(), value, Path::new(path))?;
                }
                ReferenceKind::Body => {
                    let value: Body = decode(bytes.as_bytes(), Path::new(path))?;
                    value.validate(&value.id)?;
                    insert_unique(
                        &mut catalog.bodies,
                        value.id.clone(),
                        value,
                        Path::new(path),
                    )?;
                }
                ReferenceKind::Firmware => {
                    let value: Firmware = decode(bytes.as_bytes(), Path::new(path))?;
                    value.validate(&value.id)?;
                    insert_unique(
                        &mut catalog.firmwares,
                        value.id.clone(),
                        value,
                        Path::new(path),
                    )?;
                }
            }
        }
        catalog.validate_references()?;
        Ok(catalog)
    }

    /// Resolve a body and an explicitly selected firmware by identity.
    pub fn resolve(&self, body_id: &str, firmware_id: &str) -> Result<ResolvedSelection, String> {
        let body = self
            .bodies
            .get(body_id)
            .ok_or_else(|| format!("unknown body identity {body_id}"))?
            .clone();
        let firmware = self
            .firmwares
            .get(firmware_id)
            .ok_or_else(|| format!("unknown firmware identity {firmware_id}"))?
            .clone();
        if firmware.body_id != body.id {
            return Err(format!(
                "firmware {} belongs to body {}, not {}",
                firmware.id, firmware.body_id, body.id
            ));
        }
        let soc = self
            .socs
            .get(&body.soc_id)
            .ok_or_else(|| format!("body {} refers to missing SoC {}", body.id, body.soc_id))?
            .clone();
        let parts = body
            .parts
            .iter()
            .map(|id| {
                self.parts
                    .get(id)
                    .cloned()
                    .ok_or_else(|| format!("body {} refers to missing part {id}", body.id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ResolvedSelection {
            soc,
            body,
            firmware,
            parts,
        })
    }

    /// Resolve firmware and derive its body and SoC identities.
    pub fn resolve_firmware(&self, firmware_id: &str) -> Result<ResolvedSelection, String> {
        let firmware = self
            .firmwares
            .get(firmware_id)
            .ok_or_else(|| format!("unknown firmware identity {firmware_id}"))?;
        self.resolve(&firmware.body_id, firmware_id)
    }

    /// Return a resolved SoC by its stable identity.
    pub fn soc(&self, id: &str) -> Option<&Soc> {
        self.socs.get(id)
    }

    /// Return a resolved body by its stable identity.
    pub fn body(&self, id: &str) -> Option<&Body> {
        self.bodies.get(id)
    }

    /// Return a resolved firmware by its stable identity.
    pub fn firmware(&self, id: &str) -> Option<&Firmware> {
        self.firmwares.get(id)
    }

    /// Return a resolved reusable part by its stable identity.
    pub fn part(&self, id: &str) -> Option<&Part> {
        self.parts.get(id)
    }

    fn validate_references(&self) -> Result<(), String> {
        for body in self.bodies.values() {
            if !self.socs.contains_key(&body.soc_id) {
                return Err(format!(
                    "body {} refers to missing SoC {}",
                    body.id, body.soc_id
                ));
            }
            for part_id in &body.parts {
                if !self.parts.contains_key(part_id) {
                    return Err(format!("body {} refers to missing part {part_id}", body.id));
                }
            }
            for (role, part_id, expected_model) in [
                (
                    "sensor",
                    body.sensor.as_ref().map(|item| item.part_id.as_str()),
                    0u8,
                ),
                (
                    "lens",
                    body.lens.as_ref().map(|item| item.part_id.as_str()),
                    1u8,
                ),
                (
                    "display",
                    body.display.as_ref().map(|item| item.part_id.as_str()),
                    2u8,
                ),
                (
                    "microcontroller",
                    body.microcontroller
                        .as_ref()
                        .map(|item| item.part_id.as_str()),
                    3u8,
                ),
                (
                    "power controller",
                    body.power_control
                        .as_ref()
                        .map(|item| item.part_id.as_str()),
                    4u8,
                ),
                (
                    "card slot",
                    body.card_slot.as_ref().map(|item| item.part_id.as_str()),
                    5u8,
                ),
            ] {
                if let Some(part_id) = part_id {
                    if !body.parts.iter().any(|candidate| candidate == part_id) {
                        return Err(format!(
                            "body {} {role} is not listed among its installed parts",
                            body.id
                        ));
                    }
                    let part = &self.parts[part_id];
                    let model_matches = matches!(
                        (expected_model, &part.model),
                        (0, PartModel::ImageSensor { .. })
                            | (1, PartModel::LensMechanism { .. })
                            | (2, PartModel::DisplayPanel { .. })
                            | (3, PartModel::Microcontroller { .. })
                            | (4, PartModel::PowerControl { .. })
                            | (5, PartModel::SdCardSlot)
                    );
                    if !model_matches {
                        return Err(format!(
                            "body {} {role} part {part_id} has an incompatible model",
                            body.id
                        ));
                    }
                }
            }
            for component in [
                body.sensor.as_ref().map(|item| item.part_id.as_str()),
                body.lens.as_ref().map(|item| item.part_id.as_str()),
                body.display.as_ref().map(|item| item.part_id.as_str()),
                body.microcontroller
                    .as_ref()
                    .map(|item| item.part_id.as_str()),
                body.power_control
                    .as_ref()
                    .map(|item| item.part_id.as_str()),
                body.card_slot.as_ref().map(|item| item.part_id.as_str()),
            ]
            .into_iter()
            .flatten()
            {
                if !body.parts.iter().any(|part_id| part_id == component) {
                    return Err(format!(
                        "body {} component refers to undeclared part {component}",
                        body.id
                    ));
                }
            }
            for connection in [
                body.sensor.as_ref().map(|item| item.connection_id.as_str()),
                body.lens.as_ref().map(|item| item.connection_id.as_str()),
                body.display
                    .as_ref()
                    .map(|item| item.connection_id.as_str()),
                body.microcontroller
                    .as_ref()
                    .map(|item| item.connection_id.as_str()),
                body.power_control
                    .as_ref()
                    .map(|item| item.connection_id.as_str()),
                body.card_slot
                    .as_ref()
                    .map(|item| item.connection_id.as_str()),
            ]
            .into_iter()
            .flatten()
            {
                if !body.connections.iter().any(|item| item.id == connection) {
                    return Err(format!(
                        "body {} component refers to missing connection {connection}",
                        body.id
                    ));
                }
            }
            for wiring in &body.wiring {
                let source_exists = body
                    .connections
                    .iter()
                    .any(|item| item.id == wiring.source_id)
                    || body.parts.contains(&wiring.source_id)
                    || self.socs[&body.soc_id]
                        .blocks
                        .iter()
                        .any(|item| item.id == wiring.source_id);
                let target_exists = body
                    .connections
                    .iter()
                    .any(|item| item.id == wiring.target_id)
                    || body.parts.contains(&wiring.target_id)
                    || self.socs[&body.soc_id]
                        .blocks
                        .iter()
                        .any(|item| item.id == wiring.target_id);
                if !source_exists || !target_exists {
                    return Err(format!(
                        "body {} wiring refers to an unknown component",
                        body.id
                    ));
                }
            }
            let soc = &self.socs[&body.soc_id];
            let block_kind = |id: &str, expected: SocBlockKind, role: &str| {
                let block = soc
                    .blocks
                    .iter()
                    .find(|block| block.id == id)
                    .ok_or_else(|| {
                        format!("body {} {role} refers to missing SoC block {id}", body.id)
                    })?;
                if block.kind != expected {
                    return Err(format!(
                        "body {} {role} uses SoC block {id} with the wrong kind",
                        body.id
                    ));
                }
                Ok(())
            };
            if let Some(sensor) = &body.sensor {
                block_kind(&sensor.controller_id, SocBlockKind::ControlBus, "sensor")?;
            }
            if let Some(display) = &body.display {
                if let Some(route) = &display.route {
                    block_kind(
                        &route.controller_id,
                        SocBlockKind::SerialController,
                        "display",
                    )?;
                }
            }
            if let Some(mcu) = &body.microcontroller {
                block_kind(
                    &mcu.controller_id,
                    SocBlockKind::SerialController,
                    "microcontroller",
                )?;
                if !body
                    .connections
                    .iter()
                    .any(|item| item.id == mcu.select_connection_id)
                {
                    return Err(format!(
                        "body {} microcontroller refers to missing chip-select connection {}",
                        body.id, mcu.select_connection_id
                    ));
                }
            }
            if let Some(power) = &body.power_control {
                block_kind(
                    &power.controller_id,
                    SocBlockKind::SerialController,
                    "power controller",
                )?;
                block_kind(&power.adc_block_id, SocBlockKind::Adc, "power controller")?;
            }
            if let Some(card) = &body.card_slot {
                for connection_id in [
                    &card.detect_connection_id,
                    &card.write_protect_connection_id,
                ] {
                    if !body
                        .connections
                        .iter()
                        .any(|item| item.id == *connection_id)
                    {
                        return Err(format!(
                            "body {} card slot refers to missing contact connection {connection_id}",
                            body.id
                        ));
                    }
                }
            }
        }
        for firmware in self.firmwares.values() {
            if !self.bodies.contains_key(&firmware.body_id) {
                return Err(format!(
                    "firmware {} refers to missing body {}",
                    firmware.id, firmware.body_id
                ));
            }
        }
        Ok(())
    }
}

/// Validate a stable catalog, component or content identity.
pub fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 128
        || !id.as_bytes()[0].is_ascii_alphanumeric()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
        || id == "."
        || id == ".."
    {
        return Err("identity must be a bounded, safe identifier".into());
    }
    Ok(())
}

fn unique_ids<'a>(ids: impl IntoIterator<Item = &'a str>, label: &str) -> Result<(), String> {
    let mut unique = BTreeSet::new();
    for id in ids {
        validate_id(id)?;
        if !unique.insert(id) {
            return Err(format!("duplicate {label} identity {id}"));
        }
    }
    Ok(())
}

fn insert_unique<T>(
    target: &mut BTreeMap<String, T>,
    id: String,
    value: T,
    path: &Path,
) -> Result<(), String> {
    validate_id(&id)?;
    if target.insert(id.clone(), value).is_some() {
        return Err(format!("duplicate identity {id} in {}", path.display()));
    }
    Ok(())
}

fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8], path: &Path) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn read_json_files(directory: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("could not list {}: {error}", directory.display()))?;
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|error| format!("could not read {}: {error}", directory.display()))?
            .path();
        if path.is_dir() {
            paths.extend(read_json_files(&path)?);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            paths.push((
                path.clone(),
                fs::read(&path)
                    .map_err(|error| format!("could not read {}: {error}", path.display()))?,
            ));
        }
    }
    paths.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(paths)
}

#[derive(Clone, Copy, Debug)]
enum ReferenceKind {
    Soc,
    Part,
    Body,
    Firmware,
}

include!(concat!(env!("OUT_DIR"), "/bundled_catalog.rs"));

fn bundled_documents() -> impl Iterator<Item = (ReferenceKind, &'static str, &'static str)> {
    BUNDLED_CATALOG.iter().copied()
}

#[cfg(test)]
mod tests;

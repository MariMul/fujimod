# Generated from the Rust catalog schemas.
from __future__ import annotations

from typing import Literal, NotRequired, Required, TypedDict


class AdditionalUiModes(TypedDict, total=False):
    play: Required[int]
    setup_play: Required[int]


class AddressDecoder(TypedDict, total=False):
    choices: Required[list[DecoderChoice]]
    id: Required[str]
    initial: Required[DecoderSelection]


class AddressIdentity(TypedDict, total=False):
    address: Required[int]
    sha256: Required[str]
    size: Required[int]


class AddressMapping(TypedDict, total=False):
    address: Required[int]
    core: Required[str]
    id: Required[str]
    permissions: Required[int]
    physical: Required[int]
    size: Required[RegionSize]


class AperturePosition(TypedDict, total=False):
    f_number: Required[int]
    position: Required[int]


type BlockSignal = Literal["detect", "write_protect", "data_ready", "clock"]


Body = TypedDict(
    "Body",
    {
        "$schema": Required['Literal["https://fujimod.com/schemas/body.schema.json"]'],
        "card_slot": NotRequired["CardSlotAssembly | None"],
        "connections": Required["list[BodyConnection]"],
        "control_bus_chips": Required["list[ControlBusChip]"],
        "controls": NotRequired["BodyControls | None"],
        "display": NotRequired["DisplayAssembly | None"],
        "dram_bytes": Required["int"],
        "id": Required["str"],
        "lens": NotRequired["LensAssembly | None"],
        "microcontroller": NotRequired["MicrocontrollerAssembly | None"],
        "nand": NotRequired["NandGeometry | None"],
        "parts": Required["list[str]"],
        "power_control": NotRequired["PowerControlAssembly | None"],
        "schema": Required["int"],
        "sensor": NotRequired["SensorAssembly | None"],
        "soc_id": Required["str"],
        "wiring": Required["list[BodyWiring]"],
    },
    total=False,
)


class BodyConnection(TypedDict, total=False):
    id: Required[str]
    kind: Required[ConnectionKind]
    signals: Required[list[ConnectionSignal]]


class BodyControl(TypedDict, total=False):
    assignable: Required[bool]
    connection_ids: Required[list[str]]
    default_function: NotRequired[str | None]
    kind: Required[ControlKind]
    location: Required[ControlLocation]
    manual_page: NotRequired[int | None]
    name: Required[str]
    positions: Required[list[ControlPosition]]
    pushable: Required[bool]


class BodyControls(TypedDict, total=False):
    analog_levels: Required[int]
    controls: Required[dict[str, BodyControl]]
    manual: Required[ManualReference]


class BodyWiring(TypedDict, total=False):
    signal: Required[WireSignal]
    source_id: Required[str]
    target_id: Required[str]


class BootRomIdentity(TypedDict, total=False):
    address: Required[int]
    content: Required[ContentIdentity]


class CalibrationInterval(TypedDict, total=False):
    lower: Required[int]
    upper: Required[int]


class CardSlotAssembly(TypedDict, total=False):
    connection_id: Required[str]
    controller_id: NotRequired[str | None]
    detect_connection_id: Required[str]
    part_id: Required[str]
    write_protect_connection_id: Required[str]


class ClockDomain(TypedDict, total=False):
    frequency_hz: NotRequired[int | None]
    id: Required[str]
    parent_id: NotRequired[str | None]


type ConfigurationLocation = (
    ConfigurationLocationVariant0 | ConfigurationLocationVariant1
)


class ConfigurationLocationVariant0(TypedDict, total=False):
    address: Required[int]
    kind: Required[Literal["pointer"]]


class ConfigurationLocationVariant1(TypedDict, total=False):
    address: Required[int]
    kind: Required[Literal["direct"]]


type ConnectionKind = Literal[
    "control",
    "usb",
    "hdmi",
    "sd-card",
    "remote",
    "hot-shoe",
    "external-power",
    "battery-door",
    "sensor",
    "lens",
    "display",
    "microcontroller",
    "power-control",
]


type ConnectionSignal = (
    ConnectionSignalVariant0
    | ConnectionSignalVariant1
    | ConnectionSignalVariant2
    | ConnectionSignalVariant3
)


class ConnectionSignalVariant0(TypedDict, total=False):
    active_level: Required[bool]
    kind: Required[Literal["gpio"]]
    pin: Required[int]
    port: Required[int]


class ConnectionSignalVariant1(TypedDict, total=False):
    channel: Required[int]
    kind: Required[Literal["adc"]]
    threshold: NotRequired[int | None]


class ConnectionSignalVariant2(TypedDict, total=False):
    block_id: Required[str]
    kind: Required[Literal["block"]]
    signal: Required[BlockSignal]


class ConnectionSignalVariant3(TypedDict, total=False):
    address: Required[int]
    block_id: Required[str]
    channel: Required[int]
    kind: Required[Literal["serial"]]


class ContactPosition(TypedDict, total=False):
    contacts: Required[list[bool]]
    drive: Required[int]


class ContentIdentity(TypedDict, total=False):
    sha256: Required[str]
    size: Required[int]


class ControlAction(TypedDict, total=False):
    keys: Required[list[str]]


class ControlBusChip(TypedDict, total=False):
    address: Required[int]
    channel: Required[int]
    controller_id: Required[str]
    polarity_selector: Required[int]


type ControlInput = ControlInputVariant0 | ControlInputVariant1 | ControlInputVariant2


class ControlInputVariant0(TypedDict, total=False):
    callback: NotRequired[int | None]
    connection_id: Required[str]
    event: Required[int]
    expected_result: NotRequired[int | None]
    intervals: Required[list[CalibrationInterval]]
    kind: Required[Literal["adc"]]
    scanner_index: Required[int]


class ControlInputVariant1(TypedDict, total=False):
    connection_id: Required[str]
    event: Required[int]
    expected_level: Required[bool]
    kind: Required[Literal["gpio"]]
    scanner: Required[GpioScanner]
    scanner_index: Required[int]


class ControlInputVariant2(TypedDict, total=False):
    connection_id: Required[str]
    event: Required[int]
    guard: Required[int]
    kind: Required[Literal["gpio_quadrature"]]
    phase_map: Required[list[int]]
    reset: Required[list[int]]
    threshold: Required[int]
    transitions: Required[list[int]]


type ControlInterface = ControlInterfaceVariant0 | ControlInterfaceVariant1


class ControlInterfaceVariant0(TypedDict, total=False):
    connection_id: Required[str]
    endpoint: Required[int]
    kind: Required[Literal["power_switch"]]
    mask: Required[int]
    off_value: Required[int]
    on_value: Required[int]
    register: Required[int]
    saved_inputs: Required[int]


class ControlInterfaceVariant1(TypedDict, total=False):
    connection_id: Required[str]
    contacts: Required[int]
    coordinate_bits: Required[int]
    height: Required[int]
    irq: Required[int]
    irq_mode: Required[int]
    kind: Required[Literal["prefixed_touch"]]
    mode: Required[int]
    offset_tags: Required[list[int]]
    priority: Required[int]
    register: Required[int]
    report_bytes: Required[int]
    width: Required[int]


type ControlKind = Literal[
    "dial",
    "button",
    "lever",
    "switch",
    "ring",
    "shutter",
    "wheel",
    "joystick",
    "touchscreen",
]


type ControlLocation = Literal["top", "front", "rear", "lens", "bottom"]


class ControlPosition(TypedDict, total=False):
    description: NotRequired[str | None]
    id: Required[str]
    label: NotRequired[str | None]


class ControlReaders(TypedDict, total=False):
    calibration: Required[int]
    scanner: Required[ScannerReader]


class CoprocessorWrite(TypedDict, total=False):
    core: Required[str]
    cp: Required[int]
    crm: Required[int]
    crn: Required[int]
    opc1: Required[int]
    opc2: Required[int]
    secure: Required[int]
    set_bits: Required[int]
    wide: Required[bool]


class CoreRelease(TypedDict, total=False):
    mask: Required[int]
    register: Required[int]
    source: Required[int]
    target: Required[int]


class DecoderChoice(TypedDict, total=False):
    id: Required[str]
    selection: Required[DecoderSelection]


class DecoderSelection(TypedDict, total=False):
    dma_mappings: Required[list[str]]
    processors: Required[list[str]]


class DisplayAssembly(TypedDict, total=False):
    connection_id: Required[str]
    part_id: Required[str]
    route: NotRequired[SerialRoute | None]


type DisplayOrientation = Literal["landscape", "portrait", "tilting"]


class DmaMapping(TypedDict, total=False):
    address: Required[int]
    id: Required[str]
    permissions: Required[int]
    physical: Required[int]
    size: Required[RegionSize]


Firmware = TypedDict(
    "Firmware",
    {
        "$schema": Required[
            'Literal["https://fujimod.com/schemas/firmware.schema.json"]'
        ],
        "body_id": Required["str"],
        "boot_rom": NotRequired["BootRomIdentity | None"],
        "generated_facts": NotRequired["FirmwareGeneratedFacts | None"],
        "id": Required["str"],
        "installation": NotRequired["FirmwareInstallation | None"],
        "schema": Required["int"],
        "update": NotRequired["ContentIdentity | None"],
        "version": Required["str"],
    },
    total=False,
)


class FirmwareControl(TypedDict, total=False):
    actions: Required[dict[str, ControlAction]]
    input: NotRequired[dict[str, list[ControlInput]] | None]
    interface: NotRequired[ControlInterface | None]
    keys: Required[list[str]]
    positions: Required[list[FirmwareControlPosition]]
    unavailable_reason: NotRequired[str | None]


class FirmwareControlPosition(TypedDict, total=False):
    active: NotRequired[bool | None]
    keys: Required[list[str]]
    position: Required[str]


class FirmwareControls(TypedDict, total=False):
    controls: Required[dict[str, FirmwareControl]]
    debug_keys_available: Required[bool]
    input_readers: Required[ControlReaders]
    native_key_names: Required[NativeKeyNames]
    scanner: Required[ScannerTable]


class FirmwareGeneratedFacts(TypedDict, total=False):
    additional_ui_modes: NotRequired[AdditionalUiModes | None]
    cfgdata_location: NotRequired[ConfigurationLocation | None]
    controls: NotRequired[FirmwareControls | None]
    identity_checks: Required[list[AddressIdentity]]
    menu_cursor: NotRequired[int | None]
    menu_keys: NotRequired[MenuKeys | None]
    observation: NotRequired[FirmwareObservation | None]
    uncached_alias: NotRequired[int | None]


class FirmwareImageSection(TypedDict, total=False):
    address: Required[int]
    name: Required[str]
    native_flash_offset: NotRequired[int | None]
    sha256: Required[str]
    size: Required[int]


class FirmwareInstallation(TypedDict, total=False):
    entry_labels: Required[dict[str, str]]
    flash_offset: Required[int]
    image_sections: Required[list[FirmwareImageSection]]
    loader_validation: NotRequired[FirmwareLoaderValidation | None]


class FirmwareLoaderValidation(TypedDict, total=False):
    entry: Required[int]
    sections: Required[list[str]]


class FirmwareObservation(TypedDict, total=False):
    menu_active: Required[int]
    menu_node: Required[int]
    pending_mode: Required[int]
    readiness: Required[ReadinessAddresses]
    rec_mode: Required[int]
    setup_mode: Required[int]
    system_mode: Required[int]
    ui_mode: Required[int]


class FocusAxis(TypedDict, total=False):
    infinity_position: Required[int]
    output: Required[OpticalQuantity]
    steps_per_mm: Required[int]


class GeneratedSceneAsset(TypedDict, total=False):
    id: Required[str]
    pattern: Required[list[int]]
    size: Required[int]


type GpioScanner = GpioScannerVariant0 | GpioScannerVariant1


class GpioScannerVariant0(TypedDict, total=False):
    kind: Required[Literal["direct"]]


class GpioScannerVariant1(TypedDict, total=False):
    kind: Required[Literal["combined"]]
    lower: Required[int]
    source: Required[int]
    upper: Required[int]


class HomeSwitch(TypedDict, total=False):
    threshold: Required[int]


class InterruptRoute(TypedDict, total=False):
    core: Required[str]
    line: Required[int]
    source_block: Required[str]


class IrisOptics(TypedDict, total=False):
    output: Required[OpticalQuantity]
    positions: Required[list[AperturePosition]]
    travel_limits: Required[list[int]]


class LensAssembly(TypedDict, total=False):
    connection_id: Required[str]
    encoder_connection_id: NotRequired[str | None]
    focus: NotRequired[FocusAxis | None]
    home_connections: Required[list[LensHomeConnection]]
    iris: NotRequired[IrisOptics | None]
    part_id: Required[str]
    pulse: Required[PulseInterface]
    route: Required[SerialRoute]


class LensHomeConnection(TypedDict, total=False):
    connection_id: Required[str]
    motor: Required[int]


class LensMotor(TypedDict, total=False):
    home: NotRequired[HomeSwitch | None]
    initial_position: Required[int]
    settle_ticks: Required[int]
    ticks_per_step: Required[int]
    travel_limits: Required[list[int]]


type LensProtocol = Literal["banked", "three-byte", "mixed-width", "dc-zoom"]


class MProfileState(TypedDict, total=False):
    cpuid: Required[int]
    vector_base: Required[int]


class ManualReference(TypedDict, total=False):
    pages: Required[list[int]]
    title: Required[str]
    url: Required[str]


type MemoryKind = Literal["dram", "rom", "sram", "tcm"]


class MemoryRegion(TypedDict, total=False):
    base: Required[int]
    id: Required[str]
    kind: Required[MemoryKind]
    size: Required[RegionSize]


class MenuKeys(TypedDict, total=False):
    back: Required[str]
    down: Required[str]
    menu: Required[str]
    up: Required[str]


class MicrocontrollerAssembly(TypedDict, total=False):
    channel: Required[int]
    connection_id: Required[str]
    controller_id: Required[str]
    part_id: Required[str]
    select_connection_id: Required[str]


class NandGeometry(TypedDict, total=False):
    bytes: Required[int]
    page_bytes: Required[int]
    pages_per_block: Required[int]
    spare_bytes: Required[int]


class NativeKeyName(TypedDict, total=False):
    event: Required[int]
    name: Required[str]
    string_address: Required[int]


class NativeKeyNames(TypedDict, total=False):
    reader: Required[int]
    rows: Required[list[NativeKeyName]]
    sha256: Required[str]
    size: Required[int]


type OpticalQuantity = Literal["lens_focus", "lens_aperture"]


Part = TypedDict(
    "Part",
    {
        "$schema": Required['Literal["https://fujimod.com/schemas/part.schema.json"]'],
        "id": Required["str"],
        "model": Required["PartModel"],
        "schema": Required["int"],
    },
    total=False,
)


class PartEncoder(TypedDict, total=False):
    phase_ticks: Required[int]
    rest_phase: Required[int]


type PartModel = (
    PartModelVariant0
    | PartModelVariant1
    | PartModelVariant2
    | PartModelVariant3
    | PartModelVariant4
    | PartModelVariant5
    | PartModelVariant6
    | PartModelVariant7
)


class PartModelVariant0(TypedDict, total=False):
    encoder: NotRequired[PartEncoder | None]
    kind: Required[Literal["lens-mechanism"]]
    motors: Required[list[LensMotor]]
    protocol: Required[LensProtocol]
    zoom: NotRequired[ZoomMechanism | None]


class PartModelVariant1(TypedDict, total=False):
    kind: Required[Literal["contact-actuator"]]
    positions: Required[list[ContactPosition]]
    settle_ticks: Required[int]


class PartModelVariant2(TypedDict, total=False):
    endpoint: Required[SerialOutputKind]
    kind: Required[Literal["serial-output"]]


class PartModelVariant3(TypedDict, total=False):
    clock_registers: Required[RegisterRange]
    initial_registers: Required[list[int]]
    kind: Required[Literal["microcontroller"]]
    packet_registers: Required[RegisterRange]
    registers: Required[RegisterRange]
    status_register: NotRequired[int | None]


class PartModelVariant4(TypedDict, total=False):
    kind: Required[Literal["power-control"]]
    status_registers: Required[list[StatusRegister]]


class PartModelVariant5(TypedDict, total=False):
    kind: Required[Literal["sd-card-slot"]]


class PartModelVariant6(TypedDict, total=False):
    black_level: Required[int]
    color_filter: Required[list[int]]
    control: Required[SensorControlInterface]
    full_scale: Required[int]
    kind: Required[Literal["image-sensor"]]
    raster: Required[SensorRaster]
    reference_white: Required[list[int]]


class PartModelVariant7(TypedDict, total=False):
    height: NotRequired[int | None]
    kind: Required[Literal["display-panel"]]
    orientation: NotRequired[DisplayOrientation | None]
    serial: Required[SerialOutputKind]
    used_mask: Required[int]
    width: NotRequired[int | None]


class PeripheralMapping(TypedDict, total=False):
    address: Required[int]
    block_id: Required[str]
    core: Required[str]
    offset: NotRequired[int | None]


class PowerControlAssembly(TypedDict, total=False):
    channel: Required[int]
    connection_id: Required[str]
    controller_id: Required[str]
    part_id: Required[str]
    select_connection_id: Required[str]


type ProcessorArchitecture = Literal["arm32", "aarch64"]


class ProcessorCore(TypedDict, total=False):
    id: Required[str]
    model: Required[ProcessorModel]
    mprofile: NotRequired[MProfileState | None]
    registers: Required[list[RegisterValue]]
    released: Required[bool]
    rprofile: NotRequired[RProfileState | None]


type ProcessorModel = Literal["cortex-r5f", "cortex-a7", "cortex-a53"]


type ProcessorRegister = Literal["pc", "sp", "pstate", "cpsr"]


class PulseInterface(TypedDict, total=False):
    counter_channels: Required[list[int]]
    initial_phase: Required[int]
    quadrature: Required[bool]
    timer_channels: Required[list[int]]


class RProfileState(TypedDict, total=False):
    atcm: Required[TcmState]
    btcm: Required[TcmState]
    cp14_identity: Required[int]


class ReadinessAddresses(TypedDict, total=False):
    category_one: Required[int]
    category_two: Required[int]
    excluded_mask: Required[int]
    input_pending: Required[int]
    operation_alert_timer: Required[int]
    override_address: Required[int]


type RegionSize = RegionSizeVariant0 | RegionSizeVariant1


class RegionSizeVariant0(TypedDict, total=False):
    bytes: Required[int]
    kind: Required[Literal["bytes"]]


class RegionSizeVariant1(TypedDict, total=False):
    kind: Required[Literal["body_dram"]]


class RegisterRange(TypedDict, total=False):
    count: Required[int]
    first: Required[int]
    width_bytes: Required[int]


class RegisterValue(TypedDict, total=False):
    register: Required[ProcessorRegister]
    value: Required[int]


class ResolvedConfiguration(TypedDict, total=False):
    configuration: Required[RunConfiguration]
    reference: Required[ResolvedSelection]


class ResolvedSelection(TypedDict, total=False):
    body: Required[Body]
    firmware: Required[Firmware]
    parts: Required[list[Part]]
    soc: Required[Soc]


class RunAdcInputs(TypedDict, total=False):
    block_id: Required[str]
    samples: Required[list[int]]


class RunCardInput(TypedDict, total=False):
    connection_id: Required[str]
    present: Required[bool]
    write_protected: Required[bool]


RunConfiguration = TypedDict(
    "RunConfiguration",
    {
        "$schema": Required[
            'Literal["https://fujimod.com/schemas/emulator-run-configuration.schema.json"]'
        ],
        "body_id": Required["str"],
        "execution": Required["RunExecution"],
        "firmware_id": Required["str"],
        "id": Required["str"],
        "inputs": Required["RunInputs"],
        "overlays": Required["list[str]"],
        "scene": Required["RunScene"],
        "schema": Required["int"],
        "start": Required["RunStart"],
    },
    total=False,
)


class RunConnectionState(TypedDict, total=False):
    connected: Required[bool]
    connection_id: Required[str]


type RunEngine = Literal["unicorn"]


class RunExecution(TypedDict, total=False):
    engine: Required[RunEngine]
    instructions_per_tick: Required[RunInstructionRate]
    mode: Required[RunExecutionMode]
    quantum: Required[int]


type RunExecutionMode = Literal["strict"]


class RunFirstStart(TypedDict, total=False):
    hold_ticks: Required[int]
    key_id: Required[str]
    max_presses: Required[int]
    settle_ticks: Required[int]


class RunGpioInputs(TypedDict, total=False):
    block_id: Required[str]
    connections: Required[list[RunConnectionState]]
    keys: Required[list[RunKeyState]]


class RunImage(TypedDict, total=False):
    asset: Required[str]
    height: Required[int]
    stride: Required[int]
    width: Required[int]


class RunImages(TypedDict, total=False):
    live_image: Required[RunImage]


class RunInputs(TypedDict, total=False):
    adc: Required[RunAdcInputs]
    card: Required[RunCardInput]
    first_start: NotRequired[RunFirstStart | None]
    gpio: Required[RunGpioInputs]
    mcu: Required[RunMcuInput]
    power: Required[RunPowerInput]
    serial_registers: Required[list[RunSerialRegisters]]
    usb: Required[RunUsbInput]


class RunInstructionRate(TypedDict, total=False):
    denominator: Required[int]
    numerator: Required[int]


class RunKeyState(TypedDict, total=False):
    connection_id: Required[str]
    pressed: Required[bool]


class RunMcuInput(TypedDict, total=False):
    packet: Required[list[int]]
    part_id: Required[str]
    rtc: Required[RunRtcInput]


RunOverlay = TypedDict(
    "RunOverlay",
    {
        "$schema": Required[
            'Literal["https://fujimod.com/schemas/emulator-run-overlay.schema.json"]'
        ],
        "change": Required["RunOverlayChange"],
        "configuration_id": Required["str"],
        "id": Required["str"],
        "schema": Required["int"],
    },
    total=False,
)


type RunOverlayChange = RunOverlayChangeVariant0 | RunOverlayChangeVariant1


class RunOverlayChangeVariant0(TypedDict, total=False):
    connected: Required[bool]
    kind: Required[Literal["usb_connection"]]


class RunOverlayChangeVariant1(TypedDict, total=False):
    kind: Required[Literal["start"]]
    start: Required[RunStart]


class RunPowerInput(TypedDict, total=False):
    boot_reply: Required[int]
    part_id: Required[str]
    powered_on: Required[bool]
    spi_status: Required[list[RunRegisterValue]]


class RunRegisterValue(TypedDict, total=False):
    register: Required[int]
    value: Required[int]


class RunRtcInput(TypedDict, total=False):
    initial_seconds: Required[int]
    maximum_seconds: Required[int]
    started: Required[int]
    ticks_per_second: Required[int]


class RunScene(TypedDict, total=False):
    frame_period_ticks: Required[int]
    generated_assets: Required[list[GeneratedSceneAsset]]
    images: Required[RunImages]
    statistics: Required[RunStatistics]


class RunSerialRegisters(TypedDict, total=False):
    part_id: Required[str]
    values: Required[list[RunRegisterValue]]


type RunStart = RunStartVariant0 | RunStartVariant1


class RunStartVariant0(TypedDict, total=False):
    kind: Required[Literal["rom"]]


class RunStartVariant1(TypedDict, total=False):
    kind: Required[Literal["loader"]]


class RunStatistics(TypedDict, total=False):
    live_statistics: Required[list[RunStatisticsSpan]]


class RunStatisticsSpan(TypedDict, total=False):
    asset: Required[str]
    destination: Required[int]
    offset: Required[int]


class RunUsbInput(TypedDict, total=False):
    connected: Required[bool]
    connection_id: Required[str]


class ScannerReader(TypedDict, total=False):
    routine: Required[AddressIdentity]
    stable_offset: Required[int]


class ScannerTable(TypedDict, total=False):
    base: Required[int]
    count: Required[int]
    lookup: Required[int]
    sha256: Required[str]
    stride: Required[int]


class SensorAssembly(TypedDict, total=False):
    connection_id: Required[str]
    part_id: Required[str]


class SensorControlInterface(TypedDict, total=False):
    data_bits: Required[int]
    register_address_bits: Required[int]


class SensorRaster(TypedDict, total=False):
    height: Required[int]
    image_area: Required[list[int]]
    pixel_clock_hz: NotRequired[int | None]
    width: Required[int]


type SerialOutputKind = Literal["panel", "registers", "command_data", "fixed"]


class SerialRoute(TypedDict, total=False):
    channel: Required[int]
    clock_hz: NotRequired[int | None]
    controller_id: Required[str]
    endpoint_id: Required[str]
    select_connection_id: Required[str]
    word_bits: Required[int]


Soc = TypedDict(
    "Soc",
    {
        "$schema": Required['Literal["https://fujimod.com/schemas/soc.schema.json"]'],
        "architecture": Required["ProcessorArchitecture"],
        "blocks": Required["list[SocBlock]"],
        "clocks": Required["list[ClockDomain]"],
        "decoders": Required["list[AddressDecoder]"],
        "dma_mappings": Required["list[DmaMapping]"],
        "id": Required["str"],
        "interrupts": Required["list[InterruptRoute]"],
        "mappings": Required["list[AddressMapping]"],
        "memory_regions": Required["list[MemoryRegion]"],
        "peripherals": Required["list[PeripheralMapping]"],
        "processors": Required["list[ProcessorCore]"],
        "reset": Required["SocReset"],
        "schema": Required["int"],
    },
    total=False,
)


class SocBlock(TypedDict, total=False):
    base: Required[int]
    clock_id: NotRequired[str | None]
    id: Required[str]
    kind: Required[SocBlockKind]
    size: Required[int]


type SocBlockKind = Literal[
    "gpio",
    "adc",
    "interrupt-controller",
    "timer",
    "dma",
    "control-bus",
    "serial-controller",
    "nand-controller",
    "sd-card-controller",
    "usb-controller",
    "display-controller",
    "imaging-controller",
    "reset-controller",
    "clock-controller",
    "address-window",
]


class SocReset(TypedDict, total=False):
    coprocessor_writes: Required[list[CoprocessorWrite]]
    release: NotRequired[CoreRelease | None]


class StatusRegister(TypedDict, total=False):
    address: Required[int]
    value: Required[int]


class TcmState(TypedDict, total=False):
    bytes: Required[int]
    reset_address: Required[int]


type WireSignal = Literal[
    "gpio",
    "adc",
    "serial_data",
    "serial_clock",
    "image_data",
    "display_data",
    "microcontroller",
    "storage",
    "power",
]


class ZoomMechanism(TypedDict, total=False):
    maximum_position: Required[int]
    minimum_position: Required[int]
    pulse_ticks: Required[int]

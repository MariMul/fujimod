# Generated from the Rust catalog schemas.
from __future__ import annotations

from typing import Literal, Never, NotRequired, Required, TypedDict


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


type ApplicationBlockInput = (
    ApplicationBlockInputVariant0
    | ApplicationBlockInputVariant1
    | ApplicationBlockInputVariant2
    | ApplicationBlockInputVariant3
    | ApplicationBlockInputVariant4
)


class ApplicationBlockInputVariant0(TypedDict, total=False):
    block_id: Required[str]
    kind: Required[Literal["adc"]]
    samples: Required[list[int]]


class ApplicationBlockInputVariant1(TypedDict, total=False):
    block_id: Required[str]
    kind: Required[Literal["gpio"]]
    levels: Required[list[int]]
    outputs: Required[list[GpioOutputWord]]


class ApplicationBlockInputVariant2(TypedDict, total=False):
    block_id: Required[str]
    kind: Required[Literal["temperature"]]
    millicelsius: Required[int]


class ApplicationBlockInputVariant3(TypedDict, total=False):
    block_id: Required[str]
    idle_high: Required[bool]
    kind: Required[Literal["serial_line"]]


class ApplicationBlockInputVariant4(TypedDict, total=False):
    block_id: Required[str]
    erased_tail: NotRequired[int | None]
    kind: Required[Literal["flash"]]
    part_id: Required[str]
    seeds: Required[list[FlashSeed]]
    source: Required[ApplicationFlashSource]


class ApplicationCommandImageInput(TypedDict, total=False):
    block_ids: Required[list[str]]
    luma: Required[int]


type ApplicationFlashSource = Literal["firmware_update"]


class ApplicationFrameInput(TypedDict, total=False):
    active_ticks: NotRequired[int | None]
    block_ids: Required[list[str]]
    luma: NotRequired[int | None]
    period_ticks: Required[int]


type ApplicationHardware = ApplicationHardwareVariant0 | ApplicationHardwareVariant1


class ApplicationHardwareVariant0(TypedDict, total=False):
    caches: Required[CacheGeometry]
    configuration_base: Required[int]
    kind: Required[Literal["armv7a"]]


class ApplicationHardwareVariant1(TypedDict, total=False):
    affinities: Required[dict[str, int]]
    counter_clock_id: Required[str]
    kind: Required[Literal["armv8a"]]
    midr_el1: Required[int]
    physical_timer_irq: Required[int]
    pmu_clock_id: Required[str]
    virtual_timer_irq: Required[int]


type ApplicationPartInput = (
    ApplicationPartInputVariant0
    | ApplicationPartInputVariant1
    | ApplicationPartInputVariant2
    | ApplicationPartInputVariant3
)


class ApplicationPartInputVariant0(TypedDict, total=False):
    kind: Required[Literal["register_words"]]
    part_id: Required[str]
    registers: Required[list[RegisterWord]]
    rtc_ticks_per_second: NotRequired[int | None]


class ApplicationPartInputVariant1(TypedDict, total=False):
    kind: Required[Literal["clock"]]
    part_id: Required[str]
    seconds: Required[int]
    valid: Required[bool]


class ApplicationPartInputVariant2(TypedDict, total=False):
    high: Required[bool]
    kind: Required[Literal["idle_level"]]
    part_id: Required[str]


class ApplicationPartInputVariant3(TypedDict, total=False):
    idle_high: Required[bool]
    kind: Required[Literal["bis_revision"]]
    part_id: Required[str]
    revision: Required[list[int]]


class ApplicationStatisticsInput(TypedDict, total=False):
    black_level: Required[int]
    block_id: Required[str]
    luma: Required[int]
    samples_per_cell: Required[int]
    signal_shift: Required[int]


type ApplicationTiming = (
    ApplicationTimingVariant0
    | ApplicationTimingVariant1
    | ApplicationTimingVariant2
    | ApplicationTimingVariant3
)


class ApplicationTimingVariant0(TypedDict, total=False):
    block_ids: Required[list[str]]
    kind: Required[Literal["delay"]]
    operation: Required[TimingOperation]
    ticks: Required[int]


class ApplicationTimingVariant1(TypedDict, total=False):
    block_id: Required[str]
    bytes_per_tick: Required[int]
    kind: Required[Literal["throughput"]]
    setup_ticks: Required[int]


class ApplicationTimingVariant2(TypedDict, total=False):
    block_ids: Required[list[str]]
    kind: Required[Literal["parent_clock"]]
    ticks_per_parent: Required[int]


class ApplicationTimingVariant3(TypedDict, total=False):
    erase_ticks: Required[int]
    kind: Required[Literal["storage_programming"]]
    page_ticks: Required[int]
    part_id: Required[str]


class BitmapLayout(TypedDict, total=False):
    data_bytes: Required[int]
    data_offset: Required[int]
    height_bytes: Required[int]
    height_offset: Required[int]
    kind_bytes: Required[int]
    kind_offset: Required[int]
    width_bytes: Required[int]
    width_offset: Required[int]


class BitmapResources(TypedDict, total=False):
    final_first_index: Required[int]
    final_table: Required[int]
    last_index: Required[int]
    last_record: Required[int]
    layout: Required[BitmapLayout]
    stride: Required[int]
    table: Required[int]


type BlockHardware = (
    BlockHardwareVariant0
    | BlockHardwareVariant1
    | BlockHardwareVariant2
    | BlockHardwareVariant3
    | BlockHardwareVariant4
    | BlockHardwareVariant5
)


class BlockHardwareVariant0(TypedDict, total=False):
    channels: Required[int]
    kind: Required[Literal["dma"]]
    request_mux_block_id: Required[str]


class BlockHardwareVariant1(TypedDict, total=False):
    aperture_block_id: Required[str]
    kind: Required[Literal["serial_flash"]]


class BlockHardwareVariant2(TypedDict, total=False):
    bank_bytes: Required[int]
    kind: Required[Literal["imaging_input_buffer"]]
    sram_region_id: Required[str]


class BlockHardwareVariant3(TypedDict, total=False):
    kind: Required[Literal["display"]]
    picture_layout: NotRequired[PictureLayout | None]
    video_layout: NotRequired[VideoLayout | None]


class BlockHardwareVariant4(TypedDict, total=False):
    kind: Required[Literal["serial_dma"]]
    receive_request: Required[int]
    transmit_request: Required[int]


class BlockHardwareVariant5(TypedDict, total=False):
    companion_irq_base: Required[int]
    kind: Required[Literal["companion_dma"]]


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
        "hardware": NotRequired["BodyHardware | None"],
        "id": Required["str"],
        "lens": NotRequired["LensAssembly | None"],
        "microcontroller": NotRequired["MicrocontrollerAssembly | None"],
        "nand": NotRequired["NandGeometry | None"],
        "parts": Required["list[str]"],
        "power_control": NotRequired["PowerControlAssembly | None"],
        "ram_regions": NotRequired["list[BodyRamRegion]"],
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


class BodyHardware(TypedDict, total=False):
    gpio_feedback: Required[list[GpioFeedback]]
    keypad_samples: Required[dict[str, int]]
    serial_attachments: Required[list[SerialAttachment]]
    thermal_connection_ids: Required[list[str]]


class BodyRamRegion(TypedDict, total=False):
    base: Required[int]
    bytes: Required[int]
    id: Required[str]
    kind: Required[MemoryKind]


class BodyWiring(TypedDict, total=False):
    signal: Required[WireSignal]
    source_id: Required[str]
    target_id: Required[str]


class BootRomIdentity(TypedDict, total=False):
    address: Required[int]
    content: Required[ContentIdentity]


class BoundPosition(TypedDict, total=False):
    asserted: Required[bool]
    description: NotRequired[str | None]
    id: Required[str]
    key: Required[InputBinding]
    label: NotRequired[str | None]


class CacheGeometry(TypedDict, total=False):
    l1_data_bytes: Required[int]
    l1_instruction_bytes: Required[int]
    l2_bytes: Required[int]


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


class CompanionHardware(TypedDict, total=False):
    clock_id: Required[str]
    mailbox: Required[CompanionMailbox]
    mailbox_block_id: Required[str]
    processor_id: Required[str]
    vector_base: Required[int]


class CompanionMailbox(TypedDict, total=False):
    control: Required[int]
    fault: Required[int]
    reply: Required[int]
    request: Required[int]
    status: Required[int]


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
    | ConnectionSignalVariant4
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


class ConnectionSignalVariant4(TypedDict, total=False):
    controller_id: Required[str]
    kind: Required[Literal["interrupt"]]
    line: Required[int]


class ContactPosition(TypedDict, total=False):
    contacts: Required[list[bool]]
    drive: Required[int]


class ContentIdentity(TypedDict, total=False):
    sha256: Required[str]
    size: Required[int]


class ControlAction(TypedDict, total=False):
    keys: Required[list[str]]


type ControlBehavior = (
    ControlBehaviorVariant0
    | ControlBehaviorVariant1
    | ControlBehaviorVariant2
    | ControlBehaviorVariant3
    | ControlBehaviorVariant4
    | ControlBehaviorVariant5
    | ControlBehaviorVariant6
    | ControlBehaviorVariant7
)


class ControlBehaviorVariant0(TypedDict, total=False):
    kind: Required[Literal["button"]]
    press: Required[InputBinding]


class ControlBehaviorVariant1(TypedDict, total=False):
    kind: Required[Literal["shutter"]]
    stages: Required[dict[str, list[InputBinding]]]


class ControlBehaviorVariant2(TypedDict, total=False):
    directions: Required[dict[str, InputBinding]]
    kind: Required[Literal["pad"]]
    press: NotRequired[InputBinding | None]


class ControlBehaviorVariant3(TypedDict, total=False):
    endless: Required[bool]
    kind: Required[Literal["dial"]]
    motion: Required[ControlMotion]
    press: NotRequired[InputBinding | None]


class ControlBehaviorVariant4(TypedDict, total=False):
    endless: Required[bool]
    kind: Required[Literal["ring"]]
    motion: Required[ControlMotion]
    press: NotRequired[InputBinding | None]


class ControlBehaviorVariant5(TypedDict, total=False):
    kind: Required[Literal["switch"]]
    positions: Required[list[BoundPosition]]


class ControlBehaviorVariant6(TypedDict, total=False):
    kind: Required[Literal["lever"]]
    positions: Required[list[BoundPosition]]


class ControlBehaviorVariant7(TypedDict, total=False):
    kind: Required[Literal["touch"]]


class ControlBusAttachment(TypedDict, total=False):
    address: Required[int]
    channel: Required[int]
    controller_id: Required[str]


class ControlBusChip(TypedDict, total=False):
    address: Required[int]
    channel: Required[int]
    controller_id: Required[str]
    id: Required[str]
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


type ControlMotion = ControlMotionVariant0 | ControlMotionVariant1


class ControlMotionVariant0(TypedDict, total=False):
    kind: Required[Literal["positions"]]
    positions: Required[list[BoundPosition]]


class ControlMotionVariant1(TypedDict, total=False):
    decrease: Required[InputBinding]
    increase: Required[InputBinding]
    kind: Required[Literal["steps"]]


class ControlPosition(TypedDict, total=False):
    description: NotRequired[str | None]
    id: Required[str]
    label: NotRequired[str | None]


class ControlReaders(TypedDict, total=False):
    calibration: Required[int]
    scanner: Required[ScannerReader]


type ControlRole = Literal["command_control"]


type ControlSurface = Literal["top", "rear", "front", "bottom"]


class ControllerInterruptInput(TypedDict, total=False):
    line: Required[int]
    output_id: Required[str]
    source_block_id: Required[str]


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


class CursorFields(TypedDict, total=False):
    cursor: Required[int]
    filtered_count: Required[int]
    pointer: Required[int]
    raw_count: Required[int]
    subcursor: Required[int]
    value_cursor: Required[int]
    visible_count: Required[int]


class CursorLayout(TypedDict, total=False):
    fields: Required[CursorFields]
    size: Required[int]
    status_offset: Required[int]
    status_stride: Required[int]
    status_width: Required[int]


class DebugKey(TypedDict, total=False):
    event: Required[int]
    name: Required[str]


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


type ElectricalInput = (
    ElectricalInputVariant0 | ElectricalInputVariant1 | ElectricalInputVariant2
)


class ElectricalInputVariant0(TypedDict, total=False):
    callback: NotRequired[int | None]
    connection_id: Required[str]
    expected_result: NotRequired[int | None]
    intervals: Required[list[CalibrationInterval]]
    kind: Required[Literal["adc"]]


class ElectricalInputVariant1(TypedDict, total=False):
    connection_id: Required[str]
    expected_level: Required[bool]
    kind: Required[Literal["gpio"]]
    scanner: Required[GpioScanner]


class ElectricalInputVariant2(TypedDict, total=False):
    connection_id: Required[str]
    decoder_event: Required[int]
    guard: Required[int]
    kind: Required[Literal["gpio_quadrature"]]
    phase_map: Required[list[int]]
    reset: Required[list[int]]
    threshold: Required[int]
    transitions: Required[list[int]]


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


FirmwareControlIds = TypedDict(
    "FirmwareControlIds",
    {
        "$schema": Required["str"],
        "debug_keys": Required["list[DebugKey]"],
        "firmware_id": Required["str"],
        "id": Required["str"],
        "input_readers": Required["ControlReaders"],
        "inputs": Required["list[LocatedInput]"],
        "interfaces": Required["dict[str, ControlInterface]"],
        "native_key_names": Required["NativeKeyNames"],
        "scanner": Required["ScannerTable"],
        "schema": Required["int"],
    },
    total=False,
)


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


FirmwareFacts = TypedDict(
    "FirmwareFacts",
    {
        "$schema": Required["str"],
        "body_id": Required["str"],
        "configuration": Required["ConfigurationLocation"],
        "firmware_id": Required["str"],
        "firmware_sha256": Required["str"],
        "identity_checks": Required["list[AddressIdentity]"],
        "menus": Required["MenuTables"],
        "resources": Required["Resources"],
        "scanout": Required["Scanout"],
        "schema": Required["int"],
        "state": Required["FirmwareObservation"],
        "uncached_alias": Required["int"],
    },
    total=False,
)


class FirmwareGeneratedFacts(TypedDict, total=False):
    additional_ui_modes: NotRequired[AdditionalUiModes | None]
    cfgdata_location: NotRequired[ConfigurationLocation | None]
    controls: NotRequired[FirmwareControls | None]
    identity_checks: Required[list[AddressIdentity]]
    menu_cursor: NotRequired[int | None]
    menu_keys: NotRequired[MenuKeys | None]
    observation: NotRequired[FirmwareObservation | None]
    remote: NotRequired[FirmwareFacts | None]
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


class FlashSeed(TypedDict, total=False):
    offset: Required[int]
    value: Required[int]


class FocusAxis(TypedDict, total=False):
    infinity_position: Required[int]
    output: Required[OpticalQuantity]
    steps_per_mm: Required[int]


class GeneratedSceneAsset(TypedDict, total=False):
    id: Required[str]
    pattern: Required[list[int]]
    size: Required[int]


class GlyphResources(TypedDict, total=False):
    data_address_offset: Required[int]
    data_size_offset: Required[int]
    icon_index: Required[int]
    record_size: Required[int]
    registry_address: Required[int]


class GpioFeedback(TypedDict, total=False):
    input_connection_id: Required[str]
    output_connection_id: Required[str]


class GpioOutputWord(TypedDict, total=False):
    levels: Required[int]
    port: Required[int]


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


type InputBinding = InputBindingVariant0 | InputBindingVariant1


class InputBindingVariant0(TypedDict, total=False):
    kind: Required[Literal["debug_key"]]
    name: Required[str]


class InputBindingVariant1(TypedDict, total=False):
    kind: Required[Literal["native_input"]]
    name: Required[str]


type InputTarget = InputTargetVariant0 | InputTargetVariant1 | InputTargetVariant2


class InputTargetVariant0(TypedDict, total=False):
    control_id: Required[str]
    kind: Required[Literal["control"]]


class InputTargetVariant1(TypedDict, total=False):
    connection_id: Required[str]
    kind: Required[Literal["connection"]]


class InputTargetVariant2(TypedDict, total=False):
    kind: Required[Literal["unclaimed"]]


type InterruptControllerHardware = (
    InterruptControllerHardwareVariant0 | InterruptControllerHardwareVariant1
)


class InterruptControllerHardwareVariant0(TypedDict, total=False):
    cpu_interface_block_id: Required[str]
    distributor_block_id: Required[str]
    kind: Required[Literal["gic_v2"]]
    lines: Required[int]


class InterruptControllerHardwareVariant1(TypedDict, total=False):
    distributor_block_id: Required[str]
    kind: Required[Literal["gic_v3"]]
    lines: Required[int]
    redistributor_block_id: Required[str]
    redistributor_stride: Required[int]


class InterruptRoute(TypedDict, total=False):
    core: Required[str]
    line: Required[int]
    output_id: NotRequired[str | None]
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


class LocatedInput(TypedDict, total=False):
    debug_key: NotRequired[str | None]
    electrical: NotRequired[ElectricalInput | None]
    event: Required[int]
    id: Required[str]
    scanner: NotRequired[LocatedScannerRecord | None]
    target: Required[InputTarget]


class LocatedScannerRecord(TypedDict, total=False):
    index: Required[int]
    witness: Required[ScannerRecordWitness]


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


class MenuFields(TypedDict, total=False):
    flags_word: Required[int]
    handlers_pointer: Required[int]
    item_array: Required[int]
    menu_id: Required[int]
    parent_pointer: Required[int]
    rows_per_page: Required[int]
    setting_id: Required[int]
    type_code: Required[int]


class MenuKeys(TypedDict, total=False):
    back: Required[str]
    down: Required[str]
    menu: Required[str]
    up: Required[str]


class MenuLayout(TypedDict, total=False):
    caption_words: Required[int]
    fields: Required[MenuFields]
    max_kind: Required[int]
    nowrap_flags: Required[int]
    row_child: Required[int]
    row_key: Required[int]
    row_size: Required[int]
    sentinel: Required[int]
    size: Required[int]


class MenuNavigation(TypedDict, total=False):
    setup_first_tab: Required[int]
    shooting_first_tab: Required[int]
    shooting_last_tab: Required[int]


class MenuTables(TypedDict, total=False):
    cursor: Required[CursorLayout]
    cursor_address: Required[int]
    layout: Required[MenuLayout]
    navigation: Required[MenuNavigation]
    page_tabs: Required[MenuTablesVariant0]
    q_grid: Required[QuickGrid]
    roles: Required[dict[str, int]]
    root_count: Required[int]
    root_routes: Required[MenuTablesVariant1]
    root_titles: Required[MenuTablesVariant2]
    roots_address: Required[int]


type MenuTablesVariant0 = dict[str, Never]


type MenuTablesVariant1 = dict[str, Never]


type MenuTablesVariant2 = dict[str, Never]


class MicrocontrollerAssembly(TypedDict, total=False):
    channel: Required[int]
    connection_id: Required[str]
    controller_id: Required[str]
    part_id: Required[str]
    select_connection_id: Required[str]
    word_bits: Required[int]


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
    | PartModelVariant8
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
    radiometry: NotRequired[SensorRadiometry | None]
    raster: Required[SensorRaster]
    reference_white: Required[list[int]]


class PartModelVariant7(TypedDict, total=False):
    height: NotRequired[int | None]
    kind: Required[Literal["display-panel"]]
    orientation: NotRequired[DisplayOrientation | None]
    serial: Required[SerialOutputKind]
    used_mask: Required[int]
    width: NotRequired[int | None]


class PartModelVariant8(TypedDict, total=False):
    interface: Required[PeripheralPart]
    kind: Required[Literal["peripheral"]]


class PeripheralMapping(TypedDict, total=False):
    address: Required[int]
    block_id: Required[str]
    core: Required[str]
    offset: NotRequired[int | None]


type PeripheralPart = (
    PeripheralPartVariant0
    | PeripheralPartVariant1
    | PeripheralPartVariant2
    | PeripheralPartVariant3
    | PeripheralPartVariant4
    | PeripheralPartVariant5
    | PeripheralPartVariant6
    | PeripheralPartVariant7
    | PeripheralPartVariant8
    | PeripheralPartVariant9
)


class PeripheralPartVariant0(TypedDict, total=False):
    kind: Required[Literal["focus-actuator"]]


class PeripheralPartVariant1(TypedDict, total=False):
    kind: Required[Literal["iris-actuator"]]


class PeripheralPartVariant2(TypedDict, total=False):
    kind: Required[Literal["audio-serial-output"]]


class PeripheralPartVariant3(TypedDict, total=False):
    kind: Required[Literal["imaging-serial-output"]]


class PeripheralPartVariant4(TypedDict, total=False):
    kind: Required[Literal["imaging-sensor"]]
    read_gate_mask: Required[int]
    read_gate_register: Required[int]


class PeripheralPartVariant5(TypedDict, total=False):
    kind: Required[Literal["power-controller"]]


class PeripheralPartVariant6(TypedDict, total=False):
    kind: Required[Literal["serial-calibration-registers"]]


class PeripheralPartVariant7(TypedDict, total=False):
    kind: Required[Literal["bis-configuration-registers"]]


class PeripheralPartVariant8(TypedDict, total=False):
    kind: Required[Literal["lsm6ds-configuration-registers"]]


class PeripheralPartVariant9(TypedDict, total=False):
    erase_bytes: NotRequired[int | None]
    jedec_id: NotRequired[int | None]
    kind: Required[Literal["serial-nor-flash"]]
    page_bytes: NotRequired[int | None]


class PhysicalControl(TypedDict, total=False):
    assignable: Required[bool]
    behavior: NotRequired[ControlBehavior | None]
    connection_ids: Required[list[str]]
    default_function: NotRequired[str | None]
    face: NotRequired[str | None]
    icon: NotRequired[str | None]
    location: Required[ControlSurface]
    manual_page: NotRequired[int | None]
    mechanical: Required[bool]
    name: Required[str]
    role: NotRequired[ControlRole | None]


PhysicalControls = TypedDict(
    "PhysicalControls",
    {
        "$schema": Required["str"],
        "analog_levels": Required["int"],
        "body_id": Required["str"],
        "controls": Required["dict[str, PhysicalControl]"],
        "id": Required["str"],
        "manual": Required["ManualReference"],
        "schema": Required["int"],
    },
    total=False,
)


type PictureLayout = Literal["bgra8888-straight"]


type PixelFormat = Literal["bgra8888"]


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
    released: NotRequired[bool | None]
    rprofile: NotRequired[RProfileState | None]


type ProcessorModel = Literal["cortex-r5f", "cortex-a7", "cortex-a53", "cortex-m4"]


type ProcessorRegister = Literal["pc", "sp", "pstate", "cpsr"]


class PulseInterface(TypedDict, total=False):
    counter_channels: Required[list[int]]
    initial_phase: Required[int]
    quadrature: Required[bool]
    timer_channels: Required[list[int]]


class QuickGrid(TypedDict, total=False):
    columns: Required[int]
    menu_code: Required[int]
    rows: Required[int]


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


class RegisterWord(TypedDict, total=False):
    register: Required[int]
    value: Required[int]


class ResolvedConfiguration(TypedDict, total=False):
    configuration: Required[RunConfiguration]
    reference: Required[ResolvedSelection]


class ResolvedSelection(TypedDict, total=False):
    body: Required[Body]
    control_ids: NotRequired[FirmwareControlIds | None]
    firmware: Required[Firmware]
    parts: Required[list[Part]]
    physical_controls: NotRequired[PhysicalControls | None]
    soc: Required[Soc]


class Resources(TypedDict, total=False):
    bitmaps: Required[BitmapResources]
    glyph_first: Required[int]
    glyph_last: Required[int]
    glyphs: Required[GlyphResources]
    language: Required[ConfigurationLocation]
    last_caption: Required[int]
    private_text_bias: Required[int]
    private_text_first: Required[int]
    private_text_last: Required[int]
    supplementary_address: Required[int]
    supplementary_first: Required[int]
    supplementary_max_units: Required[int]
    supplementary_stride: Required[int]
    text_max_units: Required[int]
    text_metrics_offset: Required[int]
    text_pointer_offset: Required[int]
    text_record_size: Required[int]
    text_unit_bytes: Required[int]
    text_variants_offset: Required[int]


class RootRoute(TypedDict, total=False):
    entry: Required[str]
    tab: NotRequired[int | None]


class RunAdcInputs(TypedDict, total=False):
    block_id: Required[str]
    samples: Required[list[int]]


class RunBrowserCamera(TypedDict, total=False):
    preview: Required[RunPreview]
    sensor: Required[RunBrowserSensor]
    still_capture: Required[RunStillCapture]
    virtual_card: Required[RunVirtualCard]


class RunBrowserSensor(TypedDict, total=False):
    frames: Required[str]


type RunCardFilesystem = Literal["fat32"]


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
        "browser_camera": NotRequired["RunBrowserCamera | None"],
        "execution": NotRequired["RunExecution | None"],
        "firmware_id": Required["str"],
        "id": Required["str"],
        "inputs": NotRequired["RunInputs | None"],
        "overlays": Required["list[str]"],
        "purpose": Required["RunPurpose"],
        "scene": NotRequired["RunScene | None"],
        "schema": Required["int"],
        "start": Required["RunStart"],
    },
    total=False,
)


class RunConnectionState(TypedDict, total=False):
    connected: Required[bool]
    connection_id: Required[str]


class RunEncoderInput(TypedDict, total=False):
    connection_id: Required[str]
    phase: Required[int]


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
    language: NotRequired[str | None]
    max_presses: Required[int]
    settle_ticks: Required[int]


class RunGpioInputs(TypedDict, total=False):
    block_id: Required[str]
    connections: Required[list[RunConnectionState]]
    keys: Required[list[RunKeyState]]
    outputs: Required[list[RunGpioOutputState]]
    unassigned_input_level: Required[bool]


class RunGpioOutputState(TypedDict, total=False):
    connection_id: Required[str]
    high: Required[bool]
    signal_index: Required[int]


class RunImage(TypedDict, total=False):
    asset: Required[str]
    height: Required[int]
    stride: Required[int]
    width: Required[int]


class RunImages(TypedDict, total=False):
    live_image: Required[RunImage]


type RunInputs = RunInputsVariant0 | RunInputsVariant1 | RunInputsVariant2


class RunInputsVariant0(TypedDict, total=False):
    adc: Required[RunAdcInputs]
    card: Required[RunCardInput]
    encoders: Required[list[RunEncoderInput]]
    first_start: NotRequired[RunFirstStart | None]
    gpio: Required[RunGpioInputs]
    kind: Required[Literal["exr2"]]
    mcu: Required[RunMcuInput]
    power: Required[RunPowerInput]
    serial_registers: Required[list[RunSerialRegisters]]
    usb: Required[RunUsbInput]


class RunInputsVariant1(TypedDict, total=False):
    blocks: Required[list[ApplicationBlockInput]]
    kind: Required[Literal["xp4"]]
    parts: Required[list[ApplicationPartInput]]
    ticks_per_second: Required[int]
    timings: Required[list[ApplicationTiming]]


class RunInputsVariant2(TypedDict, total=False):
    blocks: Required[list[ApplicationBlockInput]]
    kind: Required[Literal["xp5"]]
    parts: Required[list[ApplicationPartInput]]
    ticks_per_second: Required[int]
    timings: Required[list[ApplicationTiming]]


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


type RunOverlayChange = (
    RunOverlayChangeVariant0
    | RunOverlayChangeVariant1
    | RunOverlayChangeVariant2
    | RunOverlayChangeVariant3
    | RunOverlayChangeVariant4
)


class RunOverlayChangeVariant0(TypedDict, total=False):
    connected: Required[bool]
    kind: Required[Literal["usb_connection"]]


class RunOverlayChangeVariant1(TypedDict, total=False):
    kind: Required[Literal["start"]]
    start: Required[RunStart]


class RunOverlayChangeVariant2(TypedDict, total=False):
    kind: Required[Literal["purpose"]]
    purpose: Required[RunPurpose]


class RunOverlayChangeVariant3(TypedDict, total=False):
    execution: Required[RunExecution]
    kind: Required[Literal["execution"]]


class RunOverlayChangeVariant4(TypedDict, total=False):
    browser_camera: Required[RunBrowserCamera]
    kind: Required[Literal["browser_camera"]]


class RunPowerInput(TypedDict, total=False):
    boot_reply: Required[int]
    part_id: Required[str]
    powered_on: Required[bool]
    spi_status: Required[list[RunRegisterValue]]


class RunPreview(TypedDict, total=False):
    active_statistics_every_frames: Required[int]
    basis: Required[str]
    frames_per_second: Required[int]
    statistics_every_frames: Required[int]
    statistics_profile: Required[str]


type RunPurpose = Literal[
    "rom_boot",
    "service_session",
    "rehearsal",
    "boot_access",
    "shutdown",
]


class RunRegisterValue(TypedDict, total=False):
    register: Required[int]
    value: Required[int]


class RunRtcInput(TypedDict, total=False):
    initial_seconds: Required[int]
    maximum_seconds: Required[int]
    started: Required[int]
    ticks_per_second: Required[int]


type RunScene = RunSceneVariant0 | RunSceneVariant1 | RunSceneVariant2


class RunSceneVariant0(TypedDict, total=False):
    frame_period_ticks: Required[int]
    generated_assets: Required[list[GeneratedSceneAsset]]
    images: Required[RunImages]
    kind: Required[Literal["exr2"]]
    statistics: Required[RunStatistics]


class RunSceneVariant1(TypedDict, total=False):
    command_images: Required[list[ApplicationCommandImageInput]]
    frames: Required[list[ApplicationFrameInput]]
    kind: Required[Literal["xp4"]]
    statistics: Required[list[ApplicationStatisticsInput]]


class RunSceneVariant2(TypedDict, total=False):
    command_images: Required[list[ApplicationCommandImageInput]]
    frames: Required[list[ApplicationFrameInput]]
    kind: Required[Literal["xp5"]]
    statistics: Required[list[ApplicationStatisticsInput]]


class RunSerialRegisters(TypedDict, total=False):
    part_id: Required[str]
    values: Required[list[RunRegisterValue]]


type RunStart = RunStartVariant0 | RunStartVariant1 | RunStartVariant2


class RunStartVariant0(TypedDict, total=False):
    kind: Required[Literal["rom"]]


class RunStartVariant1(TypedDict, total=False):
    kind: Required[Literal["loader"]]


class RunStartVariant2(TypedDict, total=False):
    kind: Required[Literal["sd_card"]]


class RunStatistics(TypedDict, total=False):
    live_statistics: Required[list[RunStatisticsSpan]]


class RunStatisticsSpan(TypedDict, total=False):
    asset: Required[str]
    destination: Required[int]
    offset: Required[int]


type RunStillCapture = Literal["register-capture-readonly-card-observation"]


class RunUsbInput(TypedDict, total=False):
    connected: Required[bool]
    connection_id: Required[str]


class RunVirtualCard(TypedDict, total=False):
    filesystem: Required[RunCardFilesystem]
    label: Required[str]
    partition_start: Required[int]
    sectors: Required[int]
    sectors_per_cluster: Required[int]


type ScannerInputKind = Literal[
    "kind_0",
    "kind_1",
    "kind_2",
    "kind_3",
    "kind_4",
    "kind_5",
]


class ScannerReader(TypedDict, total=False):
    pressed_value: Required[int]
    released_value: Required[int]
    routine: Required[AddressIdentity]
    stable_offset: Required[int]
    stable_width: Required[int]


class ScannerRecordWitness(TypedDict, total=False):
    input_kind: Required[ScannerInputKind]
    lower: Required[int]
    physical_key: Required[int]
    source: Required[int]
    upper: Required[int]


class ScannerTable(TypedDict, total=False):
    base: Required[int]
    count: Required[int]
    lookup: Required[int]
    sha256: Required[str]
    stride: Required[int]


class Scanout(TypedDict, total=False):
    address: Required[int]
    address_mask: Required[int]
    channel: Required[int]
    height: Required[int]
    instance: Required[int]
    layer: Required[int]
    pixel_format: Required[PixelFormat]
    pointer_address: Required[int]
    stride_bytes: Required[int]
    width: Required[int]


class SensorAssembly(TypedDict, total=False):
    connection_id: Required[str]
    control_bus: NotRequired[ControlBusAttachment | None]
    part_id: Required[str]


class SensorControlInterface(TypedDict, total=False):
    data_bits: Required[int]
    register_address_bits: Required[int]


type SensorIllumination = Literal["uniform", "cosine-fourth"]


class SensorProjection(TypedDict, total=False):
    focal_length_mm: Required[int]
    relative_illumination: Required[SensorIllumination]
    sensor_width_mm: Required[int]
    subject_distance_mm: Required[int]


class SensorRadiometry(TypedDict, total=False):
    period_ticks: Required[int]
    projection: Required[SensorProjection]
    rolling_references: Required[list[SensorRollingReference]]
    saturation_lux_seconds: Required[int]
    shutter_closing_delay_seconds: Required[list[tuple[int, int]]]
    shutter_output: Required[int]
    ticks_per_second: Required[int]
    white_luminance: Required[int]


class SensorRaster(TypedDict, total=False):
    height: Required[int]
    image_area: Required[list[int]]
    pixel_clock_hz: NotRequired[int | None]
    width: Required[int]


class SensorRollingReference(TypedDict, total=False):
    horizontal: Required[int]
    lines: Required[int]
    mode: Required[int]
    vertical: Required[int]


class SerialAttachment(TypedDict, total=False):
    connection_id: Required[str]
    controller_id: Required[str]
    part_id: Required[str]
    select_connection_id: NotRequired[str | None]


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
        "hardware": NotRequired["SocHardware | None"],
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
    processor_core_id: NotRequired[str | None]
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


class SocHardware(TypedDict, total=False):
    application: NotRequired[ApplicationHardware | None]
    blocks: Required[dict[str, BlockHardware]]
    companion: NotRequired[CompanionHardware | None]
    interrupt_controller: NotRequired[InterruptControllerHardware | None]
    interrupt_inputs: Required[list[ControllerInterruptInput]]


class SocReset(TypedDict, total=False):
    coprocessor_writes: Required[list[CoprocessorWrite]]
    release: NotRequired[CoreRelease | None]


class StatusRegister(TypedDict, total=False):
    address: Required[int]
    value: Required[int]


class TcmState(TypedDict, total=False):
    bytes: Required[int]
    reset_address: Required[int]


type TimingOperation = Literal[
    "settle",
    "sample",
    "command",
    "reset",
    "pixel",
    "acknowledge",
    "complete",
]


type VideoLayout = Literal["yycbcr422"]


type WireSignal = Literal[
    "gpio",
    "adc",
    "serial_data",
    "serial_clock",
    "image_data",
    "display_data",
    "microcontroller",
    "storage",
    "pulse",
    "power",
]


class ZoomMechanism(TypedDict, total=False):
    maximum_position: Required[int]
    minimum_position: Required[int]
    pulse_ticks: Required[int]

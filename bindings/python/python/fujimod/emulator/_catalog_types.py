# Generated from the Rust catalog schemas.
from typing import Literal, Never, NotRequired, Required, TypedDict, TypeAlias, Union

AddressDecoder = TypedDict(
    "AddressDecoder",
    {
        "choices": Required[list["DecoderChoice"]],
        "id": Required[str],
        "initial": Required["DecoderSelection"],
    },
    total=False,
)

AddressMapping = TypedDict(
    "AddressMapping",
    {
        "address": Required[int],
        "core": Required[str],
        "permissions": Required[int],
        "physical": Required[int],
        "size": Required["RegionSize"],
    },
    total=False,
)

BlockSignal: TypeAlias = Union[Literal["detect"], Literal["write_protect"], Literal["data_ready"], Literal["clock"]]

Body = TypedDict(
    "Body",
    {
        "$schema": Required[Literal["https://fujimod.com/schemas/body.schema.json"]],
        "card_slot": NotRequired[Union["CardSlotAssembly", None]],
        "connections": Required[list["BodyConnection"]],
        "control_bus_chips": Required[list["ControlBusChip"]],
        "display": NotRequired[Union["DisplayAssembly", None]],
        "dram_bytes": Required[int],
        "id": Required[str],
        "lens": NotRequired[Union["LensAssembly", None]],
        "microcontroller": NotRequired[Union["MicrocontrollerAssembly", None]],
        "nand": NotRequired[Union["NandGeometry", None]],
        "parts": Required[list[str]],
        "power_control": NotRequired[Union["PowerControlAssembly", None]],
        "schema": Required[int],
        "sensor": NotRequired[Union["SensorAssembly", None]],
        "soc_id": Required[str],
        "wiring": Required[list["BodyWiring"]],
    },
    total=False,
)

BodyConnection = TypedDict(
    "BodyConnection",
    {
        "id": Required[str],
        "kind": Required["ConnectionKind"],
        "signals": Required[list["ConnectionSignal"]],
    },
    total=False,
)

BodyWiring = TypedDict(
    "BodyWiring",
    {
        "signal": Required["WireSignal"],
        "source_id": Required[str],
        "target_id": Required[str],
    },
    total=False,
)

BootRomIdentity = TypedDict(
    "BootRomIdentity",
    {
        "address": Required[int],
        "content": Required["ContentIdentity"],
    },
    total=False,
)

CardSlotAssembly = TypedDict(
    "CardSlotAssembly",
    {
        "connection_id": Required[str],
        "detect_connection_id": Required[str],
        "part_id": Required[str],
        "write_protect_connection_id": Required[str],
    },
    total=False,
)

ClockDomain = TypedDict(
    "ClockDomain",
    {
        "frequency_hz": NotRequired[Union[int, None]],
        "id": Required[str],
        "parent_id": NotRequired[Union[str, None]],
    },
    total=False,
)

ConnectionKind: TypeAlias = Union[Literal["control"], Literal["usb"], Literal["hdmi"], Literal["sd-card"], Literal["remote"], Literal["hot-shoe"], Literal["external-power"], Literal["battery-door"], Literal["sensor"], Literal["lens"], Literal["display"], Literal["microcontroller"]]

ConnectionSignal: TypeAlias = Union["ConnectionSignalVariant0", "ConnectionSignalVariant1", "ConnectionSignalVariant2", "ConnectionSignalVariant3"]

ConnectionSignalVariant0 = TypedDict(
    "ConnectionSignalVariant0",
    {
        "active_level": Required[bool],
        "kind": Required[Literal["gpio"]],
        "pin": Required[int],
        "port": Required[int],
    },
    total=False,
)

ConnectionSignalVariant1 = TypedDict(
    "ConnectionSignalVariant1",
    {
        "channel": Required[int],
        "kind": Required[Literal["adc"]],
        "threshold": Required[int],
    },
    total=False,
)

ConnectionSignalVariant2 = TypedDict(
    "ConnectionSignalVariant2",
    {
        "block_id": Required[str],
        "kind": Required[Literal["block"]],
        "signal": Required["BlockSignal"],
    },
    total=False,
)

ConnectionSignalVariant3 = TypedDict(
    "ConnectionSignalVariant3",
    {
        "address": Required[int],
        "block_id": Required[str],
        "channel": Required[int],
        "kind": Required[Literal["serial"]],
    },
    total=False,
)

ContactPosition = TypedDict(
    "ContactPosition",
    {
        "contacts": Required[list[bool]],
        "drive": Required[int],
    },
    total=False,
)

ContentIdentity = TypedDict(
    "ContentIdentity",
    {
        "sha256": Required[str],
        "size": Required[int],
    },
    total=False,
)

ControlBusChip = TypedDict(
    "ControlBusChip",
    {
        "address": Required[int],
        "channel": Required[int],
        "polarity_selector": Required[int],
    },
    total=False,
)

CoprocessorWrite = TypedDict(
    "CoprocessorWrite",
    {
        "core": Required[str],
        "cp": Required[int],
        "crm": Required[int],
        "crn": Required[int],
        "opc1": Required[int],
        "opc2": Required[int],
        "secure": Required[int],
        "set_bits": Required[int],
        "wide": Required[bool],
    },
    total=False,
)

CoreRelease = TypedDict(
    "CoreRelease",
    {
        "mask": Required[int],
        "register": Required[int],
        "source": Required[int],
        "target": Required[int],
    },
    total=False,
)

DecoderChoice = TypedDict(
    "DecoderChoice",
    {
        "id": Required[str],
        "selection": Required["DecoderSelection"],
    },
    total=False,
)

DecoderSelection = TypedDict(
    "DecoderSelection",
    {
        "dma_mappings": Required[list[str]],
        "processors": Required[list[str]],
    },
    total=False,
)

DisplayAssembly = TypedDict(
    "DisplayAssembly",
    {
        "connection_id": Required[str],
        "part_id": Required[str],
        "route": NotRequired[Union["SerialRoute", None]],
    },
    total=False,
)

DisplayOrientation: TypeAlias = Union[Literal["landscape"], Literal["portrait"], Literal["tilting"]]

DmaMapping = TypedDict(
    "DmaMapping",
    {
        "address": Required[int],
        "permissions": Required[int],
        "physical": Required[int],
        "size": Required["RegionSize"],
    },
    total=False,
)

Firmware = TypedDict(
    "Firmware",
    {
        "$schema": Required[Literal["https://fujimod.com/schemas/firmware.schema.json"]],
        "body_id": Required[str],
        "boot_rom": NotRequired[Union["BootRomIdentity", None]],
        "generated_facts": NotRequired[Union["FirmwareGeneratedFacts", None]],
        "id": Required[str],
        "installation": NotRequired[Union["FirmwareInstallation", None]],
        "schema": Required[int],
        "update": NotRequired[Union["ContentIdentity", None]],
        "version": Required[str],
    },
    total=False,
)

FirmwareGeneratedFacts = TypedDict(
    "FirmwareGeneratedFacts",
    {
        "first_start": NotRequired[Union["FirstStart", None]],
        "observation": NotRequired[Union["FirmwareObservation", None]],
    },
    total=False,
)

FirmwareInstallation = TypedDict(
    "FirmwareInstallation",
    {
        "flash_offset": Required[int],
        "loader_entry": NotRequired[Union[int, None]],
    },
    total=False,
)

FirmwareObservation = TypedDict(
    "FirmwareObservation",
    {
        "menu_active": Required[int],
        "menu_node": Required[int],
        "pending_mode": Required[int],
        "readiness": Required["ReadinessAddresses"],
        "rec_mode": Required[int],
        "setup_mode": Required[int],
        "system_mode": Required[int],
        "ui_mode": Required[int],
    },
    total=False,
)

FirstStart = TypedDict(
    "FirstStart",
    {
        "adc_block_id": Required[str],
        "calibration_pointer": Required[int],
        "channel": Required[int],
        "hold_ticks": Required[int],
        "lower_offset": Required[int],
        "max_presses": Required[int],
        "settle_ticks": Required[int],
        "upper_offset": Required[int],
    },
    total=False,
)

GeneratedSceneAsset = TypedDict(
    "GeneratedSceneAsset",
    {
        "id": Required[str],
        "pattern": Required[list[int]],
        "size": Required[int],
    },
    total=False,
)

HomeSwitch = TypedDict(
    "HomeSwitch",
    {
        "active_high": Required[bool],
        "pin": Required["Pin"],
        "threshold": Required[int],
    },
    total=False,
)

InterruptRoute = TypedDict(
    "InterruptRoute",
    {
        "core": Required[str],
        "line": Required[int],
        "source_block": Required[str],
    },
    total=False,
)

LensAssembly = TypedDict(
    "LensAssembly",
    {
        "connection_id": Required[str],
        "part_id": Required[str],
        "pulse": Required["PulseInterface"],
    },
    total=False,
)

LensMotor = TypedDict(
    "LensMotor",
    {
        "home": NotRequired[Union["HomeSwitch", None]],
        "initial_position": Required[int],
        "settle_ticks": Required[int],
        "ticks_per_step": Required[int],
        "travel_limits": Required[list[int]],
    },
    total=False,
)

LensProtocol: TypeAlias = Union[Literal["banked"], Literal["three-byte"], Literal["mixed-width"], Literal["dc-zoom"]]

MProfileState = TypedDict(
    "MProfileState",
    {
        "cpuid": Required[int],
        "vector_base": Required[int],
    },
    total=False,
)

MemoryKind: TypeAlias = Union[Literal["dram"], Literal["rom"], Literal["sram"], Literal["tcm"]]

MemoryRegion = TypedDict(
    "MemoryRegion",
    {
        "base": Required[int],
        "id": Required[str],
        "kind": Required["MemoryKind"],
        "size": Required["RegionSize"],
    },
    total=False,
)

MicrocontrollerAssembly = TypedDict(
    "MicrocontrollerAssembly",
    {
        "channel": Required[int],
        "connection_id": Required[str],
        "controller_id": Required[str],
        "part_id": Required[str],
        "select_connection_id": Required[str],
    },
    total=False,
)

NandGeometry = TypedDict(
    "NandGeometry",
    {
        "bytes": Required[int],
        "page_bytes": Required[int],
        "pages_per_block": Required[int],
        "spare_bytes": Required[int],
    },
    total=False,
)

Part = TypedDict(
    "Part",
    {
        "$schema": Required[Literal["https://fujimod.com/schemas/part.schema.json"]],
        "id": Required[str],
        "model": Required["PartModel"],
        "schema": Required[int],
    },
    total=False,
)

PartEncoder = TypedDict(
    "PartEncoder",
    {
        "phase_ticks": Required[int],
        "pins": Required[list["Pin"]],
        "rest_phase": Required[int],
    },
    total=False,
)

PartModel: TypeAlias = Union["PartModelVariant0", "PartModelVariant1", "PartModelVariant2", "PartModelVariant3", "PartModelVariant4", "PartModelVariant5", "PartModelVariant6", "PartModelVariant7"]

PartModelVariant0 = TypedDict(
    "PartModelVariant0",
    {
        "encoder": NotRequired[Union["PartEncoder", None]],
        "kind": Required[Literal["lens-mechanism"]],
        "motors": Required[list["LensMotor"]],
        "protocol": Required["LensProtocol"],
        "zoom": NotRequired[Union["ZoomMechanism", None]],
    },
    total=False,
)

PartModelVariant1 = TypedDict(
    "PartModelVariant1",
    {
        "contact_pins": Required[list["Pin"]],
        "drive_pins": Required[list["Pin"]],
        "kind": Required[Literal["contact-actuator"]],
        "positions": Required[list["ContactPosition"]],
        "settle_ticks": Required[int],
    },
    total=False,
)

PartModelVariant2 = TypedDict(
    "PartModelVariant2",
    {
        "endpoint": Required["SerialOutputKind"],
        "kind": Required[Literal["serial-output"]],
    },
    total=False,
)

PartModelVariant3 = TypedDict(
    "PartModelVariant3",
    {
        "kind": Required[Literal["microcontroller"]],
        "registers": Required["RegisterRange"],
    },
    total=False,
)

PartModelVariant4 = TypedDict(
    "PartModelVariant4",
    {
        "kind": Required[Literal["power-control"]],
        "status_registers": Required[list["StatusRegister"]],
    },
    total=False,
)

PartModelVariant5 = TypedDict(
    "PartModelVariant5",
    {
        "kind": Required[Literal["sd-card-slot"]],
    },
    total=False,
)

PartModelVariant6 = TypedDict(
    "PartModelVariant6",
    {
        "black_level": Required[int],
        "color_filter": Required[list[int]],
        "control": Required["SensorControlInterface"],
        "full_scale": Required[int],
        "kind": Required[Literal["image-sensor"]],
        "raster": Required["SensorRaster"],
        "reference_white": Required[list[int]],
    },
    total=False,
)

PartModelVariant7 = TypedDict(
    "PartModelVariant7",
    {
        "height": Required[int],
        "kind": Required[Literal["display-panel"]],
        "orientation": Required["DisplayOrientation"],
        "serial": Required["SerialOutputKind"],
        "used_mask": Required[int],
        "width": Required[int],
    },
    total=False,
)

PeripheralMapping = TypedDict(
    "PeripheralMapping",
    {
        "address": Required[int],
        "block_id": Required[str],
        "core": Required[str],
        "physical": Required[int],
        "size": Required[int],
    },
    total=False,
)

Pin = TypedDict(
    "Pin",
    {
        "pin": Required[int],
        "port": Required[int],
    },
    total=False,
)

PowerControlAssembly = TypedDict(
    "PowerControlAssembly",
    {
        "adc_block_id": Required[str],
        "adc_channel": Required[int],
        "connection_id": Required[str],
        "controller_id": Required[str],
        "part_id": Required[str],
    },
    total=False,
)

ProcessorArchitecture: TypeAlias = Union[Literal["arm32"], Literal["aarch64"]]

ProcessorCore = TypedDict(
    "ProcessorCore",
    {
        "id": Required[str],
        "model": Required["ProcessorModel"],
        "mprofile": NotRequired[Union["MProfileState", None]],
        "registers": Required[list["RegisterValue"]],
        "released": Required[bool],
        "rprofile": NotRequired[Union["RProfileState", None]],
    },
    total=False,
)

ProcessorModel: TypeAlias = Union[Literal["cortex-r5f"], Literal["cortex-a7"], Literal["cortex-a53"]]

ProcessorRegister: TypeAlias = Union[Literal["pc"], Literal["sp"], Literal["pstate"], Literal["cpsr"]]

PulseInterface = TypedDict(
    "PulseInterface",
    {
        "counter_channels": Required[list[int]],
        "initial_phase": Required[int],
        "quadrature": Required[bool],
        "timer_channels": Required[list[int]],
    },
    total=False,
)

RProfileState = TypedDict(
    "RProfileState",
    {
        "atcm": Required["TcmState"],
        "btcm": Required["TcmState"],
        "cp14_identity": Required[int],
    },
    total=False,
)

ReadinessAddresses = TypedDict(
    "ReadinessAddresses",
    {
        "category_one": Required[int],
        "category_two": Required[int],
        "excluded_mask": Required[int],
        "input_pending": Required[int],
        "operation_alert_timer": Required[int],
        "override_address": Required[int],
    },
    total=False,
)

RegionSize: TypeAlias = Union["RegionSizeVariant0", "RegionSizeVariant1"]

RegionSizeVariant0 = TypedDict(
    "RegionSizeVariant0",
    {
        "bytes": Required[int],
        "kind": Required[Literal["bytes"]],
    },
    total=False,
)

RegionSizeVariant1 = TypedDict(
    "RegionSizeVariant1",
    {
        "kind": Required[Literal["body_dram"]],
    },
    total=False,
)

RegisterRange = TypedDict(
    "RegisterRange",
    {
        "count": Required[int],
        "first": Required[int],
        "width_bytes": Required[int],
    },
    total=False,
)

RegisterValue = TypedDict(
    "RegisterValue",
    {
        "register": Required["ProcessorRegister"],
        "value": Required[int],
    },
    total=False,
)

ResolvedSelection = TypedDict(
    "ResolvedSelection",
    {
        "$schema": NotRequired[Literal["https://fujimod.com/schemas/resolved-selection.schema.json"]],
        "body": Required["Body"],
        "firmware": Required["Firmware"],
        "parts": Required[list["Part"]],
        "soc": Required["Soc"],
    },
    total=False,
)

RunAdcInputs = TypedDict(
    "RunAdcInputs",
    {
        "block_id": Required[str],
        "samples": Required[list[int]],
    },
    total=False,
)

RunCardInput = TypedDict(
    "RunCardInput",
    {
        "part_id": Required[str],
        "present": Required[bool],
        "write_protected": Required[bool],
    },
    total=False,
)

RunConfiguration = TypedDict(
    "RunConfiguration",
    {
        "$schema": Required[Literal["https://fujimod.com/schemas/emulator-run-configuration.schema.json"]],
        "body_id": Required[str],
        "execution": Required["RunExecution"],
        "firmware_id": Required[str],
        "id": Required[str],
        "inputs": Required["RunInputs"],
        "overlays": Required[list[str]],
        "scene": Required["RunScene"],
        "schema": Required[int],
        "start": Required["RunStart"],
    },
    total=False,
)

RunConnectionState = TypedDict(
    "RunConnectionState",
    {
        "connected": Required[bool],
        "connection_id": Required[str],
    },
    total=False,
)

RunEngine: TypeAlias = Literal["unicorn"]

RunExecution = TypedDict(
    "RunExecution",
    {
        "engine": Required["RunEngine"],
        "instructions_per_tick": Required["RunInstructionRate"],
        "mode": Required["RunExecutionMode"],
        "quantum": Required[int],
    },
    total=False,
)

RunExecutionMode: TypeAlias = Literal["strict"]

RunGpioInputs = TypedDict(
    "RunGpioInputs",
    {
        "block_id": Required[str],
        "connections": Required[list["RunConnectionState"]],
        "keys": Required[list["RunKeyState"]],
    },
    total=False,
)

RunImage = TypedDict(
    "RunImage",
    {
        "asset": Required[str],
        "height": Required[int],
        "stride": Required[int],
        "width": Required[int],
    },
    total=False,
)

RunImages = TypedDict(
    "RunImages",
    {
        "live_image": Required["RunImage"],
    },
    total=False,
)

RunInputs = TypedDict(
    "RunInputs",
    {
        "adc": Required["RunAdcInputs"],
        "card": Required["RunCardInput"],
        "gpio": Required["RunGpioInputs"],
        "mcu": Required["RunMcuInput"],
        "power": Required["RunPowerInput"],
        "serial_registers": Required[list["RunSerialRegisters"]],
        "usb": Required["RunUsbInput"],
    },
    total=False,
)

RunInstructionRate = TypedDict(
    "RunInstructionRate",
    {
        "denominator": Required[int],
        "numerator": Required[int],
    },
    total=False,
)

RunKeyState = TypedDict(
    "RunKeyState",
    {
        "connection_id": Required[str],
        "pressed": Required[bool],
    },
    total=False,
)

RunMcuInput = TypedDict(
    "RunMcuInput",
    {
        "packet": Required[list[int]],
        "part_id": Required[str],
        "rtc": Required["RunRtcInput"],
    },
    total=False,
)

RunOverlay = TypedDict(
    "RunOverlay",
    {
        "$schema": Required[Literal["https://fujimod.com/schemas/emulator-run-overlay.schema.json"]],
        "change": Required["RunOverlayChange"],
        "configuration_id": Required[str],
        "id": Required[str],
        "schema": Required[int],
    },
    total=False,
)

RunOverlayChange: TypeAlias = Union["RunOverlayChangeVariant0", "RunOverlayChangeVariant1"]

RunOverlayChangeVariant0 = TypedDict(
    "RunOverlayChangeVariant0",
    {
        "connected": Required[bool],
        "kind": Required[Literal["usb_connection"]],
    },
    total=False,
)

RunOverlayChangeVariant1 = TypedDict(
    "RunOverlayChangeVariant1",
    {
        "kind": Required[Literal["start"]],
        "start": Required["RunStart"],
    },
    total=False,
)

RunPowerInput = TypedDict(
    "RunPowerInput",
    {
        "boot_reply": Required[int],
        "part_id": Required[str],
        "powered_on": Required[bool],
        "spi_status": Required[list["RunRegisterValue"]],
    },
    total=False,
)

RunRegisterValue = TypedDict(
    "RunRegisterValue",
    {
        "register": Required[int],
        "value": Required[int],
    },
    total=False,
)

RunRtcInput = TypedDict(
    "RunRtcInput",
    {
        "initial_seconds": Required[int],
        "maximum_seconds": Required[int],
        "started": Required[int],
        "ticks_per_second": Required[int],
    },
    total=False,
)

RunScene = TypedDict(
    "RunScene",
    {
        "frame_period_ticks": Required[int],
        "generated_assets": Required[list["GeneratedSceneAsset"]],
        "images": Required["RunImages"],
        "statistics": Required["RunStatistics"],
    },
    total=False,
)

RunSerialRegisters = TypedDict(
    "RunSerialRegisters",
    {
        "part_id": Required[str],
        "values": Required[list["RunRegisterValue"]],
    },
    total=False,
)

RunStart: TypeAlias = Union["RunStartVariant0", "RunStartVariant1"]

RunStartVariant0 = TypedDict(
    "RunStartVariant0",
    {
        "kind": Required[Literal["rom"]],
    },
    total=False,
)

RunStartVariant1 = TypedDict(
    "RunStartVariant1",
    {
        "kind": Required[Literal["loader"]],
    },
    total=False,
)

RunStatistics = TypedDict(
    "RunStatistics",
    {
        "live_statistics": Required[list["RunStatisticsSpan"]],
    },
    total=False,
)

RunStatisticsSpan = TypedDict(
    "RunStatisticsSpan",
    {
        "asset": Required[str],
        "destination": Required[int],
        "offset": Required[int],
    },
    total=False,
)

RunUsbInput = TypedDict(
    "RunUsbInput",
    {
        "connected": Required[bool],
        "connection_id": Required[str],
    },
    total=False,
)

SensorAssembly = TypedDict(
    "SensorAssembly",
    {
        "channel": Required[int],
        "connection_id": Required[str],
        "control": Required["SensorControlInterface"],
        "controller_id": Required[str],
        "part_id": Required[str],
    },
    total=False,
)

SensorControlInterface = TypedDict(
    "SensorControlInterface",
    {
        "address": Required[int],
        "data_bits": Required[int],
        "register_address_bits": Required[int],
    },
    total=False,
)

SensorRaster = TypedDict(
    "SensorRaster",
    {
        "height": Required[int],
        "image_area": Required[list[int]],
        "pixel_clock_hz": NotRequired[Union[int, None]],
        "width": Required[int],
    },
    total=False,
)

SerialOutputKind: TypeAlias = Union[Literal["panel"], Literal["registers"], Literal["command_data"], Literal["fixed"]]

SerialRoute = TypedDict(
    "SerialRoute",
    {
        "channel": Required[int],
        "clock_hz": NotRequired[Union[int, None]],
        "controller_id": Required[str],
        "endpoint_id": Required[str],
        "select_active_high": Required[bool],
        "select_pin": Required["Pin"],
        "word_bits": Required[int],
    },
    total=False,
)

Soc = TypedDict(
    "Soc",
    {
        "$schema": Required[Literal["https://fujimod.com/schemas/soc.schema.json"]],
        "architecture": Required["ProcessorArchitecture"],
        "blocks": Required[list["SocBlock"]],
        "clocks": Required[list["ClockDomain"]],
        "decoders": Required[list["AddressDecoder"]],
        "dma_mappings": Required[list["DmaMapping"]],
        "id": Required[str],
        "interrupts": Required[list["InterruptRoute"]],
        "mappings": Required[list["AddressMapping"]],
        "memory_regions": Required[list["MemoryRegion"]],
        "peripherals": Required[list["PeripheralMapping"]],
        "processors": Required[list["ProcessorCore"]],
        "reset": Required["SocReset"],
        "schema": Required[int],
    },
    total=False,
)

SocBlock = TypedDict(
    "SocBlock",
    {
        "base": Required[int],
        "clock_id": NotRequired[Union[str, None]],
        "id": Required[str],
        "interrupt_lines": Required[list[int]],
        "kind": Required["SocBlockKind"],
        "size": Required[int],
    },
    total=False,
)

SocBlockKind: TypeAlias = Union[Literal["gpio"], Literal["adc"], Literal["interrupt-controller"], Literal["timer"], Literal["dma"], Literal["control-bus"], Literal["serial-controller"], Literal["nand-controller"], Literal["usb-controller"], Literal["display-controller"], Literal["imaging-controller"], Literal["reset-controller"], Literal["clock-controller"]]

SocReset = TypedDict(
    "SocReset",
    {
        "coprocessor_writes": Required[list["CoprocessorWrite"]],
        "release": NotRequired[Union["CoreRelease", None]],
    },
    total=False,
)

StatusRegister = TypedDict(
    "StatusRegister",
    {
        "address": Required[int],
        "value": Required[int],
    },
    total=False,
)

TcmState = TypedDict(
    "TcmState",
    {
        "bytes": Required[int],
        "reset_address": Required[int],
    },
    total=False,
)

WireSignal: TypeAlias = Union[Literal["gpio"], Literal["adc"], Literal["serial_data"], Literal["serial_clock"], Literal["image_data"], Literal["display_data"], Literal["microcontroller"], Literal["storage"], Literal["power"]]

ZoomMechanism = TypedDict(
    "ZoomMechanism",
    {
        "maximum_position": Required[int],
        "minimum_position": Required[int],
        "pulse_ticks": Required[int],
    },
    total=False,
)


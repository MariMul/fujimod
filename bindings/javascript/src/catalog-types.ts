// Generated from the Rust catalog schemas.

export type AddressDecoder = {
  readonly "choices": readonly DecoderChoice[];
  readonly "id": string;
  readonly "initial": DecoderSelection;
};

export type AddressMapping = {
  readonly "address": number;
  readonly "core": string;
  readonly "permissions": number;
  readonly "physical": number;
  readonly "size": RegionSize;
};

export type BlockSignal = "detect" | "write_protect" | "data_ready" | "clock";

export type Body = {
  readonly "$schema": "https://fujimod.com/schemas/body.schema.json";
  readonly "card_slot"?: CardSlotAssembly | null;
  readonly "connections": readonly BodyConnection[];
  readonly "control_bus_chips": readonly ControlBusChip[];
  readonly "display"?: DisplayAssembly | null;
  readonly "dram_bytes": number;
  readonly "id": string;
  readonly "lens"?: LensAssembly | null;
  readonly "microcontroller"?: MicrocontrollerAssembly | null;
  readonly "nand"?: NandGeometry | null;
  readonly "parts": readonly string[];
  readonly "power_control"?: PowerControlAssembly | null;
  readonly "schema": number;
  readonly "sensor"?: SensorAssembly | null;
  readonly "soc_id": string;
  readonly "wiring": readonly BodyWiring[];
};

export type BodyConnection = {
  readonly "id": string;
  readonly "kind": ConnectionKind;
  readonly "signals": readonly ConnectionSignal[];
};

export type BodyWiring = {
  readonly "signal": WireSignal;
  readonly "source_id": string;
  readonly "target_id": string;
};

export type BootRomIdentity = {
  readonly "address": number;
  readonly "content": ContentIdentity;
};

export type CardSlotAssembly = {
  readonly "connection_id": string;
  readonly "detect_connection_id": string;
  readonly "part_id": string;
  readonly "write_protect_connection_id": string;
};

export type ClockDomain = {
  readonly "frequency_hz"?: number | null;
  readonly "id": string;
  readonly "parent_id"?: string | null;
};

export type ConnectionKind = "control" | "usb" | "hdmi" | "sd-card" | "remote" | "hot-shoe" | "external-power" | "battery-door" | "sensor" | "lens" | "display" | "microcontroller";

export type ConnectionSignal = ConnectionSignalVariant0 | ConnectionSignalVariant1 | ConnectionSignalVariant2 | ConnectionSignalVariant3;

export type ConnectionSignalVariant0 = {
  readonly "active_level": boolean;
  readonly "kind": "gpio";
  readonly "pin": number;
  readonly "port": number;
};

export type ConnectionSignalVariant1 = {
  readonly "channel": number;
  readonly "kind": "adc";
  readonly "threshold": number;
};

export type ConnectionSignalVariant2 = {
  readonly "block_id": string;
  readonly "kind": "block";
  readonly "signal": BlockSignal;
};

export type ConnectionSignalVariant3 = {
  readonly "address": number;
  readonly "block_id": string;
  readonly "channel": number;
  readonly "kind": "serial";
};

export type ContactPosition = {
  readonly "contacts": readonly boolean[];
  readonly "drive": number;
};

export type ContentIdentity = {
  readonly "sha256": string;
  readonly "size": number;
};

export type ControlBusChip = {
  readonly "address": number;
  readonly "channel": number;
  readonly "polarity_selector": number;
};

export type CoprocessorWrite = {
  readonly "core": string;
  readonly "cp": number;
  readonly "crm": number;
  readonly "crn": number;
  readonly "opc1": number;
  readonly "opc2": number;
  readonly "secure": number;
  readonly "set_bits": number;
  readonly "wide": boolean;
};

export type CoreRelease = {
  readonly "mask": number;
  readonly "register": number;
  readonly "source": number;
  readonly "target": number;
};

export type DecoderChoice = {
  readonly "id": string;
  readonly "selection": DecoderSelection;
};

export type DecoderSelection = {
  readonly "dma_mappings": readonly string[];
  readonly "processors": readonly string[];
};

export type DisplayAssembly = {
  readonly "connection_id": string;
  readonly "part_id": string;
  readonly "route"?: SerialRoute | null;
};

export type DisplayOrientation = "landscape" | "portrait" | "tilting";

export type DmaMapping = {
  readonly "address": number;
  readonly "permissions": number;
  readonly "physical": number;
  readonly "size": RegionSize;
};

export type Firmware = {
  readonly "$schema": "https://fujimod.com/schemas/firmware.schema.json";
  readonly "body_id": string;
  readonly "boot_rom"?: BootRomIdentity | null;
  readonly "generated_facts"?: FirmwareGeneratedFacts | null;
  readonly "id": string;
  readonly "installation"?: FirmwareInstallation | null;
  readonly "schema": number;
  readonly "update"?: ContentIdentity | null;
  readonly "version": string;
};

export type FirmwareGeneratedFacts = {
  readonly "first_start"?: FirstStart | null;
  readonly "observation"?: FirmwareObservation | null;
};

export type FirmwareInstallation = {
  readonly "flash_offset": number;
  readonly "loader_entry"?: number | null;
};

export type FirmwareObservation = {
  readonly "menu_active": number;
  readonly "menu_node": number;
  readonly "pending_mode": number;
  readonly "readiness": ReadinessAddresses;
  readonly "rec_mode": number;
  readonly "setup_mode": number;
  readonly "system_mode": number;
  readonly "ui_mode": number;
};

export type FirstStart = {
  readonly "adc_block_id": string;
  readonly "calibration_pointer": number;
  readonly "channel": number;
  readonly "hold_ticks": number;
  readonly "lower_offset": number;
  readonly "max_presses": number;
  readonly "settle_ticks": number;
  readonly "upper_offset": number;
};

export type GeneratedSceneAsset = {
  readonly "id": string;
  readonly "pattern": readonly number[];
  readonly "size": number;
};

export type HomeSwitch = {
  readonly "active_high": boolean;
  readonly "pin": Pin;
  readonly "threshold": number;
};

export type InterruptRoute = {
  readonly "core": string;
  readonly "line": number;
  readonly "source_block": string;
};

export type LensAssembly = {
  readonly "connection_id": string;
  readonly "part_id": string;
  readonly "pulse": PulseInterface;
};

export type LensMotor = {
  readonly "home"?: HomeSwitch | null;
  readonly "initial_position": number;
  readonly "settle_ticks": number;
  readonly "ticks_per_step": number;
  readonly "travel_limits": readonly number[];
};

export type LensProtocol = "banked" | "three-byte" | "mixed-width" | "dc-zoom";

export type MProfileState = {
  readonly "cpuid": number;
  readonly "vector_base": number;
};

export type MemoryKind = "dram" | "rom" | "sram" | "tcm";

export type MemoryRegion = {
  readonly "base": number;
  readonly "id": string;
  readonly "kind": MemoryKind;
  readonly "size": RegionSize;
};

export type MicrocontrollerAssembly = {
  readonly "channel": number;
  readonly "connection_id": string;
  readonly "controller_id": string;
  readonly "part_id": string;
  readonly "select_connection_id": string;
};

export type NandGeometry = {
  readonly "bytes": number;
  readonly "page_bytes": number;
  readonly "pages_per_block": number;
  readonly "spare_bytes": number;
};

export type Part = {
  readonly "$schema": "https://fujimod.com/schemas/part.schema.json";
  readonly "id": string;
  readonly "model": PartModel;
  readonly "schema": number;
};

export type PartEncoder = {
  readonly "phase_ticks": number;
  readonly "pins": readonly Pin[];
  readonly "rest_phase": number;
};

export type PartModel = PartModelVariant0 | PartModelVariant1 | PartModelVariant2 | PartModelVariant3 | PartModelVariant4 | PartModelVariant5 | PartModelVariant6 | PartModelVariant7;

export type PartModelVariant0 = {
  readonly "encoder"?: PartEncoder | null;
  readonly "kind": "lens-mechanism";
  readonly "motors": readonly LensMotor[];
  readonly "protocol": LensProtocol;
  readonly "zoom"?: ZoomMechanism | null;
};

export type PartModelVariant1 = {
  readonly "contact_pins": readonly Pin[];
  readonly "drive_pins": readonly Pin[];
  readonly "kind": "contact-actuator";
  readonly "positions": readonly ContactPosition[];
  readonly "settle_ticks": number;
};

export type PartModelVariant2 = {
  readonly "endpoint": SerialOutputKind;
  readonly "kind": "serial-output";
};

export type PartModelVariant3 = {
  readonly "kind": "microcontroller";
  readonly "registers": RegisterRange;
};

export type PartModelVariant4 = {
  readonly "kind": "power-control";
  readonly "status_registers": readonly StatusRegister[];
};

export type PartModelVariant5 = {
  readonly "kind": "sd-card-slot";
};

export type PartModelVariant6 = {
  readonly "black_level": number;
  readonly "color_filter": readonly number[];
  readonly "control": SensorControlInterface;
  readonly "full_scale": number;
  readonly "kind": "image-sensor";
  readonly "raster": SensorRaster;
  readonly "reference_white": readonly number[];
};

export type PartModelVariant7 = {
  readonly "height": number;
  readonly "kind": "display-panel";
  readonly "orientation": DisplayOrientation;
  readonly "serial": SerialOutputKind;
  readonly "used_mask": number;
  readonly "width": number;
};

export type PeripheralMapping = {
  readonly "address": number;
  readonly "block_id": string;
  readonly "core": string;
  readonly "physical": number;
  readonly "size": number;
};

export type Pin = {
  readonly "pin": number;
  readonly "port": number;
};

export type PowerControlAssembly = {
  readonly "adc_block_id": string;
  readonly "adc_channel": number;
  readonly "connection_id": string;
  readonly "controller_id": string;
  readonly "part_id": string;
};

export type ProcessorArchitecture = "arm32" | "aarch64";

export type ProcessorCore = {
  readonly "id": string;
  readonly "model": ProcessorModel;
  readonly "mprofile"?: MProfileState | null;
  readonly "registers": readonly RegisterValue[];
  readonly "released": boolean;
  readonly "rprofile"?: RProfileState | null;
};

export type ProcessorModel = "cortex-r5f" | "cortex-a7" | "cortex-a53";

export type ProcessorRegister = "pc" | "sp" | "pstate" | "cpsr";

export type PulseInterface = {
  readonly "counter_channels": readonly number[];
  readonly "initial_phase": number;
  readonly "quadrature": boolean;
  readonly "timer_channels": readonly number[];
};

export type RProfileState = {
  readonly "atcm": TcmState;
  readonly "btcm": TcmState;
  readonly "cp14_identity": number;
};

export type ReadinessAddresses = {
  readonly "category_one": number;
  readonly "category_two": number;
  readonly "excluded_mask": number;
  readonly "input_pending": number;
  readonly "operation_alert_timer": number;
  readonly "override_address": number;
};

export type RegionSize = RegionSizeVariant0 | RegionSizeVariant1;

export type RegionSizeVariant0 = {
  readonly "bytes": number;
  readonly "kind": "bytes";
};

export type RegionSizeVariant1 = {
  readonly "kind": "body_dram";
};

export type RegisterRange = {
  readonly "count": number;
  readonly "first": number;
  readonly "width_bytes": number;
};

export type RegisterValue = {
  readonly "register": ProcessorRegister;
  readonly "value": number;
};

export type ResolvedSelection = {
  readonly "$schema"?: "https://fujimod.com/schemas/resolved-selection.schema.json";
  readonly "body": Body;
  readonly "firmware": Firmware;
  readonly "parts": readonly Part[];
  readonly "soc": Soc;
};

export type RunAdcInputs = {
  readonly "block_id": string;
  readonly "samples": readonly number[];
};

export type RunCardInput = {
  readonly "part_id": string;
  readonly "present": boolean;
  readonly "write_protected": boolean;
};

export type RunConfiguration = {
  readonly "$schema": "https://fujimod.com/schemas/emulator-run-configuration.schema.json";
  readonly "body_id": string;
  readonly "execution": RunExecution;
  readonly "firmware_id": string;
  readonly "id": string;
  readonly "inputs": RunInputs;
  readonly "overlays": readonly string[];
  readonly "scene": RunScene;
  readonly "schema": number;
  readonly "start": RunStart;
};

export type RunConnectionState = {
  readonly "connected": boolean;
  readonly "connection_id": string;
};

export type RunEngine = "unicorn";

export type RunExecution = {
  readonly "engine": RunEngine;
  readonly "instructions_per_tick": RunInstructionRate;
  readonly "mode": RunExecutionMode;
  readonly "quantum": number;
};

export type RunExecutionMode = "strict";

export type RunGpioInputs = {
  readonly "block_id": string;
  readonly "connections": readonly RunConnectionState[];
  readonly "keys": readonly RunKeyState[];
};

export type RunImage = {
  readonly "asset": string;
  readonly "height": number;
  readonly "stride": number;
  readonly "width": number;
};

export type RunImages = {
  readonly "live_image": RunImage;
};

export type RunInputs = {
  readonly "adc": RunAdcInputs;
  readonly "card": RunCardInput;
  readonly "gpio": RunGpioInputs;
  readonly "mcu": RunMcuInput;
  readonly "power": RunPowerInput;
  readonly "serial_registers": readonly RunSerialRegisters[];
  readonly "usb": RunUsbInput;
};

export type RunInstructionRate = {
  readonly "denominator": number;
  readonly "numerator": number;
};

export type RunKeyState = {
  readonly "connection_id": string;
  readonly "pressed": boolean;
};

export type RunMcuInput = {
  readonly "packet": readonly number[];
  readonly "part_id": string;
  readonly "rtc": RunRtcInput;
};

export type RunOverlay = {
  readonly "$schema": "https://fujimod.com/schemas/emulator-run-overlay.schema.json";
  readonly "change": RunOverlayChange;
  readonly "configuration_id": string;
  readonly "id": string;
  readonly "schema": number;
};

export type RunOverlayChange = RunOverlayChangeVariant0 | RunOverlayChangeVariant1;

export type RunOverlayChangeVariant0 = {
  readonly "connected": boolean;
  readonly "kind": "usb_connection";
};

export type RunOverlayChangeVariant1 = {
  readonly "kind": "start";
  readonly "start": RunStart;
};

export type RunPowerInput = {
  readonly "boot_reply": number;
  readonly "part_id": string;
  readonly "powered_on": boolean;
  readonly "spi_status": readonly RunRegisterValue[];
};

export type RunRegisterValue = {
  readonly "register": number;
  readonly "value": number;
};

export type RunRtcInput = {
  readonly "initial_seconds": number;
  readonly "maximum_seconds": number;
  readonly "started": number;
  readonly "ticks_per_second": number;
};

export type RunScene = {
  readonly "frame_period_ticks": number;
  readonly "generated_assets": readonly GeneratedSceneAsset[];
  readonly "images": RunImages;
  readonly "statistics": RunStatistics;
};

export type RunSerialRegisters = {
  readonly "part_id": string;
  readonly "values": readonly RunRegisterValue[];
};

export type RunStart = RunStartVariant0 | RunStartVariant1;

export type RunStartVariant0 = {
  readonly "kind": "rom";
};

export type RunStartVariant1 = {
  readonly "kind": "loader";
};

export type RunStatistics = {
  readonly "live_statistics": readonly RunStatisticsSpan[];
};

export type RunStatisticsSpan = {
  readonly "asset": string;
  readonly "destination": number;
  readonly "offset": number;
};

export type RunUsbInput = {
  readonly "connected": boolean;
  readonly "connection_id": string;
};

export type SensorAssembly = {
  readonly "channel": number;
  readonly "connection_id": string;
  readonly "control": SensorControlInterface;
  readonly "controller_id": string;
  readonly "part_id": string;
};

export type SensorControlInterface = {
  readonly "address": number;
  readonly "data_bits": number;
  readonly "register_address_bits": number;
};

export type SensorRaster = {
  readonly "height": number;
  readonly "image_area": readonly number[];
  readonly "pixel_clock_hz"?: number | null;
  readonly "width": number;
};

export type SerialOutputKind = "panel" | "registers" | "command_data" | "fixed";

export type SerialRoute = {
  readonly "channel": number;
  readonly "clock_hz"?: number | null;
  readonly "controller_id": string;
  readonly "endpoint_id": string;
  readonly "select_active_high": boolean;
  readonly "select_pin": Pin;
  readonly "word_bits": number;
};

export type Soc = {
  readonly "$schema": "https://fujimod.com/schemas/soc.schema.json";
  readonly "architecture": ProcessorArchitecture;
  readonly "blocks": readonly SocBlock[];
  readonly "clocks": readonly ClockDomain[];
  readonly "decoders": readonly AddressDecoder[];
  readonly "dma_mappings": readonly DmaMapping[];
  readonly "id": string;
  readonly "interrupts": readonly InterruptRoute[];
  readonly "mappings": readonly AddressMapping[];
  readonly "memory_regions": readonly MemoryRegion[];
  readonly "peripherals": readonly PeripheralMapping[];
  readonly "processors": readonly ProcessorCore[];
  readonly "reset": SocReset;
  readonly "schema": number;
};

export type SocBlock = {
  readonly "base": number;
  readonly "clock_id"?: string | null;
  readonly "id": string;
  readonly "interrupt_lines": readonly number[];
  readonly "kind": SocBlockKind;
  readonly "size": number;
};

export type SocBlockKind = "gpio" | "adc" | "interrupt-controller" | "timer" | "dma" | "control-bus" | "serial-controller" | "nand-controller" | "usb-controller" | "display-controller" | "imaging-controller" | "reset-controller" | "clock-controller";

export type SocReset = {
  readonly "coprocessor_writes": readonly CoprocessorWrite[];
  readonly "release"?: CoreRelease | null;
};

export type StatusRegister = {
  readonly "address": number;
  readonly "value": number;
};

export type TcmState = {
  readonly "bytes": number;
  readonly "reset_address": number;
};

export type WireSignal = "gpio" | "adc" | "serial_data" | "serial_clock" | "image_data" | "display_data" | "microcontroller" | "storage" | "power";

export type ZoomMechanism = {
  readonly "maximum_position": number;
  readonly "minimum_position": number;
  readonly "pulse_ticks": number;
};


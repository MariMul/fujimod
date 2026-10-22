// Generated from the Rust catalog schemas.

export type AdditionalUiModes = {
  readonly "play": number;
  readonly "setup_play": number;
};

export type AddressDecoder = {
  readonly "choices": readonly DecoderChoice[];
  readonly "id": string;
  readonly "initial": DecoderSelection;
};

export type AddressIdentity = {
  readonly "address": number;
  readonly "sha256": string;
  readonly "size": number;
};

export type AddressMapping = {
  readonly "address": number;
  readonly "core": string;
  readonly "id": string;
  readonly "permissions": number;
  readonly "physical": number;
  readonly "size": RegionSize;
};

export type AperturePosition = {
  readonly "f_number": number;
  readonly "position": number;
};

export type ApplicationBlockInput = ApplicationBlockInputVariant0 | ApplicationBlockInputVariant1 | ApplicationBlockInputVariant2 | ApplicationBlockInputVariant3 | ApplicationBlockInputVariant4;

export type ApplicationBlockInputVariant0 = {
  readonly "block_id": string;
  readonly "kind": "adc";
  readonly "samples": readonly number[];
};

export type ApplicationBlockInputVariant1 = {
  readonly "block_id": string;
  readonly "kind": "gpio";
  readonly "levels": readonly number[];
  readonly "outputs": readonly GpioOutputWord[];
};

export type ApplicationBlockInputVariant2 = {
  readonly "block_id": string;
  readonly "kind": "temperature";
  readonly "millicelsius": number;
};

export type ApplicationBlockInputVariant3 = {
  readonly "block_id": string;
  readonly "idle_high": boolean;
  readonly "kind": "serial_line";
};

export type ApplicationBlockInputVariant4 = {
  readonly "block_id": string;
  readonly "erased_tail"?: number | null;
  readonly "kind": "flash";
  readonly "part_id": string;
  readonly "seeds": readonly FlashSeed[];
  readonly "source": ApplicationFlashSource;
};

export type ApplicationCommandImageInput = {
  readonly "block_ids": readonly string[];
  readonly "luma": number;
};

export type ApplicationFlashSource = "firmware_update";

export type ApplicationFrameInput = {
  readonly "active_ticks"?: number | null;
  readonly "block_ids": readonly string[];
  readonly "luma"?: number | null;
  readonly "period_ticks": number;
};

export type ApplicationHardware = ApplicationHardwareVariant0 | ApplicationHardwareVariant1;

export type ApplicationHardwareVariant0 = {
  readonly "caches": CacheGeometry;
  readonly "configuration_base": number;
  readonly "kind": "armv7a";
};

export type ApplicationHardwareVariant1 = {
  readonly "affinities": Record<string, number>;
  readonly "counter_clock_id": string;
  readonly "kind": "armv8a";
  readonly "midr_el1": number;
  readonly "physical_timer_irq": number;
  readonly "pmu_clock_id": string;
  readonly "virtual_timer_irq": number;
};

export type ApplicationPartInput = ApplicationPartInputVariant0 | ApplicationPartInputVariant1 | ApplicationPartInputVariant2 | ApplicationPartInputVariant3;

export type ApplicationPartInputVariant0 = {
  readonly "kind": "register_words";
  readonly "part_id": string;
  readonly "registers": readonly RegisterWord[];
  readonly "rtc_ticks_per_second"?: number | null;
};

export type ApplicationPartInputVariant1 = {
  readonly "kind": "clock";
  readonly "part_id": string;
  readonly "seconds": number;
  readonly "valid": boolean;
};

export type ApplicationPartInputVariant2 = {
  readonly "high": boolean;
  readonly "kind": "idle_level";
  readonly "part_id": string;
};

export type ApplicationPartInputVariant3 = {
  readonly "idle_high": boolean;
  readonly "kind": "bis_revision";
  readonly "part_id": string;
  readonly "revision": readonly number[];
};

export type ApplicationStatisticsInput = {
  readonly "black_level": number;
  readonly "block_id": string;
  readonly "luma": number;
  readonly "samples_per_cell": number;
  readonly "signal_shift": number;
};

export type ApplicationTiming = ApplicationTimingVariant0 | ApplicationTimingVariant1 | ApplicationTimingVariant2 | ApplicationTimingVariant3;

export type ApplicationTimingVariant0 = {
  readonly "block_ids": readonly string[];
  readonly "kind": "delay";
  readonly "operation": TimingOperation;
  readonly "ticks": number;
};

export type ApplicationTimingVariant1 = {
  readonly "block_id": string;
  readonly "bytes_per_tick": number;
  readonly "kind": "throughput";
  readonly "setup_ticks": number;
};

export type ApplicationTimingVariant2 = {
  readonly "block_ids": readonly string[];
  readonly "kind": "parent_clock";
  readonly "ticks_per_parent": number;
};

export type ApplicationTimingVariant3 = {
  readonly "erase_ticks": number;
  readonly "kind": "storage_programming";
  readonly "page_ticks": number;
  readonly "part_id": string;
};

export type BitmapLayout = {
  readonly "data_bytes": number;
  readonly "data_offset": number;
  readonly "height_bytes": number;
  readonly "height_offset": number;
  readonly "kind_bytes": number;
  readonly "kind_offset": number;
  readonly "width_bytes": number;
  readonly "width_offset": number;
};

export type BitmapResources = {
  readonly "final_first_index": number;
  readonly "final_table": number;
  readonly "last_index": number;
  readonly "last_record": number;
  readonly "layout": BitmapLayout;
  readonly "stride": number;
  readonly "table": number;
};

export type BlockHardware = BlockHardwareVariant0 | BlockHardwareVariant1 | BlockHardwareVariant2 | BlockHardwareVariant3 | BlockHardwareVariant4 | BlockHardwareVariant5;

export type BlockHardwareVariant0 = {
  readonly "channels": number;
  readonly "kind": "dma";
  readonly "request_mux_block_id": string;
};

export type BlockHardwareVariant1 = {
  readonly "aperture_block_id": string;
  readonly "kind": "serial_flash";
};

export type BlockHardwareVariant2 = {
  readonly "bank_bytes": number;
  readonly "kind": "imaging_input_buffer";
  readonly "sram_region_id": string;
};

export type BlockHardwareVariant3 = {
  readonly "kind": "display";
  readonly "picture_layout"?: PictureLayout | null;
  readonly "video_layout"?: VideoLayout | null;
};

export type BlockHardwareVariant4 = {
  readonly "kind": "serial_dma";
  readonly "receive_request": number;
  readonly "transmit_request": number;
};

export type BlockHardwareVariant5 = {
  readonly "companion_irq_base": number;
  readonly "kind": "companion_dma";
};

export type BlockSignal = "detect" | "write_protect" | "data_ready" | "clock";

export type Body = {
  readonly "$schema": "https://fujimod.com/schemas/body.schema.json";
  readonly "card_slot"?: CardSlotAssembly | null;
  readonly "connections": readonly BodyConnection[];
  readonly "control_bus_chips": readonly ControlBusChip[];
  readonly "controls"?: BodyControls | null;
  readonly "display"?: DisplayAssembly | null;
  readonly "dram_bytes": number;
  readonly "hardware"?: BodyHardware | null;
  readonly "id": string;
  readonly "lens"?: LensAssembly | null;
  readonly "microcontroller"?: MicrocontrollerAssembly | null;
  readonly "nand"?: NandGeometry | null;
  readonly "parts": readonly string[];
  readonly "power_control"?: PowerControlAssembly | null;
  readonly "ram_regions"?: readonly BodyRamRegion[];
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

export type BodyControl = {
  readonly "assignable": boolean;
  readonly "connection_ids": readonly string[];
  readonly "default_function"?: string | null;
  readonly "kind": ControlKind;
  readonly "location": ControlLocation;
  readonly "manual_page"?: number | null;
  readonly "name": string;
  readonly "positions": readonly ControlPosition[];
  readonly "pushable": boolean;
};

export type BodyControls = {
  readonly "analog_levels": number;
  readonly "controls": Record<string, BodyControl>;
  readonly "manual": ManualReference;
};

export type BodyHardware = {
  readonly "gpio_feedback": readonly GpioFeedback[];
  readonly "keypad_samples": Record<string, number>;
  readonly "serial_attachments": readonly SerialAttachment[];
  readonly "thermal_connection_ids": readonly string[];
};

export type BodyRamRegion = {
  readonly "base": number;
  readonly "bytes": number;
  readonly "id": string;
  readonly "kind": MemoryKind;
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

export type CacheGeometry = {
  readonly "l1_data_bytes": number;
  readonly "l1_instruction_bytes": number;
  readonly "l2_bytes": number;
};

export type CalibrationInterval = {
  readonly "lower": number;
  readonly "upper": number;
};

export type CardSlotAssembly = {
  readonly "connection_id": string;
  readonly "controller_id"?: string | null;
  readonly "detect_connection_id": string;
  readonly "part_id": string;
  readonly "write_protect_connection_id": string;
};

export type ClockDomain = {
  readonly "frequency_hz"?: number | null;
  readonly "id": string;
  readonly "parent_id"?: string | null;
};

export type CompanionHardware = {
  readonly "clock_id": string;
  readonly "mailbox": CompanionMailbox;
  readonly "mailbox_block_id": string;
  readonly "processor_id": string;
  readonly "vector_base": number;
};

export type CompanionMailbox = {
  readonly "control": number;
  readonly "fault": number;
  readonly "reply": number;
  readonly "request": number;
  readonly "status": number;
};

export type ConfigurationLocation = ConfigurationLocationVariant0 | ConfigurationLocationVariant1;

export type ConfigurationLocationVariant0 = {
  readonly "address": number;
  readonly "kind": "pointer";
};

export type ConfigurationLocationVariant1 = {
  readonly "address": number;
  readonly "kind": "direct";
};

export type ConnectionKind = "control" | "usb" | "hdmi" | "sd-card" | "remote" | "hot-shoe" | "external-power" | "battery-door" | "sensor" | "lens" | "display" | "microcontroller" | "power-control";

export type ConnectionSignal = ConnectionSignalVariant0 | ConnectionSignalVariant1 | ConnectionSignalVariant2 | ConnectionSignalVariant3 | ConnectionSignalVariant4;

export type ConnectionSignalVariant0 = {
  readonly "active_level": boolean;
  readonly "kind": "gpio";
  readonly "pin": number;
  readonly "port": number;
};

export type ConnectionSignalVariant1 = {
  readonly "channel": number;
  readonly "kind": "adc";
  readonly "threshold"?: number | null;
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

export type ConnectionSignalVariant4 = {
  readonly "controller_id": string;
  readonly "kind": "interrupt";
  readonly "line": number;
};

export type ContactPosition = {
  readonly "contacts": readonly boolean[];
  readonly "drive": number;
};

export type ContentIdentity = {
  readonly "sha256": string;
  readonly "size": number;
};

export type ControlAction = {
  readonly "keys": readonly string[];
};

export type ControlBusAttachment = {
  readonly "address": number;
  readonly "channel": number;
  readonly "controller_id": string;
};

export type ControlBusChip = {
  readonly "address": number;
  readonly "channel": number;
  readonly "controller_id": string;
  readonly "id": string;
  readonly "polarity_selector": number;
};

export type ControlInput = ControlInputVariant0 | ControlInputVariant1 | ControlInputVariant2;

export type ControlInputVariant0 = {
  readonly "callback"?: number | null;
  readonly "connection_id": string;
  readonly "event": number;
  readonly "expected_result"?: number | null;
  readonly "intervals": readonly CalibrationInterval[];
  readonly "kind": "adc";
  readonly "scanner_index": number;
};

export type ControlInputVariant1 = {
  readonly "connection_id": string;
  readonly "event": number;
  readonly "expected_level": boolean;
  readonly "kind": "gpio";
  readonly "scanner": GpioScanner;
  readonly "scanner_index": number;
};

export type ControlInputVariant2 = {
  readonly "connection_id": string;
  readonly "event": number;
  readonly "guard": number;
  readonly "kind": "gpio_quadrature";
  readonly "phase_map": readonly number[];
  readonly "reset": readonly number[];
  readonly "threshold": number;
  readonly "transitions": readonly number[];
};

export type ControlInterface = ControlInterfaceVariant0 | ControlInterfaceVariant1;

export type ControlInterfaceVariant0 = {
  readonly "connection_id": string;
  readonly "endpoint": number;
  readonly "kind": "power_switch";
  readonly "mask": number;
  readonly "off_value": number;
  readonly "on_value": number;
  readonly "register": number;
  readonly "saved_inputs": number;
};

export type ControlInterfaceVariant1 = {
  readonly "connection_id": string;
  readonly "contacts": number;
  readonly "coordinate_bits": number;
  readonly "height": number;
  readonly "irq": number;
  readonly "irq_mode": number;
  readonly "kind": "prefixed_touch";
  readonly "mode": number;
  readonly "offset_tags": readonly number[];
  readonly "priority": number;
  readonly "register": number;
  readonly "report_bytes": number;
  readonly "width": number;
};

export type ControlKind = "dial" | "button" | "lever" | "switch" | "ring" | "shutter" | "wheel" | "joystick" | "touchscreen";

export type ControlLocation = "top" | "front" | "rear" | "lens" | "bottom";

export type ControlPosition = {
  readonly "description"?: string | null;
  readonly "id": string;
  readonly "label"?: string | null;
};

export type ControlReaders = {
  readonly "calibration": number;
  readonly "scanner": ScannerReader;
};

export type ControllerInterruptInput = {
  readonly "line": number;
  readonly "output_id": string;
  readonly "source_block_id": string;
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

export type CursorFields = {
  readonly "cursor": number;
  readonly "filtered_count": number;
  readonly "pointer": number;
  readonly "raw_count": number;
  readonly "subcursor": number;
  readonly "value_cursor": number;
  readonly "visible_count": number;
};

export type CursorLayout = {
  readonly "fields": CursorFields;
  readonly "size": number;
  readonly "status_offset": number;
  readonly "status_stride": number;
  readonly "status_width": number;
};

export type DebugKey = {
  readonly "event": number;
  readonly "name": string;
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
  readonly "id": string;
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

export type FirmwareControl = {
  readonly "actions": Record<string, ControlAction>;
  readonly "input"?: Record<string, readonly ControlInput[]> | null;
  readonly "interface"?: ControlInterface | null;
  readonly "keys": readonly string[];
  readonly "positions": readonly FirmwareControlPosition[];
  readonly "unavailable_reason"?: string | null;
};

export type FirmwareControlPosition = {
  readonly "active"?: boolean | null;
  readonly "keys": readonly string[];
  readonly "position": string;
};

export type FirmwareControls = {
  readonly "controls": Record<string, FirmwareControl>;
  readonly "debug_keys_available": boolean;
  readonly "input_readers": ControlReaders;
  readonly "native_key_names": NativeKeyNames;
  readonly "scanner": ScannerTable;
};

export type FirmwareFacts = {
  readonly "$schema": string;
  readonly "body_id": string;
  readonly "configuration": ConfigurationLocation;
  readonly "control_names": Record<string, string>;
  readonly "controls": FirmwareControls;
  readonly "debug_keys": readonly DebugKey[];
  readonly "firmware_id": string;
  readonly "firmware_sha256": string;
  readonly "identity_checks": readonly AddressIdentity[];
  readonly "input_bindings": Record<string, string>;
  readonly "menus": MenuTables;
  readonly "resources": Resources;
  readonly "scanout": Scanout;
  readonly "schema": number;
  readonly "state": FirmwareObservation;
  readonly "uncached_alias": number;
};

export type FirmwareGeneratedFacts = {
  readonly "additional_ui_modes"?: AdditionalUiModes | null;
  readonly "cfgdata_location"?: ConfigurationLocation | null;
  readonly "controls"?: FirmwareControls | null;
  readonly "identity_checks": readonly AddressIdentity[];
  readonly "menu_cursor"?: number | null;
  readonly "menu_keys"?: MenuKeys | null;
  readonly "observation"?: FirmwareObservation | null;
  readonly "remote"?: FirmwareFacts | null;
  readonly "uncached_alias"?: number | null;
};

export type FirmwareImageSection = {
  readonly "address": number;
  readonly "name": string;
  readonly "native_flash_offset"?: number | null;
  readonly "sha256": string;
  readonly "size": number;
};

export type FirmwareInstallation = {
  readonly "entry_labels": Record<string, string>;
  readonly "flash_offset": number;
  readonly "image_sections": readonly FirmwareImageSection[];
  readonly "loader_validation"?: FirmwareLoaderValidation | null;
};

export type FirmwareLoaderValidation = {
  readonly "entry": number;
  readonly "sections": readonly string[];
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

export type FlashSeed = {
  readonly "offset": number;
  readonly "value": number;
};

export type FocusAxis = {
  readonly "infinity_position": number;
  readonly "output": OpticalQuantity;
  readonly "steps_per_mm": number;
};

export type GeneratedSceneAsset = {
  readonly "id": string;
  readonly "pattern": readonly number[];
  readonly "size": number;
};

export type GlyphResources = {
  readonly "data_address_offset": number;
  readonly "data_size_offset": number;
  readonly "icon_index": number;
  readonly "record_size": number;
  readonly "registry_address": number;
};

export type GpioFeedback = {
  readonly "input_connection_id": string;
  readonly "output_connection_id": string;
};

export type GpioOutputWord = {
  readonly "levels": number;
  readonly "port": number;
};

export type GpioScanner = GpioScannerVariant0 | GpioScannerVariant1;

export type GpioScannerVariant0 = {
  readonly "kind": "direct";
};

export type GpioScannerVariant1 = {
  readonly "kind": "combined";
  readonly "lower": number;
  readonly "source": number;
  readonly "upper": number;
};

export type HomeSwitch = {
  readonly "threshold": number;
};

export type InterruptControllerHardware = InterruptControllerHardwareVariant0 | InterruptControllerHardwareVariant1;

export type InterruptControllerHardwareVariant0 = {
  readonly "cpu_interface_block_id": string;
  readonly "distributor_block_id": string;
  readonly "kind": "gic_v2";
  readonly "lines": number;
};

export type InterruptControllerHardwareVariant1 = {
  readonly "distributor_block_id": string;
  readonly "kind": "gic_v3";
  readonly "lines": number;
  readonly "redistributor_block_id": string;
  readonly "redistributor_stride": number;
};

export type InterruptRoute = {
  readonly "core": string;
  readonly "line": number;
  readonly "output_id"?: string | null;
  readonly "source_block": string;
};

export type IrisOptics = {
  readonly "output": OpticalQuantity;
  readonly "positions": readonly AperturePosition[];
  readonly "travel_limits": readonly number[];
};

export type LensAssembly = {
  readonly "connection_id": string;
  readonly "encoder_connection_id"?: string | null;
  readonly "focus"?: FocusAxis | null;
  readonly "home_connections": readonly LensHomeConnection[];
  readonly "iris"?: IrisOptics | null;
  readonly "part_id": string;
  readonly "pulse": PulseInterface;
  readonly "route": SerialRoute;
};

export type LensHomeConnection = {
  readonly "connection_id": string;
  readonly "motor": number;
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

export type ManualReference = {
  readonly "pages": readonly number[];
  readonly "title": string;
  readonly "url": string;
};

export type MemoryKind = "dram" | "rom" | "sram" | "tcm";

export type MemoryRegion = {
  readonly "base": number;
  readonly "id": string;
  readonly "kind": MemoryKind;
  readonly "size": RegionSize;
};

export type MenuFields = {
  readonly "flags_word": number;
  readonly "handlers_pointer": number;
  readonly "item_array": number;
  readonly "menu_id": number;
  readonly "parent_pointer": number;
  readonly "rows_per_page": number;
  readonly "setting_id": number;
  readonly "type_code": number;
};

export type MenuKeys = {
  readonly "back": string;
  readonly "down": string;
  readonly "menu": string;
  readonly "up": string;
};

export type MenuLayout = {
  readonly "caption_words": number;
  readonly "fields": MenuFields;
  readonly "max_kind": number;
  readonly "nowrap_flags": number;
  readonly "row_child": number;
  readonly "row_key": number;
  readonly "row_size": number;
  readonly "sentinel": number;
  readonly "size": number;
};

export type MenuNavigation = {
  readonly "setup_first_tab": number;
  readonly "shooting_first_tab": number;
  readonly "shooting_last_tab": number;
};

export type MenuTables = {
  readonly "cursor": CursorLayout;
  readonly "cursor_address": number;
  readonly "layout": MenuLayout;
  readonly "navigation": MenuNavigation;
  readonly "page_tabs": MenuTablesVariant0;
  readonly "q_grid": QuickGrid;
  readonly "roles": Record<string, number>;
  readonly "root_count": number;
  readonly "root_routes": MenuTablesVariant1;
  readonly "root_titles": MenuTablesVariant2;
  readonly "roots_address": number;
};

export type MenuTablesVariant0 = Record<string, never>;

export type MenuTablesVariant1 = Record<string, never>;

export type MenuTablesVariant2 = Record<string, never>;

export type MicrocontrollerAssembly = {
  readonly "channel": number;
  readonly "connection_id": string;
  readonly "controller_id": string;
  readonly "part_id": string;
  readonly "select_connection_id": string;
  readonly "word_bits": number;
};

export type NandGeometry = {
  readonly "bytes": number;
  readonly "page_bytes": number;
  readonly "pages_per_block": number;
  readonly "spare_bytes": number;
};

export type NativeKeyName = {
  readonly "event": number;
  readonly "name": string;
  readonly "string_address": number;
};

export type NativeKeyNames = {
  readonly "reader": number;
  readonly "rows": readonly NativeKeyName[];
  readonly "sha256": string;
  readonly "size": number;
};

export type OpticalQuantity = "lens_focus" | "lens_aperture";

export type Part = {
  readonly "$schema": "https://fujimod.com/schemas/part.schema.json";
  readonly "id": string;
  readonly "model": PartModel;
  readonly "schema": number;
};

export type PartEncoder = {
  readonly "phase_ticks": number;
  readonly "rest_phase": number;
};

export type PartModel = PartModelVariant0 | PartModelVariant1 | PartModelVariant2 | PartModelVariant3 | PartModelVariant4 | PartModelVariant5 | PartModelVariant6 | PartModelVariant7 | PartModelVariant8;

export type PartModelVariant0 = {
  readonly "encoder"?: PartEncoder | null;
  readonly "kind": "lens-mechanism";
  readonly "motors": readonly LensMotor[];
  readonly "protocol": LensProtocol;
  readonly "zoom"?: ZoomMechanism | null;
};

export type PartModelVariant1 = {
  readonly "kind": "contact-actuator";
  readonly "positions": readonly ContactPosition[];
  readonly "settle_ticks": number;
};

export type PartModelVariant2 = {
  readonly "endpoint": SerialOutputKind;
  readonly "kind": "serial-output";
};

export type PartModelVariant3 = {
  readonly "clock_registers": RegisterRange;
  readonly "initial_registers": readonly number[];
  readonly "kind": "microcontroller";
  readonly "packet_registers": RegisterRange;
  readonly "registers": RegisterRange;
  readonly "status_register"?: number | null;
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
  readonly "height"?: number | null;
  readonly "kind": "display-panel";
  readonly "orientation"?: DisplayOrientation | null;
  readonly "serial": SerialOutputKind;
  readonly "used_mask": number;
  readonly "width"?: number | null;
};

export type PartModelVariant8 = {
  readonly "interface": PeripheralPart;
  readonly "kind": "peripheral";
};

export type PeripheralMapping = {
  readonly "address": number;
  readonly "block_id": string;
  readonly "core": string;
  readonly "offset"?: number | null;
};

export type PeripheralPart = PeripheralPartVariant0 | PeripheralPartVariant1 | PeripheralPartVariant2 | PeripheralPartVariant3 | PeripheralPartVariant4 | PeripheralPartVariant5 | PeripheralPartVariant6 | PeripheralPartVariant7 | PeripheralPartVariant8 | PeripheralPartVariant9;

export type PeripheralPartVariant0 = {
  readonly "kind": "focus-actuator";
};

export type PeripheralPartVariant1 = {
  readonly "kind": "iris-actuator";
};

export type PeripheralPartVariant2 = {
  readonly "kind": "audio-serial-output";
};

export type PeripheralPartVariant3 = {
  readonly "kind": "imaging-serial-output";
};

export type PeripheralPartVariant4 = {
  readonly "kind": "imaging-sensor";
  readonly "read_gate_mask": number;
  readonly "read_gate_register": number;
};

export type PeripheralPartVariant5 = {
  readonly "kind": "power-controller";
};

export type PeripheralPartVariant6 = {
  readonly "kind": "serial-calibration-registers";
};

export type PeripheralPartVariant7 = {
  readonly "kind": "bis-configuration-registers";
};

export type PeripheralPartVariant8 = {
  readonly "kind": "lsm6ds-configuration-registers";
};

export type PeripheralPartVariant9 = {
  readonly "erase_bytes"?: number | null;
  readonly "jedec_id"?: number | null;
  readonly "kind": "serial-nor-flash";
  readonly "page_bytes"?: number | null;
};

export type PictureLayout = "bgra8888-straight";

export type PixelFormat = "bgra8888";

export type PowerControlAssembly = {
  readonly "channel": number;
  readonly "connection_id": string;
  readonly "controller_id": string;
  readonly "part_id": string;
  readonly "select_connection_id": string;
};

export type ProcessorArchitecture = "arm32" | "aarch64";

export type ProcessorCore = {
  readonly "id": string;
  readonly "model": ProcessorModel;
  readonly "mprofile"?: MProfileState | null;
  readonly "registers": readonly RegisterValue[];
  readonly "released"?: boolean | null;
  readonly "rprofile"?: RProfileState | null;
};

export type ProcessorModel = "cortex-r5f" | "cortex-a7" | "cortex-a53" | "cortex-m4";

export type ProcessorRegister = "pc" | "sp" | "pstate" | "cpsr";

export type PulseInterface = {
  readonly "counter_channels": readonly number[];
  readonly "initial_phase": number;
  readonly "quadrature": boolean;
  readonly "timer_channels": readonly number[];
};

export type QuickGrid = {
  readonly "columns": number;
  readonly "menu_code": number;
  readonly "rows": number;
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

export type RegisterWord = {
  readonly "register": number;
  readonly "value": number;
};

export type ResolvedConfiguration = {
  readonly "configuration": RunConfiguration;
  readonly "reference": ResolvedSelection;
};

export type ResolvedSelection = {
  readonly "body": Body;
  readonly "firmware": Firmware;
  readonly "parts": readonly Part[];
  readonly "soc": Soc;
};

export type Resources = {
  readonly "bitmaps": BitmapResources;
  readonly "glyph_first": number;
  readonly "glyph_last": number;
  readonly "glyphs": GlyphResources;
  readonly "language": ConfigurationLocation;
  readonly "last_caption": number;
  readonly "private_text_bias": number;
  readonly "private_text_first": number;
  readonly "private_text_last": number;
  readonly "supplementary_address": number;
  readonly "supplementary_first": number;
  readonly "supplementary_max_units": number;
  readonly "supplementary_stride": number;
  readonly "text_max_units": number;
  readonly "text_metrics_offset": number;
  readonly "text_pointer_offset": number;
  readonly "text_record_size": number;
  readonly "text_unit_bytes": number;
  readonly "text_variants_offset": number;
};

export type RootRoute = {
  readonly "entry": string;
  readonly "tab"?: number | null;
};

export type RunAdcInputs = {
  readonly "block_id": string;
  readonly "samples": readonly number[];
};

export type RunCardInput = {
  readonly "connection_id": string;
  readonly "present": boolean;
  readonly "write_protected": boolean;
};

export type RunConfiguration = {
  readonly "$schema": "https://fujimod.com/schemas/emulator-run-configuration.schema.json";
  readonly "body_id": string;
  readonly "execution"?: RunExecution | null;
  readonly "firmware_id": string;
  readonly "id": string;
  readonly "inputs"?: RunInputs | null;
  readonly "overlays": readonly string[];
  readonly "purpose": RunPurpose;
  readonly "scene"?: RunScene | null;
  readonly "schema": number;
  readonly "start": RunStart;
};

export type RunConnectionState = {
  readonly "connected": boolean;
  readonly "connection_id": string;
};

export type RunEncoderInput = {
  readonly "connection_id": string;
  readonly "phase": number;
};

export type RunEngine = "unicorn";

export type RunExecution = {
  readonly "engine": RunEngine;
  readonly "instructions_per_tick": RunInstructionRate;
  readonly "mode": RunExecutionMode;
  readonly "quantum": number;
};

export type RunExecutionMode = "strict";

export type RunFirstStart = {
  readonly "hold_ticks": number;
  readonly "key_id": string;
  readonly "max_presses": number;
  readonly "settle_ticks": number;
};

export type RunGpioInputs = {
  readonly "block_id": string;
  readonly "connections": readonly RunConnectionState[];
  readonly "keys": readonly RunKeyState[];
  readonly "outputs": readonly RunGpioOutputState[];
  readonly "unassigned_input_level": boolean;
};

export type RunGpioOutputState = {
  readonly "connection_id": string;
  readonly "high": boolean;
  readonly "signal_index": number;
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

export type RunInputs = RunInputsVariant0 | RunInputsVariant1 | RunInputsVariant2;

export type RunInputsVariant0 = {
  readonly "adc": RunAdcInputs;
  readonly "card": RunCardInput;
  readonly "encoders": readonly RunEncoderInput[];
  readonly "first_start"?: RunFirstStart | null;
  readonly "gpio": RunGpioInputs;
  readonly "kind": "exr2";
  readonly "mcu": RunMcuInput;
  readonly "power": RunPowerInput;
  readonly "serial_registers": readonly RunSerialRegisters[];
  readonly "usb": RunUsbInput;
};

export type RunInputsVariant1 = {
  readonly "blocks": readonly ApplicationBlockInput[];
  readonly "kind": "xp4";
  readonly "parts": readonly ApplicationPartInput[];
  readonly "ticks_per_second": number;
  readonly "timings": readonly ApplicationTiming[];
};

export type RunInputsVariant2 = {
  readonly "blocks": readonly ApplicationBlockInput[];
  readonly "kind": "xp5";
  readonly "parts": readonly ApplicationPartInput[];
  readonly "ticks_per_second": number;
  readonly "timings": readonly ApplicationTiming[];
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

export type RunOverlayChange = RunOverlayChangeVariant0 | RunOverlayChangeVariant1 | RunOverlayChangeVariant2 | RunOverlayChangeVariant3;

export type RunOverlayChangeVariant0 = {
  readonly "connected": boolean;
  readonly "kind": "usb_connection";
};

export type RunOverlayChangeVariant1 = {
  readonly "kind": "start";
  readonly "start": RunStart;
};

export type RunOverlayChangeVariant2 = {
  readonly "kind": "purpose";
  readonly "purpose": RunPurpose;
};

export type RunOverlayChangeVariant3 = {
  readonly "execution": RunExecution;
  readonly "kind": "execution";
};

export type RunPowerInput = {
  readonly "boot_reply": number;
  readonly "part_id": string;
  readonly "powered_on": boolean;
  readonly "spi_status": readonly RunRegisterValue[];
};

export type RunPurpose = "rom_boot" | "service_session" | "rehearsal" | "boot_access" | "shutdown";

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

export type RunScene = RunSceneVariant0 | RunSceneVariant1 | RunSceneVariant2;

export type RunSceneVariant0 = {
  readonly "frame_period_ticks": number;
  readonly "generated_assets": readonly GeneratedSceneAsset[];
  readonly "images": RunImages;
  readonly "kind": "exr2";
  readonly "statistics": RunStatistics;
};

export type RunSceneVariant1 = {
  readonly "command_images": readonly ApplicationCommandImageInput[];
  readonly "frames": readonly ApplicationFrameInput[];
  readonly "kind": "xp4";
  readonly "statistics": readonly ApplicationStatisticsInput[];
};

export type RunSceneVariant2 = {
  readonly "command_images": readonly ApplicationCommandImageInput[];
  readonly "frames": readonly ApplicationFrameInput[];
  readonly "kind": "xp5";
  readonly "statistics": readonly ApplicationStatisticsInput[];
};

export type RunSerialRegisters = {
  readonly "part_id": string;
  readonly "values": readonly RunRegisterValue[];
};

export type RunStart = RunStartVariant0 | RunStartVariant1 | RunStartVariant2;

export type RunStartVariant0 = {
  readonly "kind": "rom";
};

export type RunStartVariant1 = {
  readonly "kind": "loader";
};

export type RunStartVariant2 = {
  readonly "kind": "sd_card";
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

export type ScannerReader = {
  readonly "pressed_value": number;
  readonly "released_value": number;
  readonly "routine": AddressIdentity;
  readonly "stable_offset": number;
  readonly "stable_width": number;
};

export type ScannerTable = {
  readonly "base": number;
  readonly "count": number;
  readonly "lookup": number;
  readonly "sha256": string;
  readonly "stride": number;
};

export type Scanout = {
  readonly "address": number;
  readonly "address_mask": number;
  readonly "channel": number;
  readonly "height": number;
  readonly "instance": number;
  readonly "layer": number;
  readonly "pixel_format": PixelFormat;
  readonly "pointer_address": number;
  readonly "stride_bytes": number;
  readonly "width": number;
};

export type SensorAssembly = {
  readonly "connection_id": string;
  readonly "control_bus"?: ControlBusAttachment | null;
  readonly "part_id": string;
};

export type SensorControlInterface = {
  readonly "data_bits": number;
  readonly "register_address_bits": number;
};

export type SensorRaster = {
  readonly "height": number;
  readonly "image_area": readonly number[];
  readonly "pixel_clock_hz"?: number | null;
  readonly "width": number;
};

export type SerialAttachment = {
  readonly "connection_id": string;
  readonly "controller_id": string;
  readonly "part_id": string;
  readonly "select_connection_id"?: string | null;
};

export type SerialOutputKind = "panel" | "registers" | "command_data" | "fixed";

export type SerialRoute = {
  readonly "channel": number;
  readonly "clock_hz"?: number | null;
  readonly "controller_id": string;
  readonly "endpoint_id": string;
  readonly "select_connection_id": string;
  readonly "word_bits": number;
};

export type Soc = {
  readonly "$schema": "https://fujimod.com/schemas/soc.schema.json";
  readonly "architecture": ProcessorArchitecture;
  readonly "blocks": readonly SocBlock[];
  readonly "clocks": readonly ClockDomain[];
  readonly "decoders": readonly AddressDecoder[];
  readonly "dma_mappings": readonly DmaMapping[];
  readonly "hardware"?: SocHardware | null;
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
  readonly "kind": SocBlockKind;
  readonly "processor_core_id"?: string | null;
  readonly "size": number;
};

export type SocBlockKind = "gpio" | "adc" | "interrupt-controller" | "timer" | "dma" | "control-bus" | "serial-controller" | "nand-controller" | "sd-card-controller" | "usb-controller" | "display-controller" | "imaging-controller" | "reset-controller" | "clock-controller" | "address-window";

export type SocHardware = {
  readonly "application"?: ApplicationHardware | null;
  readonly "blocks": Record<string, BlockHardware>;
  readonly "companion"?: CompanionHardware | null;
  readonly "interrupt_controller"?: InterruptControllerHardware | null;
  readonly "interrupt_inputs": readonly ControllerInterruptInput[];
};

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

export type TimingOperation = "settle" | "sample" | "command" | "reset" | "pixel" | "acknowledge" | "complete";

export type VideoLayout = "yycbcr422";

export type WireSignal = "gpio" | "adc" | "serial_data" | "serial_clock" | "image_data" | "display_data" | "microcontroller" | "storage" | "pulse" | "power";

export type ZoomMechanism = {
  readonly "maximum_position": number;
  readonly "minimum_position": number;
  readonly "pulse_ticks": number;
};


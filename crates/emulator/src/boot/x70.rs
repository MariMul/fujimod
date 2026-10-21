//! X70 run-policy and EXR II device-model assembly.
//!
//! This module binds the checked X70 hardware and firmware identities to
//! the EXR II device implementations. Fixed model values are implementation
//! policy; physical addresses, links, and starting inputs are resolved below.

use fujimod_catalog::{
    configuration::{RunConfiguration, RunInputs},
    reference::{ConnectionSignal, PartModel, ResolvedSelection, SocBlock, SocBlockKind},
};
use fujimod_emulator_devices::{
    exr2::board::{BootEntry, Model, Node, Plan, WireKind},
    graph::Mapping as DeviceMapping,
    parts::pins::Pin,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn model_recipe(name: &str) -> Result<Value, String> {
    let recipe = match name {
        "irq0" => serde_json::from_str::<Value>("{\"kind\":\"exr2-interrupt-controller\",\"options\":{\"arbitration\":\"active-bitmap\"},\"core\":0,\"base\":4294651904,\"routes\":\"imaging_routes\",\"software\":[]}").map_err(|e| e.to_string())?,
        "irq1" => serde_json::from_str::<Value>("{\"kind\":\"exr2-interrupt-controller\",\"options\":{\"arbitration\":\"active-bitmap\"},\"core\":1,\"base\":4294656000,\"routes\":\"imaging_routes\",\"software\":[{\"line\":120,\"pulse\":\"release\"}]}").map_err(|e| e.to_string())?,
        "timers" => serde_json::from_str::<Value>("{\"kind\":\"exr2-system-timers\",\"options\":{\"main_targets\":[0,1]},\"controllers\":[\"irq0\",\"irq1\"]}").map_err(|e| e.to_string())?,
        "release" => serde_json::from_str::<Value>("{\"kind\":\"core-release\",\"options\":{\"released\":false},\"core\":1,\"request\":\"release\"}").map_err(|e| e.to_string())?,
        "serial_gpio" => serde_json::from_str::<Value>("{\"kind\":\"gpio\",\"options\":{\"input_levels\":[3118,0,4608,777,0,0,0,0,0,0,0,0,128,0,0,0],\"output_levels\":[{\"pin\":{\"port\":11,\"pin\":3},\"high\":true},{\"pin\":{\"port\":12,\"pin\":3},\"high\":true},{\"pin\":{\"port\":13,\"pin\":9},\"high\":true}]},\"irqs\":[{\"pin\":{\"port\":0,\"pin\":0},\"edge\":\"both\",\"outputs\":[{\"Core\":{\"core\":0,\"line\":8}}]},{\"pin\":{\"port\":0,\"pin\":2},\"edge\":\"falling\",\"outputs\":[{\"Core\":{\"core\":0,\"line\":10}}]},{\"pin\":{\"port\":0,\"pin\":3},\"edge\":\"falling\",\"outputs\":[{\"Core\":{\"core\":0,\"line\":11}}]}]}").map_err(|e| e.to_string())?,
        "serial_spi1" => serde_json::from_str::<Value>("{\"kind\":\"spi-controller\",\"routes\":[{\"options\":{\"base\":4294459456,\"transports\":[{\"control\":224,\"configuration\":196612,\"bits\":16,\"receive\":true},{\"control\":65760,\"configuration\":196612,\"bits\":16,\"receive\":false}],\"selection\":[{\"pin\":{\"port\":12,\"pin\":3},\"high\":false}],\"transfer_ticks\":4,\"queue\":\"single\"},\"gpio\":\"serial_gpio\",\"endpoint\":{\"kind\":\"registers\",\"options\":{\"registers\":[[0,0],[1,0],[3,0],[4,0],[5,0],[6,0],[7,0],[8,0],[9,0],[10,0],[11,0],[12,0],[13,0],[14,0],[15,0],[16,0],[18,0],[19,0],[23,0],[24,0],[25,0],[26,0],[27,0],[28,0],[29,0],[30,0],[31,0],[32,0],[33,0],[34,0],[35,0],[36,0],[37,0],[38,0],[39,0],[40,0],[41,0],[42,0],[43,0],[44,0],[45,0],[46,0],[47,0]],\"readable\":false,\"status_registers\":[],\"clock\":null}}},{\"options\":{\"base\":4294459456,\"transports\":[{\"control\":224,\"configuration\":6,\"bits\":16,\"receive\":true},{\"control\":65760,\"configuration\":6,\"bits\":16,\"receive\":false}],\"selection\":[{\"pin\":{\"port\":12,\"pin\":3},\"high\":false}],\"transfer_ticks\":4,\"queue\":\"single\"},\"gpio\":\"serial_gpio\",\"endpoint\":{\"kind\":\"registers\",\"options\":{\"registers\":[[65,0],[66,0],[67,0],[68,0],[69,0],[70,0],[73,0],[74,0],[75,0],[76,0],[78,0],[80,0],[81,0],[82,0],[83,0],[84,0],[85,0],[86,0],[87,0],[88,0],[89,0],[90,0],[91,0],[92,0],[93,0],[94,170],[95,86],[96,0],[104,0],[105,0],[106,0]],\"readable\":true,\"status_registers\":[96],\"clock\":{\"registers\":[65,66,67,68],\"initial_seconds\":0,\"ticks_per_second\":12000000,\"maximum_seconds\":1609459199,\"started\":0}}}},{\"options\":{\"base\":4294459456,\"transports\":[{\"control\":224,\"configuration\":6,\"bits\":16,\"receive\":false}],\"selection\":[{\"pin\":{\"port\":11,\"pin\":3},\"high\":false},{\"pin\":{\"port\":12,\"pin\":3},\"high\":true}],\"transfer_ticks\":4,\"queue\":\"single\"},\"gpio\":\"serial_gpio\",\"endpoint\":{\"kind\":\"panel\",\"options\":{\"used_mask\":65408}}}]}").map_err(|e| e.to_string())?,
        "dram_controller" => serde_json::from_str::<Value>("{\"kind\":\"dram\",\"options\":{}}").map_err(|e| e.to_string())?,
        "dram_phy" => serde_json::from_str::<Value>("{\"kind\":\"dram-phy\",\"options\":{}}").map_err(|e| e.to_string())?,
        "reset_control" => serde_json::from_str::<Value>("{\"kind\":\"reset\",\"options\":{}}").map_err(|e| e.to_string())?,
        "clocks" => serde_json::from_str::<Value>("{\"kind\":\"clocks\",\"options\":{\"output_settle_ticks\":2,\"reset_ready\":true}}").map_err(|e| e.to_string())?,
        "block98000" => serde_json::from_str::<Value>("{\"kind\":\"block-98000\",\"options\":{\"boot_source\":1,\"selection_reset\":0},\"wiring\":{\"decoder\":0,\"selections\":[{\"word\":0,\"choice\":0},{\"word\":1,\"choice\":1},{\"word\":2,\"choice\":1}]}}").map_err(|e| e.to_string())?,
        "sd_clocks" => serde_json::from_str::<Value>("{\"kind\":\"sdhci-clocks\",\"words\":[[4294246444,2]]}").map_err(|e| e.to_string())?,
        "adc" => serde_json::from_str::<Value>("{\"kind\":\"adc\",\"options\":{\"conversion_ticks\":4,\"samples\":[0,0,1023,63,534,640,0,825,1023,60,0,0]}}").map_err(|e| e.to_string())?,
        "statistics" => serde_json::from_str::<Value>("{\"kind\":\"statistics\",\"options\":{\"clock\":{\"kind\":\"fixed\",\"ticks\":400000},\"ack_ticks\":2,\"stop_ticks\":2,\"histogram\":true,\"external_statistics\":false},\"routes\":\"imaging_routes\",\"frames\":\"live_statistics\",\"images\":\"live_image\"}").map_err(|e| e.to_string())?,
        "image_output_0" => serde_json::from_str::<Value>("{\"kind\":\"output-channel\",\"options\":{\"stop_ticks\":2,\"clock\":{\"kind\":\"fixed\",\"ticks\":400000},\"geometry\":\"skip_incomplete\"},\"base\":4293992448,\"irq\":44,\"channel\":0,\"channels\":\"image_outputs\",\"routes\":\"imaging_routes\",\"frames\":\"live_image\",\"memory_outputs\":\"memory_outputs\"}").map_err(|e| e.to_string())?,
        "image_output_1" => serde_json::from_str::<Value>("{\"kind\":\"output-channel\",\"options\":{\"stop_ticks\":2,\"clock\":{\"kind\":\"fixed\",\"ticks\":400000},\"geometry\":\"skip_incomplete\"},\"base\":4293996544,\"irq\":45,\"channel\":1,\"channels\":\"image_outputs\",\"routes\":\"imaging_routes\",\"frames\":\"live_image\",\"memory_outputs\":\"memory_outputs\"}").map_err(|e| e.to_string())?,
        "face_detector" => serde_json::from_str::<Value>("{\"kind\":\"face-detector\",\"options\":{\"mode\":\"retained_words\"}}").map_err(|e| e.to_string())?,
        "phase_detection" => serde_json::from_str::<Value>("{\"kind\":\"phase-detection\",\"options\":{\"arithmetic\":\"aperture_sad\",\"data_ticks\":400000,\"stop_ticks\":2},\"routes\":\"imaging_routes\"}").map_err(|e| e.to_string())?,
        "live_frames" => serde_json::from_str::<Value>("{\"kind\":\"external-frames\",\"activity\":{\"gpio\":\"serial_gpio\",\"selection\":{\"pin\":{\"port\":0,\"pin\":2},\"high\":false}},\"options\":{\"period_ticks\":400000},\"images\":\"live_image\",\"statistics\":\"live_statistics\"}").map_err(|e| e.to_string())?,
        "sdio" => serde_json::from_str::<Value>("{\"kind\":\"sdio-host\",\"options\":{\"transfer_ticks\":100,\"clock_ticks\":2,\"rca\":4660,\"ram_bytes\":536870912},\"module\":{\"protocol\":\"enumeration\",\"firmware_bytes\":0},\"irq\":{\"Core\":{\"core\":0,\"line\":79}}}").map_err(|e| e.to_string())?,
        "nand" => serde_json::from_str::<Value>("{\"kind\":\"nand\",\"options\":{\"erase_ticks\":64},\"medium\":\"nand\"}").map_err(|e| e.to_string())?,
        "lz" => serde_json::from_str::<Value>("{\"kind\":\"lz\",\"options\":{\"resize\":\"declared_packed_pyramid\",\"resize_ticks\":400000},\"base\":4294193152,\"controllers\":[\"irq0\",\"irq1\"]}").map_err(|e| e.to_string())?,
        "memory_dma" => serde_json::from_str::<Value>("{\"kind\":\"memory-dma\",\"options\":{},\"base\":4293926912,\"routes\":\"imaging_routes\"}").map_err(|e| e.to_string())?,
        "audio_dma" => serde_json::from_str::<Value>("{\"kind\":\"serial-dma\",\"options\":{\"ram_bytes\":536870912,\"max_transfer_bytes\":16777216}}").map_err(|e| e.to_string())?,
        "usb" => serde_json::from_str::<Value>("{\"kind\":\"usb\",\"options\":{\"endpoints\":null},\"cable\":\"usb_cable\",\"endpoints\":null}").map_err(|e| e.to_string())?,
        "power_peripherals" => serde_json::from_str::<Value>("{\"kind\":\"power\",\"options\":{\"transfer_ticks\":4,\"max_program_bytes\":536870912,\"spi0\":{\"required\":[192,48,0],\"selection\":{\"pin\":{\"port\":11,\"pin\":2},\"high\":false},\"presence\":{\"pin\":{\"port\":11,\"pin\":2},\"high\":false}}},\"gpio\":\"serial_gpio\"}").map_err(|e| e.to_string())?,
        "audio_registers" => serde_json::from_str::<Value>("{\"kind\":\"serial-audio\",\"options\":{},\"sinks\":[{},{}]}").map_err(|e| e.to_string())?,
        "pulse" => serde_json::from_str::<Value>("{\"kind\":\"exr2-pulse\",\"options\":{\"counter_channels\":[0,4],\"timer_channels\":[0,2,4,6,8,9,10,11],\"quadrature\":false,\"initial_phase\":0,\"capture_channel\":null},\"irqs\":[{\"Core\":{\"core\":0,\"line\":20}},{\"Core\":{\"core\":0,\"line\":21}},{\"Core\":{\"core\":0,\"line\":22}},{\"Core\":{\"core\":0,\"line\":23}},{\"Core\":{\"core\":0,\"line\":24}},{\"Core\":{\"core\":0,\"line\":25}},{\"Core\":{\"core\":0,\"line\":26}},{\"Core\":{\"core\":0,\"line\":27}},{\"Core\":{\"core\":0,\"line\":28}},{\"Core\":{\"core\":0,\"line\":29}},{\"Core\":{\"core\":0,\"line\":30}},{\"Core\":{\"core\":0,\"line\":31}}]}").map_err(|e| e.to_string())?,
        "lens_serial" => serde_json::from_str::<Value>("{\"kind\":\"spi-controller\",\"routes\":[{\"options\":{\"base\":4294459520,\"transports\":[{\"control\":224,\"configuration\":4,\"bits\":16,\"receive\":false}],\"selection\":[{\"pin\":{\"port\":13,\"pin\":9},\"high\":false}],\"transfer_ticks\":4,\"queue\":\"single\"},\"gpio\":\"serial_gpio\",\"endpoint\":{\"kind\":\"lens-unit\",\"options\":{\"protocol\":{\"kind\":\"banked\",\"positive_direction_high\":false,\"stop_response\":\"settled\",\"clock\":[{\"kind\":\"microstep_stages\",\"ticks_per_quantum\":3},{\"kind\":\"encoder_stages\",\"ticks_per_quantum\":3}]},\"motors\":[{\"initial_position\":0,\"travel_limits\":[-32767,32767],\"ticks_per_step\":1000,\"settle_ticks\":4000,\"home\":{\"pin\":{\"port\":0,\"pin\":0},\"threshold\":8,\"side\":\"above\"},\"counters\":[{\"channel\":4,\"divider\":1},{\"channel\":0,\"divider\":4}]},{\"initial_position\":0,\"travel_limits\":[-32767,32767],\"ticks_per_step\":1000,\"settle_ticks\":4000,\"home\":null,\"counters\":[]}],\"encoder\":null,\"zoom\":null},\"pulses\":\"pulse\",\"gpio\":\"serial_gpio\",\"completion\":[{\"Core\":{\"core\":0,\"line\":14}},{\"Core\":{\"core\":0,\"line\":12}}],\"optics\":{\"output\":\"lens_aperture\",\"positions\":[[6,16.000000000000004],[10,14.25437949024543],[14,12.699208415745595],[18,11.313708498984761],[22,10.079368399158986],[26,8.979696386474986],[30,8.000000000000002],[34,7.127189745122715],[38,6.349604207872798],[42,5.656854249492381],[46,5.039684199579493],[50,4.489848193237492],[54,4.000000000000001],[58,3.5635948725613575],[62,3.174802103936399],[78,2.8284271247461903]]},\"focus\":{\"output\":\"lens_focus\",\"infinity_position\":-111,\"steps_per_mm\":267.0}}}]}").map_err(|e| e.to_string())?,
        "control_bus0" => serde_json::from_str::<Value>("{\"kind\":\"i2c\",\"options\":{\"base\":4294463488,\"irq\":101,\"modes\":[9],\"transfer_ticks\":4,\"profile\":\"endpoint7-stationary-and9-heartbeat-latched-irq-unmeasured\",\"parts\":{\"kind\":\"primary\",\"touch\":{\"register\":0,\"core\":0,\"irq\":15,\"ticks_per_second\":12000000},\"sensor_address\":28,\"prefix_address\":85,\"sensor_fields\":[14,15,17,21,23,24,29,31,32,33,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49],\"prefix_high\":28,\"heartbeat_register\":7174,\"topology\":[24,68,57,25,12,28,85],\"stationary\":false,\"status\":[],\"commands\":[]}},\"controllers\":[\"irq0\",\"irq1\"],\"cable\":null,\"bridge\":null}").map_err(|e| e.to_string())?,
        "control_bus1" => serde_json::from_str::<Value>("{\"kind\":\"i2c\",\"options\":{\"base\":4294463552,\"irq\":102,\"modes\":[1,9],\"transfer_ticks\":4,\"profile\":\"usbcharge-disconnected-pwic-startup-tagged\",\"parts\":{\"kind\":\"secondary\",\"register_address\":10,\"pwic_address\":48,\"register_modes\":[9],\"pwic_modes\":[1],\"register_values\":[[2,0],[5,0],[12,0],[17,0],[19,0]],\"register_masks\":[[12,255],[17,255]],\"pwic_values\":[[97,0],[32,128]],\"pwic_masks\":[[97,16],[32,2]],\"pwic_read_prefixes\":[[32,[0,0,0]]],\"topology\":[48,114,10],\"connected\":false}},\"controllers\":[\"irq0\",\"irq1\"],\"cable\":\"usb_cable\",\"bridge\":null}").map_err(|e| e.to_string())?,
        "companion" => serde_json::from_str::<Value>("{\"kind\":\"fe80-processing\",\"transport\":{},\"options\":{\"irq\":88,\"pixels_per_tick\":64},\"frames\":\"live_image\",\"routes\":\"imaging_routes\"}").map_err(|e| e.to_string())?,
        "image_input" => serde_json::from_str::<Value>("{\"kind\":\"image-input\",\"options\":{\"reset_ticks\":2,\"command_ticks\":2,\"phase_ticks\":2,\"command\":\"packed_writes\",\"command_controls\":[0],\"phase_ack\":\"inactive\",\"clock\":{\"mode\":\"input_enable\",\"source\":{\"kind\":\"periodic\",\"period_ticks\":400000,\"first_delay_ticks\":null},\"secondary\":true,\"companion_secondary\":true},\"sensor_period_ticks\":null,\"raw_dma_ticks\":1000,\"readback_ticks\":750000,\"cfa\":[0,2,1,2,0,1,1,1,0,1,1,2,1,1,2,1,1,0,2,0,1,0,2,1,1,1,2,1,1,0,1,1,0,1,1,2]},\"phase_irqs\":[{\"core\":0,\"line\":39},{\"core\":0,\"line\":41}],\"raw_irq\":{\"core\":0,\"line\":36},\"readback_irq\":{\"core\":0,\"line\":38},\"shutter_output\":null,\"sensor\":null,\"parameters\":\"parameters\",\"memory_outputs\":\"memory_outputs\",\"statistics_grid\":null,\"frame_period\":null,\"edges\":\"phase_edges\"}").map_err(|e| e.to_string())?,
        "input_auxiliary" => serde_json::from_str::<Value>("{\"kind\":\"block11000\",\"options\":{\"control_mode\":{\"ClearBit0Ack\":{\"delay_ticks\":2}}}}").map_err(|e| e.to_string())?,
        "parameters" => serde_json::from_str::<Value>("{\"kind\":\"parameters\",\"options\":{},\"bank\":\"parameters\"}").map_err(|e| e.to_string())?,
        "hdmisink" => serde_json::from_str::<Value>("{\"kind\":\"hdmi-sink\",\"options\":{\"connected\":false,\"edid\":[]}}").map_err(|e| e.to_string())?,
        "hdmi" => serde_json::from_str::<Value>("{\"kind\":\"hdmi\",\"options\":{},\"sink\":\"hdmisink\"}").map_err(|e| e.to_string())?,
        "output" => serde_json::from_str::<Value>("{\"kind\":\"block86000\",\"options\":{}}").map_err(|e| e.to_string())?,
        "graphics" => serde_json::from_str::<Value>("{\"kind\":\"vector-gpu\",\"options\":{\"ram_bytes\":536870912,\"max_fill_bytes\":67108864,\"max_copy_bytes\":67108864,\"max_clear_bytes\":67108864,\"bytes_per_tick\":16,\"max_command_capture_bytes\":524288,\"copy_profile\":\"vg-32bpp-untile-unrotated\",\"clear_profile\":\"declared-vg-ui-primitives-unmeasured\"},\"controllers\":[\"irq0\",\"irq1\"]}").map_err(|e| e.to_string())?,
        "scanout" => serde_json::from_str::<Value>("{\"kind\":\"scanout\",\"options\":{\"ram_bytes\":536870912,\"packing\":\"yycbcr\",\"matrix\":\"bt601-full\",\"graphics_profile\":\"mode1-abgr8888-straight\",\"backdrop_profile\":\"declared-black-canvas-unmeasured\",\"rectangle_profile\":\"exr2-signed-ycbcr-rectangle-overlay-unmeasured\",\"max_pixels\":8847360,\"reset_ticks\":2}}").map_err(|e| e.to_string())?,
        "display_timing" => serde_json::from_str::<Value>("{\"kind\":\"display-timing\",\"options\":{\"mode\":\"configured_clocked\",\"frame_ticks\":400000,\"stop_ticks\":2},\"clock\":\"clocks\",\"controllers\":[\"irq0\",\"irq1\"]}").map_err(|e| e.to_string())?,
        "display_formatter" => serde_json::from_str::<Value>("{\"kind\":\"formatter\",\"options\":{\"latch_delay\":2}}").map_err(|e| e.to_string())?,
        "image_combination" => serde_json::from_str::<Value>("{\"kind\":\"image-combination\",\"options\":{\"reset_ticks\":16,\"arithmetic\":\"packed_image_unmeasured\",\"clock\":{\"kind\":\"throughput\",\"bytes_per_tick\":256}},\"irq\":{\"core\":0,\"line\":68}}").map_err(|e| e.to_string())?,
        "encoder_control_ring" => serde_json::from_str::<Value>("{\"kind\":\"encoder\",\"options\":{\"pins\":[{\"port\":3,\"pin\":6},{\"port\":3,\"pin\":7}],\"phase_ticks\":120000,\"rest_phase\":0},\"gpio\":\"serial_gpio\",\"counter\":null}").map_err(|e| e.to_string())?,
        _ => return Err(format!("unknown X70 implementation node {name}")),
    };
    Ok(recipe)
}

#[derive(Clone, Copy)]
struct NodeRule {
    name: &'static str,
    blocks: &'static [&'static str],
    dependencies: &'static [&'static str],
}

const NODE_RULES: &[NodeRule] = &[
    NodeRule {
        name: "irq0",
        blocks: &["interrupt-core0"],
        dependencies: &[],
    },
    NodeRule {
        name: "irq1",
        blocks: &["interrupt-core1"],
        dependencies: &[],
    },
    NodeRule {
        name: "timers",
        blocks: &["system-timers"],
        dependencies: &[],
    },
    NodeRule {
        name: "release",
        blocks: &[],
        dependencies: &[],
    },
    NodeRule {
        name: "serial_gpio",
        blocks: &["gpio", "gpio-aux"],
        dependencies: &[],
    },
    NodeRule {
        name: "serial_spi1",
        blocks: &["serial-spi1"],
        dependencies: &["serial_gpio"],
    },
    NodeRule {
        name: "dram_controller",
        blocks: &["dram-controller"],
        dependencies: &[],
    },
    NodeRule {
        name: "dram_phy",
        blocks: &["dram-phy"],
        dependencies: &[],
    },
    NodeRule {
        name: "reset_control",
        blocks: &["reset-controller"],
        dependencies: &[],
    },
    NodeRule {
        name: "clocks",
        blocks: &["clock-controller", "clock-controller-aux"],
        dependencies: &[],
    },
    NodeRule {
        name: "block98000",
        blocks: &["block-98000"],
        dependencies: &[],
    },
    NodeRule {
        name: "sd_clocks",
        blocks: &["sd-clocks-0"],
        dependencies: &[],
    },
    NodeRule {
        name: "adc",
        blocks: &["adc"],
        dependencies: &[],
    },
    NodeRule {
        name: "statistics",
        blocks: &["statistics", "statistics-aux"],
        dependencies: &["live_frames"],
    },
    NodeRule {
        name: "image_output_0",
        blocks: &["image-output-0"],
        dependencies: &["live_frames"],
    },
    NodeRule {
        name: "image_output_1",
        blocks: &["image-output-1"],
        dependencies: &["live_frames"],
    },
    NodeRule {
        name: "face_detector",
        blocks: &["face-detector-0"],
        dependencies: &[],
    },
    NodeRule {
        name: "phase_detection",
        blocks: &["phase-detection", "phase-detection-aux"],
        dependencies: &[],
    },
    NodeRule {
        name: "live_frames",
        blocks: &[],
        dependencies: &["serial_gpio"],
    },
    NodeRule {
        name: "sdio",
        blocks: &["sdio-host", "sdio-1"],
        dependencies: &[],
    },
    NodeRule {
        name: "nand",
        blocks: &["nand"],
        dependencies: &[],
    },
    NodeRule {
        name: "lz",
        blocks: &["decompressor"],
        dependencies: &["irq0", "irq1"],
    },
    NodeRule {
        name: "memory_dma",
        blocks: &["memory-dma", "memory-dma-1"],
        dependencies: &["irq0", "irq1"],
    },
    NodeRule {
        name: "audio_dma",
        blocks: &["audio-dma"],
        dependencies: &[],
    },
    NodeRule {
        name: "usb",
        blocks: &["usb-core", "usb-phy"],
        dependencies: &[],
    },
    NodeRule {
        name: "power_peripherals",
        blocks: &[
            "power-peripherals",
            "power-registers-1",
            "power-registers-2",
            "power-registers-3",
        ],
        dependencies: &["serial_gpio"],
    },
    NodeRule {
        name: "audio_registers",
        blocks: &["serial-audio", "serial-audio-aux"],
        dependencies: &[],
    },
    NodeRule {
        name: "pulse",
        blocks: &["pulse-outputs"],
        dependencies: &[],
    },
    NodeRule {
        name: "lens_serial",
        blocks: &["serial-spi2"],
        dependencies: &["pulse", "serial_gpio"],
    },
    NodeRule {
        name: "control_bus0",
        blocks: &["control-bus0"],
        dependencies: &["irq0", "irq1"],
    },
    NodeRule {
        name: "control_bus1",
        blocks: &["control-bus1"],
        dependencies: &["irq0", "irq1"],
    },
    NodeRule {
        name: "companion",
        blocks: &[
            "peripheral-window-fe318000",
            "peripheral-window-fe3b0000",
            "companion-fe80c",
            "companion-fe80d",
        ],
        dependencies: &["image_output_0", "image_output_1", "live_frames"],
    },
    NodeRule {
        name: "image_input",
        blocks: &["image-input"],
        dependencies: &["parameters"],
    },
    NodeRule {
        name: "input_auxiliary",
        blocks: &["input-auxiliary-control"],
        dependencies: &[],
    },
    NodeRule {
        name: "parameters",
        blocks: &["image-parameters"],
        dependencies: &["image_input"],
    },
    NodeRule {
        name: "hdmisink",
        blocks: &[],
        dependencies: &[],
    },
    NodeRule {
        name: "hdmi",
        blocks: &["hdmi-0", "hdmi-1", "hdmi-2"],
        dependencies: &["hdmisink"],
    },
    NodeRule {
        name: "output",
        blocks: &["output-control"],
        dependencies: &[],
    },
    NodeRule {
        name: "graphics",
        blocks: &["graphics"],
        dependencies: &["irq0", "irq1"],
    },
    NodeRule {
        name: "scanout",
        blocks: &["scanout", "scanout-aux"],
        dependencies: &[],
    },
    NodeRule {
        name: "display_timing",
        blocks: &["display-timing", "display-timing-aux"],
        dependencies: &[],
    },
    NodeRule {
        name: "display_formatter",
        blocks: &["display-formatter", "display-formatter-aux"],
        dependencies: &[],
    },
    NodeRule {
        name: "image_combination",
        blocks: &["image-combination"],
        dependencies: &["irq0", "irq1"],
    },
    NodeRule {
        name: "encoder_control_ring",
        blocks: &[],
        dependencies: &["serial_gpio"],
    },
];

pub(super) fn node_for_block(block_id: &str) -> Result<String, String> {
    NODE_RULES
        .iter()
        .find(|rule| rule.blocks.contains(&block_id))
        .map(|rule| rule.name.to_owned())
        .ok_or_else(|| format!("X70 implementation has no node for SoC block {block_id}"))
}

fn implementation_wires() -> BTreeMap<String, WireKind> {
    BTreeMap::from([
        ("release".into(), WireKind::SoftwarePulse),
        ("lens_aperture".into(), WireKind::Aperture),
        ("lens_focus".into(), WireKind::Focus),
        ("usb_cable".into(), WireKind::LogicLevel(false)),
        ("imaging_routes".into(), WireKind::IrqRoutes),
        ("image_outputs".into(), WireKind::OutputChannels),
        ("memory_outputs".into(), WireKind::MemoryOutputs),
        ("parameters".into(), WireKind::Parameters),
        ("phase_edges".into(), WireKind::PhaseEdges),
    ])
}

fn replace_bases(value: &mut Value, base: u64) {
    match value {
        Value::Object(object) => {
            if object.get("base").is_some_and(Value::is_number) {
                object.insert("base".into(), json!(base));
            }
            for child in object.values_mut() {
                replace_bases(child, base);
            }
        }
        Value::Array(items) => {
            for child in items {
                replace_bases(child, base);
            }
        }
        _ => (),
    }
}

fn apply_soc_blocks(
    recipe: &mut Value,
    rule: &NodeRule,
    selection: &ResolvedSelection,
) -> Result<(), String> {
    let Some(first) = rule.blocks.first() else {
        return Ok(());
    };
    let base = block(selection, first)?.base;
    replace_bases(recipe, base);
    if rule.name == "sd_clocks" {
        let words = recipe["words"]
            .as_array_mut()
            .ok_or("X70 SD clock model has no register words")?;
        if let Some(word) = words.first_mut().and_then(Value::as_array_mut) {
            word[0] = json!(base);
        } else {
            return Err("X70 SD clock model has no base register".into());
        }
    }
    Ok(())
}

fn block<'a>(selection: &'a ResolvedSelection, id: &str) -> Result<&'a SocBlock, String> {
    selection
        .soc
        .blocks
        .iter()
        .find(|item| item.id == id)
        .ok_or_else(|| format!("SoC {} has no block {id}", selection.soc.id))
}

fn irq(
    selection: &ResolvedSelection,
    source_block: &str,
    output_id: &str,
) -> Result<Value, String> {
    let mut routes = selection.soc.interrupts.iter().filter(|route| {
        route.source_block == source_block && route.output_id.as_deref() == Some(output_id)
    });
    let route = routes.next().ok_or_else(|| {
        format!(
            "SoC {} has no {source_block} interrupt output {output_id}",
            selection.soc.id
        )
    })?;
    if routes.next().is_some() {
        return Err(format!(
            "SoC {} has ambiguous {source_block} interrupt output {output_id}",
            selection.soc.id
        ));
    }
    let core = selection
        .soc
        .processors
        .iter()
        .position(|processor| processor.id == route.core)
        .ok_or_else(|| {
            format!(
                "interrupt output {output_id} targets absent core {}",
                route.core
            )
        })?;
    Ok(json!({"Core":{"core":core,"line":route.line}}))
}

fn apply_interrupts(
    recipe: &mut Value,
    name: &str,
    selection: &ResolvedSelection,
) -> Result<(), String> {
    match name {
        "irq0" => {
            recipe["core"] = json!(core(selection, "core0")?);
        }
        "irq1" => {
            recipe["core"] = json!(core(selection, "core1")?);
            let release = selection
                .soc
                .reset
                .release
                .as_ref()
                .ok_or("SoC has no secondary-core release facts")?;
            let route = irq(selection, "interrupt-core1", "software-release")?;
            if route["Core"]["line"] != release.source {
                return Err("SoC secondary-core release source differs from its IRQ route".into());
            }
            recipe["software"] = json!([{
                "line":route["Core"]["line"],
                "pulse":"release"
            }]);
        }
        "serial_gpio" => {
            let home = selection
                .body
                .lens
                .as_ref()
                .and_then(|lens| lens.home_connections.iter().find(|home| home.motor == 0))
                .ok_or("X70 lens motor 0 has no declared home connection")?;
            let pin = |connection_id: &str| -> Result<Value, String> {
                let signals = gpio_signals(selection, connection_id)?;
                if signals.len() != 1 {
                    return Err(format!(
                        "GPIO interrupt connection {connection_id} must resolve to one pin"
                    ));
                }
                Ok(json!({"port":signals[0].0.port,"pin":signals[0].0.pin}))
            };
            recipe["irqs"] = json!([
                {
                    "pin":pin(&home.connection_id)?,
                    "edge":"both",
                    "outputs":[irq(selection,"gpio","port-0-pin-0")?]
                },
                {
                    "pin":pin("s1")?,
                    "edge":"falling",
                    "outputs":[irq(selection,"gpio","port-0-pin-2")?]
                },
                {
                    "pin":pin("s2")?,
                    "edge":"falling",
                    "outputs":[irq(selection,"gpio","port-0-pin-3")?]
                }
            ]);
        }
        "release" => {
            let processor = selection
                .soc
                .processors
                .iter()
                .find(|processor| processor.id == "core1")
                .ok_or("SoC has no secondary processor core")?;
            recipe["core"] = json!(core(selection, "core1")?);
            recipe["options"]["released"] = json!(
                processor
                    .released
                    .ok_or("SoC does not declare the secondary-core reset state")?
            );
        }
        "pulse" => {
            recipe["irqs"] = Value::Array(
                (0..12)
                    .map(|index| irq(selection, "pulse-outputs", &format!("output-{index}")))
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        "lens_serial" => {
            recipe["routes"][0]["endpoint"]["completion"] = json!([
                irq(selection, "serial-spi2", "complete-0")?,
                irq(selection, "serial-spi2", "complete-1")?
            ]);
        }
        "control_bus0" => {
            recipe["options"]["irq"] =
                json!(irq(selection, "control-bus0", "controller")?["Core"]["line"]);
            let (touch_core, touch_line) = connection_interrupt(selection, "touch-bus")?;
            recipe["options"]["parts"]["touch"]["irq"] = json!(touch_line);
            recipe["options"]["parts"]["touch"]["core"] = json!(touch_core);
        }
        "control_bus1" => {
            recipe["options"]["irq"] =
                json!(irq(selection, "control-bus1", "controller")?["Core"]["line"]);
        }
        "image_output_0" => {
            recipe["irq"] = json!(irq(selection, "image-output-0", "output")?["Core"]["line"]);
        }
        "image_output_1" => {
            recipe["irq"] = json!(irq(selection, "image-output-1", "output")?["Core"]["line"]);
        }
        "sdio" => recipe["irq"] = irq(selection, "sdio-host", "host")?,
        "companion" => {
            recipe["options"]["irq"] =
                json!(irq(selection, "companion-fe80", "processing")?["Core"]["line"]);
        }
        "image_input" => {
            recipe["phase_irqs"] = json!([
                {"core":core(selection,"core0")?,"line":irq(selection,"image-input","phase-0")?["Core"]["line"]},
                {"core":core(selection,"core0")?,"line":irq(selection,"image-input","phase-1")?["Core"]["line"]}
            ]);
            recipe["raw_irq"] = json!({"core":core(selection,"core0")?,"line":irq(selection,"image-input","raw")?["Core"]["line"]});
            recipe["readback_irq"] = json!({"core":core(selection,"core0")?,"line":irq(selection,"image-input","readback")?["Core"]["line"]});
        }
        "image_combination" => {
            let route = irq(selection, "image-combination", "complete")?;
            recipe["irq"] = json!({
                "core":route["Core"]["core"],
                "line":route["Core"]["line"]
            });
        }
        _ => (),
    }
    Ok(())
}

fn core(selection: &ResolvedSelection, id: &str) -> Result<usize, String> {
    selection
        .soc
        .processors
        .iter()
        .position(|processor| processor.id == id)
        .ok_or_else(|| format!("SoC {} has no processor core {id}", selection.soc.id))
}

fn connection_interrupt(
    selection: &ResolvedSelection,
    connection_id: &str,
) -> Result<(usize, u32), String> {
    let mut interrupts = connection(selection, connection_id)?
        .signals
        .iter()
        .filter_map(|signal| match signal {
            ConnectionSignal::Interrupt {
                controller_id,
                line,
            } => Some((controller_id.as_str(), *line)),
            _ => None,
        });
    let (controller, line) = interrupts
        .next()
        .ok_or_else(|| format!("body connection {connection_id} has no interrupt attachment"))?;
    if interrupts.next().is_some() {
        return Err(format!(
            "body connection {connection_id} has multiple interrupt attachments"
        ));
    }
    let controller = block(selection, controller)?;
    if controller.kind != SocBlockKind::InterruptController {
        return Err(format!(
            "body connection {connection_id} uses non-interrupt block {}",
            controller.id
        ));
    }
    let processor_id = controller.processor_core_id.as_deref().ok_or_else(|| {
        format!(
            "SoC interrupt controller {} has no processor-core attachment",
            controller.id
        )
    })?;
    Ok((core(selection, processor_id)?, line))
}

fn connection<'a>(
    selection: &'a ResolvedSelection,
    id: &str,
) -> Result<&'a fujimod_catalog::reference::BodyConnection, String> {
    selection
        .body
        .connections
        .iter()
        .find(|item| item.id == id)
        .ok_or_else(|| format!("body {} has no connection {id}", selection.body.id))
}

fn gpio_signals(
    selection: &ResolvedSelection,
    connection_id: &str,
) -> Result<Vec<(Pin, bool)>, String> {
    let item = connection(selection, connection_id)?;
    let pins = item
        .signals
        .iter()
        .filter_map(|signal| match signal {
            ConnectionSignal::Gpio {
                port,
                pin,
                active_level,
            } => Some((
                Pin {
                    port: *port,
                    pin: *pin,
                },
                *active_level,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    if pins.is_empty() {
        return Err(format!(
            "body connection {connection_id} has no GPIO signal for this run input"
        ));
    }
    Ok(pins)
}

fn input_level(
    levels: &mut [u32; 16],
    assigned: &mut BTreeMap<(u8, u8), String>,
    pin: Pin,
    high: bool,
    owner: &str,
) -> Result<(), String> {
    pin.validate()?;
    if pin.port >= 16 {
        return Err("run GPIO input pin exceeds the native input-level array".into());
    }
    if let Some(previous) = assigned.insert((pin.port, pin.pin), owner.to_owned()) {
        return Err(format!(
            "run GPIO input assignments {previous} and {owner} collide on GPIO {}.{}",
            pin.port, pin.pin,
        ));
    }
    let mask = 1u32 << pin.pin;
    if high {
        levels[usize::from(pin.port)] |= mask;
    } else {
        levels[usize::from(pin.port)] &= !mask;
    }
    Ok(())
}

fn gpio_levels(
    selection: &ResolvedSelection,
    inputs: &RunInputs,
) -> Result<([u32; 16], Vec<Value>), String> {
    let unassigned = inputs.gpio.unassigned_input_level;
    let mut levels = [if unassigned { u32::MAX } else { 0 }; 16];
    let mut assigned = BTreeMap::<(u8, u8), String>::new();
    let mut handled = BTreeSet::new();
    for state in &inputs.gpio.keys {
        for (pin, active) in gpio_signals(selection, &state.connection_id)? {
            input_level(
                &mut levels,
                &mut assigned,
                pin,
                if state.pressed { active } else { !active },
                &state.connection_id,
            )?;
        }
        handled.insert(state.connection_id.as_str());
    }
    for state in &inputs.gpio.connections {
        for (pin, active) in gpio_signals(selection, &state.connection_id)? {
            input_level(
                &mut levels,
                &mut assigned,
                pin,
                if state.connected { active } else { !active },
                &state.connection_id,
            )?;
        }
        handled.insert(state.connection_id.as_str());
    }

    for state in &inputs.encoders {
        let signals = gpio_signals(selection, &state.connection_id)?;
        if signals.len() != 2 {
            return Err(format!(
                "X70 encoder {} must expose exactly two GPIO phase signals",
                state.connection_id
            ));
        }
        for (index, (pin, active)) in signals.into_iter().enumerate() {
            let phase_active = state.phase & (1 << index) != 0;
            input_level(
                &mut levels,
                &mut assigned,
                pin,
                if phase_active { active } else { !active },
                &state.connection_id,
            )?;
        }
        handled.insert(state.connection_id.as_str());
    }

    let card = selection
        .body
        .card_slot
        .as_ref()
        .ok_or("X70 run requires a declared card-slot attachment")?;
    if inputs.card.present {
        return Err("X70 has no removable-card controller".into());
    }
    for (id, asserted) in [
        (card.detect_connection_id.as_str(), inputs.card.present),
        (
            card.write_protect_connection_id.as_str(),
            inputs.card.write_protected,
        ),
    ] {
        for (pin, active) in gpio_signals(selection, id)? {
            input_level(
                &mut levels,
                &mut assigned,
                pin,
                if asserted { active } else { !active },
                id,
            )?;
        }
        handled.insert(id);
    }

    for item in &selection.body.connections {
        if item
            .signals
            .iter()
            .any(|signal| matches!(signal, ConnectionSignal::Gpio { .. }))
            && !handled.contains(item.id.as_str())
        {
            let is_select = selection
                .body
                .lens
                .as_ref()
                .is_some_and(|part| part.route.select_connection_id == item.id)
                || selection
                    .body
                    .display
                    .as_ref()
                    .and_then(|part| part.route.as_ref())
                    .is_some_and(|part| part.select_connection_id == item.id)
                || selection
                    .body
                    .microcontroller
                    .as_ref()
                    .is_some_and(|part| part.select_connection_id == item.id)
                || selection
                    .body
                    .power_control
                    .as_ref()
                    .is_some_and(|part| part.select_connection_id == item.id);
            let is_lens_endpoint = selection.body.lens.as_ref().is_some_and(|lens| {
                lens.home_connections
                    .iter()
                    .any(|home| home.connection_id == item.id)
            });
            if !is_select && !is_lens_endpoint {
                return Err(format!(
                    "GPIO connection {} has no explicit run input or installed endpoint",
                    item.id
                ));
            }
        }
    }

    let mut outputs = BTreeMap::<(u8, u8), bool>::new();
    let mut output_owners = BTreeMap::<(u8, u8), String>::new();
    for state in &inputs.gpio.outputs {
        let target = connection(selection, &state.connection_id)?;
        let signal = target
            .signals
            .get(usize::from(state.signal_index))
            .ok_or_else(|| {
                format!(
                    "GPIO output {} has no signal at index {}",
                    state.connection_id, state.signal_index
                )
            })?;
        let ConnectionSignal::Gpio { port, pin, .. } = signal else {
            return Err(format!(
                "GPIO output {} signal {} is not a GPIO pad",
                state.connection_id, state.signal_index
            ));
        };
        let pin = Pin {
            port: *port,
            pin: *pin,
        };
        pin.validate()?;
        let identity = (pin.port, pin.pin);
        if let Some(previous) = output_owners.insert(identity, state.connection_id.clone()) {
            return Err(format!(
                "run GPIO output assignments {previous} and {} collide on GPIO {}.{}",
                state.connection_id, pin.port, pin.pin
            ));
        }
        outputs.insert(identity, state.high);
    }
    let output_values = outputs
        .into_iter()
        .map(|((port, pin), high)| json!({"pin":{"port":port,"pin":pin},"high":high}))
        .collect();
    Ok((levels, output_values))
}

fn apply_run_inputs(
    recipe: &mut Value,
    name: &str,
    selection: &ResolvedSelection,
    configuration: &RunConfiguration,
) -> Result<(), String> {
    let inputs = &configuration.inputs;
    match name {
        "serial_gpio" => {
            let (levels, outputs) = gpio_levels(selection, inputs)?;
            recipe["options"]["input_levels"] = json!(levels);
            recipe["options"]["output_levels"] = json!(outputs);
        }
        "adc" => {
            let samples: [u16; 12] = inputs
                .adc
                .samples
                .as_slice()
                .try_into()
                .map_err(|_| "X70 ADC requires twelve configured samples")?;
            recipe["options"]["samples"] = json!(samples);
        }
        "usb" => {
            if inputs.usb.connection_id != "usb" {
                return Err("X70 USB input does not resolve to its installed connection".into());
            }
        }
        "power_peripherals" => {
            if !inputs.power.powered_on {
                return Err("X70 run model does not support a powered-off supply".into());
            }
            if inputs.power.boot_reply != 0 || !inputs.power.spi_status.is_empty() {
                return Err(
                    "X70 power endpoint only supports boot reply zero and no SPI status inputs"
                        .into(),
                );
            }
        }
        "control_bus0" | "control_bus1" => {
            // The body control-bus chip inventory is validated by the catalog
            // resolver. Their modeled transaction contracts are an explicit
            // X70 implementation policy in this module.
        }
        "live_frames" => {
            let s1 = inputs
                .gpio
                .keys
                .iter()
                .find(|state| state.connection_id == "s1")
                .ok_or("X70 frame input requires an explicit S1 reset state")?;
            let signal = gpio_signals(selection, &s1.connection_id)?;
            if signal.len() != 1 {
                return Err("X70 S1 frame input must resolve to one GPIO signal".into());
            }
            recipe["activity"]["selection"] = json!({
                "pin":{"port":signal[0].0.port,"pin":signal[0].0.pin},
                "high":signal[0].1,
            });
        }
        "encoder_control_ring" => {
            if inputs.encoders.len() != 1 {
                return Err(
                    "X70 implementation requires exactly one configured control-ring encoder"
                        .into(),
                );
            }
            let state = &inputs.encoders[0];
            if state.connection_id != "controls-encoder-3-6-3-7" {
                return Err(format!(
                    "X70 implementation does not support encoder connection {}",
                    state.connection_id
                ));
            }
            let signals = gpio_signals(selection, &state.connection_id)?;
            if signals.len() != 2 {
                return Err("X70 control-ring encoder must expose two GPIO signals".into());
            }
            recipe["options"]["pins"] = json!(
                signals
                    .iter()
                    .map(|(pin, _)| { json!({"port":pin.port,"pin":pin.pin}) })
                    .collect::<Vec<_>>()
            );
            recipe["options"]["rest_phase"] = json!(state.phase);
        }
        _ => (),
    }

    let frame_period = configuration.scene.frame_period_ticks;
    match name {
        "statistics" | "image_output_0" | "image_output_1" => {
            recipe["options"]["clock"]["ticks"] = json!(frame_period);
        }
        "phase_detection" => {
            recipe["options"]["data_ticks"] = json!(frame_period);
        }
        "live_frames" => {
            recipe["options"]["period_ticks"] = json!(frame_period);
        }
        "image_input" => {
            recipe["options"]["clock"]["source"]["period_ticks"] = json!(frame_period);
        }
        "display_timing" => {
            recipe["options"]["frame_ticks"] = json!(frame_period);
        }
        _ => (),
    }

    if name == "serial_spi1" {
        let packet = &inputs.mcu.packet;
        let body_mcu = selection
            .body
            .microcontroller
            .as_ref()
            .ok_or("X70 has no MCU attachment")?;
        let part = selection
            .parts
            .iter()
            .find(|part| part.id == body_mcu.part_id)
            .ok_or("X70 MCU part is unresolved")?;
        let PartModel::Microcontroller {
            packet_registers,
            clock_registers,
            status_register,
            initial_registers,
            ..
        } = &part.model
        else {
            return Err("X70 MCU attachment is not a microcontroller part".into());
        };
        let mut values = BTreeMap::<u8, u8>::new();
        for (offset, value) in packet.iter().enumerate() {
            let address = packet_registers
                .first
                .checked_add(offset as u32)
                .and_then(|value| u8::try_from(value).ok())
                .ok_or("X70 MCU packet address exceeds one byte")?;
            values.insert(address, *value);
        }
        let rtc = inputs.mcu.rtc;
        for offset in 0..clock_registers.count {
            let address = clock_registers
                .first
                .checked_add(offset)
                .and_then(|value| u8::try_from(value).ok())
                .ok_or("X70 MCU clock address exceeds one byte")?;
            values.insert(address, (rtc.initial_seconds >> (offset * 8)) as u8);
        }
        if let Some(status) = status_register {
            values.entry(*status as u8).or_insert(0);
        }
        for address in initial_registers {
            values.entry(*address as u8).or_insert(0);
        }
        for input in &inputs.serial_registers {
            if input.part_id != body_mcu.part_id {
                return Err(format!(
                    "X70 has no serial-register input implementation for part {}",
                    input.part_id
                ));
            }
            for value in &input.values {
                values.insert(value.register, value.value);
            }
        }
        {
            let clock = recipe["routes"][body_mcu.channel as usize]["endpoint"]["options"]["clock"]
                .as_object_mut()
                .ok_or("X70 MCU implementation has no clock input")?;
            clock.insert("initial_seconds".into(), json!(rtc.initial_seconds));
            clock.insert("ticks_per_second".into(), json!(rtc.ticks_per_second));
            clock.insert("maximum_seconds".into(), json!(rtc.maximum_seconds));
            clock.insert("started".into(), json!(rtc.started));
        }
        let registers =
            recipe["routes"][body_mcu.channel as usize]["endpoint"]["options"]["registers"]
                .as_array_mut()
                .ok_or("X70 MCU implementation has no register bank")?;
        for entry in registers.iter_mut() {
            let address = entry[0]
                .as_u64()
                .and_then(|value| u8::try_from(value).ok())
                .ok_or("X70 MCU register address is invalid")?;
            if let Some(value) = values.get(&address) {
                entry[1] = json!(value);
            }
        }
        for address in values.keys() {
            if !registers
                .iter()
                .any(|row| row[0].as_u64() == Some(u64::from(*address)))
            {
                return Err(format!(
                    "X70 MCU register {address} has no installed endpoint"
                ));
            }
        }
    }
    if name == "usb_cable" {
        // The typed wire is updated in assemble after all recipes are built.
    }

    Ok(())
}

fn scene(selection: &ResolvedSelection, configuration: &RunConfiguration) -> Result<Value, String> {
    let nand = selection.body.nand.ok_or("X70 body has no NAND geometry")?;
    let image = &configuration.scene.images.live_image;
    let statistics = configuration
        .scene
        .statistics
        .live_statistics
        .iter()
        .map(|span| json!({"destination":span.destination,"offset":span.offset,"asset":span.asset}))
        .collect::<Vec<_>>();
    Ok(json!({
        "images": { "live_image": {
            "width": image.width,
            "height": image.height,
            "stride": image.stride,
            "asset": image.asset,
        }},
        "statistics": { "live_statistics": statistics },
        "nands": { "nand": {
            "kind": "raw",
            "options": {
                "bytes": nand.bytes,
                "page_bytes": nand.page_bytes,
                "pages_per_block": nand.pages_per_block,
                "spare_bytes": nand.spare_bytes,
            },
            "base": "nand_base",
            "overlays": [],
            "spare": [],
        }},
    }))
}

/// Build the closed EXR II plan from a resolved X70 identity and its run inputs.
pub(super) fn assemble(
    selection: &ResolvedSelection,
    configuration: &RunConfiguration,
) -> Result<(Plan, Value, Value), String> {
    if selection.soc.id != "exr2"
        || selection.body.id != "x70"
        || selection.firmware.id != "x70-110"
        || configuration.id != "x70-110-rec"
        || configuration.start != fujimod_catalog::configuration::RunStart::Rom
    {
        return Err(
            "X70 native boot implementation does not support this selected identity".into(),
        );
    }
    let mut nodes = Vec::with_capacity(NODE_RULES.len());
    let mut serialized_nodes = Vec::with_capacity(NODE_RULES.len());
    for rule in NODE_RULES {
        let mut recipe = model_recipe(rule.name)?;
        apply_soc_blocks(&mut recipe, rule, selection)?;
        apply_interrupts(&mut recipe, rule.name, selection)?;
        apply_run_inputs(&mut recipe, rule.name, selection, configuration)?;
        let model: Model = serde_json::from_value(recipe.clone())
            .map_err(|error| format!("X70 node {}: {error}", rule.name))?;
        let windows = rule
            .blocks
            .iter()
            .map(|id| {
                let block = block(selection, id)?;
                Ok::<_, String>(DeviceMapping {
                    base: block.base,
                    size: block.size,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        nodes.push(Node {
            name: rule.name.into(),
            boot_entries: vec![BootEntry::Rom],
            windows,
            dependencies: rule
                .dependencies
                .iter()
                .map(|name| (*name).into())
                .collect(),
            model,
        });
        serialized_nodes.push(json!({
            "name": rule.name,
            "boot_entries": ["rom"],
            "windows": rule.blocks.iter().map(|id| {
                let block = block(selection, id)?;
                Ok::<_, String>(json!({"base":block.base,"size":block.size}))
            }).collect::<Result<Vec<_>,_>>()?,
            "dependencies": rule.dependencies,
            "model": recipe,
        }));
    }
    let mut wires = implementation_wires();
    wires.insert(
        "usb_cable".into(),
        WireKind::LogicLevel(configuration.inputs.usb.connected),
    );
    let plan = Plan {
        schema: 1,
        boot_entry: BootEntry::Rom,
        wires,
        nodes,
    };
    plan.validate()?;
    let plan_value = json!({
        "schema":1,
        "boot_entry":"rom",
        "wires": {
            "release":"software-pulse",
            "lens_aperture":"aperture",
            "lens_focus":"focus",
            "usb_cable":{"logic-level":configuration.inputs.usb.connected},
            "imaging_routes":"irq-routes",
            "image_outputs":"output-channels",
            "memory_outputs":"memory-outputs",
            "parameters":"parameters",
            "phase_edges":"phase-edges"
        },
        "nodes":serialized_nodes,
    });
    let scene = scene(selection, configuration)?;
    Ok((plan, plan_value, scene))
}

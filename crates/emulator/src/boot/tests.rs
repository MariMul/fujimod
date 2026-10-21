use super::*;

fn model<'a>(prepared: &'a Prepared, name: &str) -> &'a serde_json::Value {
    prepared.devices_value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["name"] == name)
        .unwrap_or_else(|| panic!("missing prepared node {name}"))
        .get("model")
        .unwrap()
}

#[test]
fn runtime_catalog_builds_checked_graph_without_language_adapters() {
    let spec = prepare("x70-110-rec", &[]).unwrap();
    let graph = &spec.execution;
    graph.validate().unwrap();
    let plan = Plan::parse(&serde_json::to_vec(&spec.devices_value).unwrap()).unwrap();
    plan.validate_reset_outputs(
        &graph
            .processors
            .iter()
            .map(|p| p.released)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    plan.validate_memory_outputs(
        &graph
            .decoders
            .iter()
            .map(|d| MemoryChoices {
                initial: d.initial,
                choices: d.choices.len(),
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let mut assets = BTreeMap::from([("nand_base".into(), vec![255; 128 << 20])]);
    for asset in &spec.generated_assets {
        assets.insert(
            asset.name.clone(),
            asset.pattern.repeat(asset.size / asset.pattern.len()),
        );
    }
    let inputs = serde_json::from_value::<preparation::Spec>(spec.scene.clone())
        .unwrap()
        .build(&assets)
        .unwrap();
    let mut resources = Resources::default();
    resources.inputs = inputs;
    plan.build(resources, 0, &mut RefuseAdapters).unwrap();
    assert!(prepare("unknown", &[]).is_err());
}

#[test]
fn checked_run_configuration_selects_runtime_and_supplies_scene_and_quantum() {
    let spec = prepare("x70-110-rec", &[]).unwrap();
    let configuration = &spec.resolved.configuration;
    assert_eq!(spec.resolved.reference.body.id, configuration.body_id);
    assert_eq!(
        spec.resolved.reference.firmware.id,
        configuration.firmware_id
    );
    assert_eq!(spec.quantum, configuration.execution.quantum);
    assert_eq!(
        spec.execution_value["instructions_per_tick"],
        serde_json::json!([
            configuration.execution.instructions_per_tick.numerator,
            configuration.execution.instructions_per_tick.denominator
        ])
    );
    assert_eq!(
        spec.generated_assets.len(),
        configuration.scene.generated_assets.len()
    );
    assert_eq!(
        spec.scene["images"]["live_image"]["asset"],
        configuration.scene.images.live_image.asset
    );
    assert_eq!(
        spec.scene["statistics"]["live_statistics"][0]["asset"],
        configuration.scene.statistics.live_statistics[0].asset
    );
    let live_frames = spec.devices_value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["name"] == "live_frames")
        .unwrap();
    assert_eq!(
        live_frames["model"]["options"]["period_ticks"],
        configuration.scene.frame_period_ticks
    );
    let json = spec.to_json();
    assert_eq!(json["configuration_id"], "x70-110-rec");
    assert_eq!(json["overlays"], serde_json::json!([]));
    assert_eq!(json["resolved"]["reference"]["body"]["id"], "x70");
    assert!(json["inputs"]["mcu"]["route"].is_number());
    assert_eq!(json["inputs"]["first_start"]["key_id"], "EXE_M");
    assert_eq!(json["inputs"]["first_start"]["hold_ticks"], 3_600_000);
    assert_eq!(json["inputs"]["first_start"]["settle_ticks"], 20_000_000);
}

#[test]
fn unsupported_run_configuration_start_is_refused_and_usb_overlay_is_applied() {
    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved.configuration.start = RunStart::Loader;
    assert!(prepared(resolved).is_err());

    let spec = prepare("x70-110-rec", &["x70-110-rec-usb".into()]).unwrap();
    assert_eq!(spec.resolved.configuration.overlays, ["x70-110-rec-usb"]);
    assert_eq!(
        spec.devices_value["wires"]["usb_cable"]["logic-level"],
        true
    );

    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved.configuration.execution.quantum = 0;
    assert!(prepared(resolved).is_err());
}

#[test]
fn settings_validate_marker_checksum_and_route_extent() {
    let mut spec = prepare("x70-110-rec", &[]).unwrap();
    let mut input = vec![0; 16];
    input[14] = 0xaa;
    input[15] = 0x56;
    settings(&mut spec.devices_value, &spec.mcu, &input).unwrap();
    input[0] = 1;
    assert!(settings(&mut spec.devices_value, &spec.mcu, &input).is_err());
    input[0] = 0;
    input[14] = 0;
    input[15] = 0;
    assert!(packet(&input).is_err());
    assert!(packet(&input[..15]).is_err());
    input[14] = 0xaa;
    input[15] = 0x56;
    assert!(settings(&mut json!({}), &spec.mcu, &input).is_err());
}

#[test]
fn selected_files_are_bounded_and_hash_checked() {
    let path = std::env::temp_dir().join(format!("fujimod-boot-input-{}", std::process::id()));
    std::fs::write(&path, b"abc").unwrap();
    assert_eq!(checked(&path, &Content::identify(b"abc")).unwrap(), b"abc");
    assert!(read(&path, 2).is_err());
    assert!(checked(&path, &Content::identify(b"abd")).is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn configured_start_inputs_change_prepared_devices_or_refuse() {
    let base = prepare("x70-110-rec", &[]).unwrap();

    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved.configuration.inputs.adc.samples[0] = 1;
    let adc = prepared(resolved).unwrap();
    assert_eq!(model(&adc, "adc")["options"]["samples"][0], 1);

    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved.configuration.inputs.mcu.packet[0] = 1;
    resolved.configuration.inputs.mcu.packet[15] =
        resolved.configuration.inputs.mcu.packet[15].wrapping_sub(1);
    let packet = prepared(resolved).unwrap();
    let packet_value = model(&packet, "serial_spi1")["routes"][packet.mcu.route]["endpoint"]
        ["options"]["registers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row[0] == packet.mcu.first_register)
        .unwrap()[1]
        .as_u64()
        .unwrap();
    assert_eq!(packet_value, 1);

    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved.configuration.inputs.mcu.rtc.initial_seconds = 7;
    resolved.configuration.inputs.mcu.rtc.ticks_per_second = 24_000_000;
    resolved.configuration.inputs.mcu.rtc.maximum_seconds = 9_999;
    resolved.configuration.inputs.mcu.rtc.started = 17;
    let rtc = prepared(resolved).unwrap();
    let rtc_route = usize::from(
        rtc.resolved
            .reference
            .body
            .microcontroller
            .as_ref()
            .unwrap()
            .channel,
    );
    let clock = &model(&rtc, "serial_spi1")["routes"][rtc_route]["endpoint"]["options"]["clock"];
    assert_eq!(clock["initial_seconds"], 7);
    assert_eq!(clock["ticks_per_second"], 24_000_000);
    assert_eq!(clock["maximum_seconds"], 9_999);
    assert_eq!(clock["started"], 17);

    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved
        .configuration
        .inputs
        .gpio
        .keys
        .iter_mut()
        .find(|state| state.connection_id == "s1")
        .unwrap()
        .pressed = true;
    let pressed = prepared(resolved).unwrap();
    let s1 = pressed
        .resolved
        .reference
        .body
        .connections
        .iter()
        .find(|connection| connection.id == "s1")
        .unwrap();
    let (port, pin, active) = s1
        .signals
        .iter()
        .find_map(|signal| match signal {
            fujimod_catalog::reference::ConnectionSignal::Gpio {
                port,
                pin,
                active_level,
            } => Some((usize::from(*port), *pin, *active_level)),
            _ => None,
        })
        .unwrap();
    let input_level = model(&pressed, "serial_gpio")["options"]["input_levels"][port]
        .as_u64()
        .unwrap();
    assert_eq!(input_level & (1 << pin) != 0, active);
    assert_ne!(
        model(&base, "serial_gpio")["options"]["input_levels"][port],
        model(&pressed, "serial_gpio")["options"]["input_levels"][port]
    );

    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved.configuration.inputs.gpio.outputs[0].high = false;
    let output = prepared(resolved).unwrap();
    assert_ne!(
        model(&base, "serial_gpio")["options"]["output_levels"],
        model(&output, "serial_gpio")["options"]["output_levels"]
    );

    let usb = prepare("x70-110-rec", &["x70-110-rec-usb".into()]).unwrap();
    assert_eq!(usb.devices_value["wires"]["usb_cable"]["logic-level"], true);

    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved.configuration.inputs.encoders[0].phase = 3;
    let encoder = prepared(resolved).unwrap();
    assert_eq!(
        model(&encoder, "encoder_control_ring")["options"]["rest_phase"],
        3
    );

    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved.configuration.scene.frame_period_ticks = 432_100;
    let scene = prepared(resolved).unwrap();
    assert_eq!(
        model(&scene, "live_frames")["options"]["period_ticks"],
        432_100
    );

    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved
        .configuration
        .inputs
        .gpio
        .connections
        .iter_mut()
        .find(|state| state.connection_id == "power")
        .unwrap()
        .connected = false;
    let power = prepared(resolved).unwrap();
    assert_ne!(
        model(&base, "serial_gpio")["options"]["input_levels"],
        model(&power, "serial_gpio")["options"]["input_levels"]
    );

    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved.configuration.inputs.power.powered_on = false;
    assert!(
        prepared(resolved)
            .err()
            .unwrap()
            .contains("powered-off supply")
    );

    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved
        .configuration
        .execution
        .instructions_per_tick
        .numerator = 11;
    resolved
        .configuration
        .execution
        .instructions_per_tick
        .denominator = 3;
    let rate = prepared(resolved).unwrap();
    assert_eq!(
        rate.execution_value["instructions_per_tick"],
        serde_json::json!([11, 3])
    );

    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved.configuration.inputs.power.boot_reply = 1;
    assert!(
        prepared(resolved)
            .err()
            .unwrap()
            .contains("boot reply zero")
    );

    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved.configuration.inputs.power.spi_status.push(
        fujimod_catalog::configuration::RunRegisterValue {
            register: 9,
            value: 1,
        },
    );
    assert!(
        prepared(resolved)
            .err()
            .unwrap()
            .contains("SPI status inputs")
    );

    let mut resolved = ResolvedConfiguration::bundled("x70-110-rec").unwrap();
    resolved.configuration.inputs.card.present = true;
    assert!(prepared(resolved).is_err());
}

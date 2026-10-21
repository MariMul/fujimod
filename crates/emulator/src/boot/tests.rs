use super::*;

#[test]
fn runtime_catalog_builds_checked_graph_without_language_adapters() {
    let configuration = RunConfiguration::bundled("x70-110-rec").unwrap();
    let spec = definition(&configuration).unwrap();
    let graph: factory::Graph = serde_json::from_value(spec.execution).unwrap();
    graph.validate().unwrap();
    let plan = Plan::parse(&serde_json::to_vec(&spec.devices).unwrap()).unwrap();
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
    let mut assets = BTreeMap::from([("nand_base".into(), vec![255; 2048])]);
    for a in spec.generated_assets {
        assets.insert(a.name, a.pattern.repeat(a.size / a.pattern.len()));
    }
    let inputs = serde_json::from_value::<preparation::Spec>(spec.scene)
        .unwrap()
        .build(&assets)
        .unwrap();
    let mut resources = Resources::default();
    resources.inputs = inputs;
    plan.build(resources, 0, &mut RefuseAdapters).unwrap();
    assert!(RunConfiguration::bundled("unknown").is_err());
}

#[test]
fn checked_run_configuration_selects_runtime_and_supplies_scene_and_quantum() {
    let configuration = RunConfiguration::bundled("x70-110-rec").unwrap();
    let spec = definition(&configuration).unwrap();
    assert_eq!(spec.body, configuration.body_id);
    assert_eq!(spec.firmware, configuration.firmware_id);
    assert_eq!(spec.quantum, configuration.execution.quantum);
    assert_eq!(
        spec.execution["instructions_per_tick"],
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
    let live_frames = spec.devices["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["name"] == "live_frames")
        .unwrap();
    assert_eq!(
        live_frames["model"]["options"]["period_ticks"],
        configuration.scene.frame_period_ticks
    );
}

#[test]
fn unsupported_run_configuration_start_and_overlays_are_refused() {
    let mut configuration = RunConfiguration::bundled("x70-110-rec").unwrap();
    configuration.start = RunStart::Loader;
    assert!(definition(&configuration).is_err());

    let mut configuration = RunConfiguration::bundled("x70-110-rec").unwrap();
    configuration.overlays.push("x70-110-rec-usb".into());
    assert!(definition(&configuration).is_err());

    let mut configuration = RunConfiguration::bundled("x70-110-rec").unwrap();
    configuration.execution.quantum = 0;
    assert!(definition(&configuration).is_err());
}

#[test]
fn settings_validate_marker_checksum_and_route_extent() {
    let configuration = RunConfiguration::bundled("x70-110-rec").unwrap();
    let mut spec = definition(&configuration).unwrap();
    let mut input = vec![0; 16];
    input[14] = 0xaa;
    input[15] = 0x56;
    settings(&mut spec.devices, &spec.mcu, &input).unwrap();
    input[0] = 1;
    assert!(settings(&mut spec.devices, &spec.mcu, &input).is_err());
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

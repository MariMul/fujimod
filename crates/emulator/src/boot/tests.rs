use super::*;

#[test]
fn runtime_catalog_builds_checked_graph_without_language_adapters() {
    let spec = definition("x70", "x70-110").unwrap();
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
    assert!(definition("unknown", "x70-110").is_err());
    assert!(definition("x70", "unknown").is_err());
}

#[test]
fn settings_validate_marker_checksum_and_route_extent() {
    let mut spec = definition("x70", "x70-110").unwrap();
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

use fujimod_catalog::ReferenceCatalog;
use fujimod_catalog_remote::{BuildMenuManifest, Facts, MENU_MANIFEST_SCHEMA_URI};

#[test]
fn selected_firmware_supplies_complete_stock_remote_inputs() {
    let selected = ReferenceCatalog::bundled()
        .unwrap()
        .resolve_firmware("x70-110")
        .unwrap();
    let facts = selected.remote_facts(None).unwrap();
    assert_eq!(facts.firmware.menus.root_count, 188);
    assert_eq!(facts.firmware.menus.cursor.fields.cursor, 8);
    assert!(
        facts
            .control_ids
            .debug_keys
            .iter()
            .any(|key| key.name == "EXE_M")
    );
    assert!(facts.physical_controls.control("menu_ok_button").is_ok());
    assert!(facts.menu_manifest.is_none());
    let roundtrip: Facts = serde_json::from_value(serde_json::to_value(&facts).unwrap()).unwrap();
    assert_eq!(roundtrip, facts);
}

#[test]
fn mismatched_build_and_invalid_scanout_fail_before_use() {
    let selected = ReferenceCatalog::bundled()
        .unwrap()
        .resolve_firmware("x70-110")
        .unwrap();
    let mut facts = selected.remote_facts(None).unwrap();
    let mut value = serde_json::to_value(&facts).unwrap();
    value["firmware"]["scanout"]["stride_bytes"] = 1.into();
    assert!(
        serde_json::from_value::<Facts>(value)
            .unwrap()
            .validate()
            .is_err()
    );
    let manifest: BuildMenuManifest = serde_json::from_value(serde_json::json!({
        "$schema": MENU_MANIFEST_SCHEMA_URI, "schema": 1,
        "firmware_id": "different-100", "firmware_sha256": "a".repeat(64),
        "elf_sha256": "b".repeat(64), "artifacts": [], "items": [],
        "symbols": {}, "memory": [], "owner_constants": {},
        "walkthrough": { "menu": null, "qfn": null, "binding": null, "canvas": null,
            "choice_sets": {}, "binding_choices": {}, "canvas_hosts": {},
            "requirements": [], "option_requirements": [], "screen_requirements": [] }
    }))
    .unwrap();
    facts.menu_manifest = Some(manifest);
    assert!(facts.validate().unwrap_err().contains("different firmware"));
}

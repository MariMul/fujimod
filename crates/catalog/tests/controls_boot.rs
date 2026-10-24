use fujimod_catalog::ReferenceCatalog;
use fujimod_catalog_controls::BootMode;
use std::path::PathBuf;

#[test]
fn confirmed_combinations_load_from_controls() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../catalog");
    for catalog in [
        ReferenceCatalog::load(root).unwrap(),
        ReferenceCatalog::bundled().unwrap(),
    ] {
        let x70 = catalog
            .resolve_firmware("x70-110")
            .unwrap()
            .physical_controls
            .unwrap();
        let x70 = fujimod_catalog_controls::PhysicalControls::decode_document(&x70, "x70")
            .unwrap();
        assert_eq!(x70.boot[&BootMode::Service].usb, Some(true));
        assert_eq!(
            x70.boot[&BootMode::Service].hold[1].stage.as_deref(),
            Some("full")
        );
        let xm5 = catalog
            .resolve_firmware("xm5-130")
            .unwrap()
            .physical_controls
            .unwrap();
        let xm5 = fujimod_catalog_controls::PhysicalControls::decode_document(&xm5, "xm5")
            .unwrap();
        assert!(xm5.boot.is_empty());
    }
}

#[test]
fn startup_controls_and_shutter_stages_are_checked() {
    let catalog = ReferenceCatalog::bundled().unwrap();
    let original = catalog
        .resolve_firmware("x70-110")
        .unwrap()
        .physical_controls
        .unwrap();
    let original = fujimod_catalog_controls::PhysicalControls::decode_document(&original, "x70")
        .unwrap();
    let mut controls = original.clone();
    controls.boot.get_mut(&BootMode::Sd).unwrap().hold[0].id = "missing".into();
    assert!(controls.validate().is_err());
    let mut controls = original.clone();
    controls.boot.get_mut(&BootMode::Service).unwrap().hold[1].stage = Some("missing".into());
    assert!(controls.validate().is_err());
}

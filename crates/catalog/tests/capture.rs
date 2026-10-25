use fujimod_catalog::ReferenceCatalog;

#[test]
fn capture_contracts_resolve_with_the_selected_firmware() {
    let catalog = ReferenceCatalog::bundled().unwrap();
    for (firmware, kind, count) in [
        ("xm5-130", "serial-nand", 6),
        ("x100vi-131", "serial-nor", 7),
    ] {
        let selection = catalog.resolve_firmware(firmware).unwrap();
        let flash = selection.flash.unwrap();
        flash.validate().unwrap();
        assert_eq!(flash.firmware_id, firmware);
        assert_eq!(flash.device.kind, kind);
        assert_eq!(flash.code_ranges.len(), count);
        assert_eq!(
            (flash.window.address, flash.window.size),
            (0xf0000000, 0x8000000)
        );
        let state = selection.state.unwrap();
        state.validate().unwrap();
        assert_eq!(state.firmware_id, firmware);
        let dram = state
            .regions
            .iter()
            .find(|region| region.name == "dram")
            .unwrap();
        assert_eq!(
            (dram.address, dram.size, dram.constant),
            (0x4000, 0x7fffc000, false)
        );
        assert!(
            state
                .regions
                .iter()
                .any(|r| r.name == "dram" && !r.constant)
        );
        assert!(
            state
                .regions
                .iter()
                .any(|r| r.name == "boot-sram" && r.address == 0xff800000)
        );
        assert!(!state.regions.iter().any(|r| r.name == "boot-rom"));
    }
    for firmware in ["x70-110", "xe2-411", "xm1-150"] {
        let state = catalog.resolve_firmware(firmware).unwrap().state.unwrap();
        let rom = state.regions.iter().find(|r| r.name == "boot-rom").unwrap();
        assert_eq!(
            (rom.address, rom.size, rom.constant),
            (0x80000000, 16384, true)
        );
        state.validate().unwrap();
    }
}

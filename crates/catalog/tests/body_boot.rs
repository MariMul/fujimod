use fujimod_catalog::{
    ReferenceCatalog,
    boot::{BootGroup, BootStatus},
};
use std::path::PathBuf;

#[test]
fn startup_facts_load_without_promoting_candidates_or_shared_body_siblings() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../catalog");
    for catalog in [
        ReferenceCatalog::load(root).unwrap(),
        ReferenceCatalog::bundled().unwrap(),
    ] {
        let xm5 = catalog.boot_options("xm5").unwrap();
        assert_eq!(
            xm5.boot_options["sd-entry-candidate"].status,
            BootStatus::Candidate
        );
        assert_eq!(
            catalog.boot_options("xpro2").unwrap().boot_options["sd-download"].status,
            BootStatus::Superseded
        );
        let xq = catalog.boot_options("xq").unwrap();
        assert_eq!(xq.boot_options["sd-download"].models, ["XQ1"]);
        let gfx = catalog.boot_options("gfx50sii").unwrap();
        assert_eq!(gfx.boot_options["sd-download"].group, BootGroup::SdBoot);
        let x70 = catalog.boot_options("x70").unwrap();
        assert_eq!(x70.boot_options["service-mode"].group, BootGroup::Service);
    }
}

#[test]
fn unqualified_dials_remain_unresolved_and_citations_are_required() {
    let catalog = ReferenceCatalog::bundled().unwrap();
    let mut xt2 = catalog.boot_options("xt2").unwrap().clone();
    let option = xt2.boot_options.get_mut("sd-download").unwrap();
    assert!(
        option
            .entry
            .hold
            .iter()
            .any(|key| key.label == "Command dial" && key.control_id.is_none())
    );
    option.evidence[0].pages.clear();
    assert!(xt2.validate().is_err());
}

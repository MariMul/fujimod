use fujimod_catalog::{Body, Soc};
use std::{fs, path::PathBuf};

#[test]
fn declared_hardware_profiles_decode_and_validate() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../catalog");
    for entry in fs::read_dir(root.join("bodies")).unwrap() {
        let path = entry.unwrap().path().join("body.json");
        let body: Body = serde_json::from_slice(&fs::read(&path).unwrap())
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        body.validate(&body.id)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }
    for entry in fs::read_dir(root.join("soc")).unwrap() {
        let path = entry.unwrap().path().join("soc.json");
        let soc: Soc = serde_json::from_slice(&fs::read(&path).unwrap())
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        soc.validate(&soc.id)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }
}

#[test]
fn hardware_graph_requires_closed_nodes_and_complete_ordering() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("../../../catalog/bodies/xm1/body.json")).unwrap();
    let graph = &mut value["hardware"]["profiles"]["xm1-rec"]["graph"];
    graph["devices"][0]["kind"] = "invented-device".into();
    assert!(serde_json::from_value::<Body>(value.clone()).is_err());
    let graph = &mut value["hardware"]["profiles"]["xm1-rec"]["graph"];
    graph["devices"][0]["kind"] = "BootControls".into();
    graph["attachment_order"].as_array_mut().unwrap().pop();
    let body: Body = serde_json::from_value(value).unwrap();
    assert!(body.validate(&body.id).unwrap_err().contains("order"));
}

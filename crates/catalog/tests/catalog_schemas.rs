use jsonschema::validator_for;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn json_files(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory).expect("catalog directory") {
        let path = entry.expect("catalog entry").path();
        if path.is_dir() {
            files.extend(json_files(&path));
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            files.push(path);
        }
    }
    files.sort();
    files
}

#[test]
fn published_emulator_and_firmware_documents_validate_their_declared_schema() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let catalog = package.join("../../catalog");
    let mut files = json_files(&catalog.join("emulator"));
    files.extend(json_files(&catalog.join("firmwares")));

    for path in files {
        let document: Value = serde_json::from_slice(&fs::read(&path).expect("catalog document"))
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let schema_reference = document
            .get("$schema")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{}: missing $schema", path.display()));
        let schema_path = path
            .parent()
            .expect("document parent")
            .join(schema_reference)
            .canonicalize()
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(schema_path.starts_with(catalog.canonicalize().unwrap()));
        let schema: Value =
            serde_json::from_slice(&fs::read(&schema_path).expect("declared catalog schema"))
                .unwrap_or_else(|error| panic!("{}: {error}", schema_path.display()));
        let validator = validator_for(&schema)
            .unwrap_or_else(|error| panic!("{}: {error}", schema_path.display()));
        let errors: Vec<_> = validator.iter_errors(&document).collect();
        assert!(
            errors.is_empty(),
            "{}: {}",
            path.display(),
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
}

#[test]
fn reference_and_run_configuration_documents_validate_generated_schemas() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let catalog = package.join("../../catalog");
    let schema_root = catalog.join("schemas");

    for (name, generated) in fujimod_catalog::schema::generated_documents() {
        let path = schema_root.join(name);
        let mut expected = serde_json::to_vec_pretty(&generated).expect("generated schema");
        expected.push(b'\n');
        let actual = fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(
            actual == expected,
            "{} differs from the Rust source types",
            path.display()
        );
    }

    let mut files = Vec::new();
    for layer in ["soc", "parts", "bodies", "firmware"] {
        files.extend(json_files(&catalog.join(layer)));
    }
    files.extend(json_files(&package.join("../../configurations/emulator")));

    for path in files {
        let document: Value = serde_json::from_slice(&fs::read(&path).expect("catalog document"))
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let schema_reference = document
            .get("$schema")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{}: missing $schema", path.display()));
        let schema_name = schema_reference
            .strip_prefix("https://fujimod.com/schemas/")
            .unwrap_or_else(|| {
                panic!(
                    "{}: schema must use a stable schema identity",
                    path.display()
                )
            });
        assert!(
            schema_name.ends_with(".schema.json") && !schema_name.contains('/'),
            "{}: schema identity is not a named generated schema",
            path.display()
        );
        let schema_path = schema_root.join(schema_name);
        let schema: Value = serde_json::from_slice(
            &fs::read(&schema_path)
                .unwrap_or_else(|error| panic!("{}: {error}", schema_path.display())),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", schema_path.display()));
        let validator = validator_for(&schema)
            .unwrap_or_else(|error| panic!("{}: {error}", schema_path.display()));
        let errors: Vec<_> = validator.iter_errors(&document).collect();
        assert!(
            errors.is_empty(),
            "{}: {}",
            path.display(),
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
}

#[test]
fn bundled_reference_records_resolve_by_identity() {
    let catalog = fujimod_catalog::ReferenceCatalog::bundled().expect("bundled references");
    for (body, firmware) in [
        ("x100s", "x100s-121"),
        ("x100t", "x100t-111"),
        ("x20", "x20-103"),
        ("x30", "x30-103"),
        ("xe2", "xe2-411"),
        ("xe2s", "xe2s-111"),
        ("xm1", "xm1-150"),
        ("xq", "xq1-121"),
        ("xq", "xq2-101"),
        ("xt1", "xt1-552"),
        ("xt10", "xt10-131"),
    ] {
        catalog
            .resolve(body, firmware)
            .unwrap_or_else(|error| panic!("{body} / {firmware}: {error}"));
    }
}

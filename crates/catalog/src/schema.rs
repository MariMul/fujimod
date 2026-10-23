//! JSON Schema documents generated from the public catalog Rust types.

use schemars::JsonSchema;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Build the committed schema documents from the serde source types.
pub fn generated_documents() -> BTreeMap<&'static str, Value> {
    let mut documents = BTreeMap::new();
    documents.insert(
        "emulator-factory-inputs.generated.schema.json",
        schema::<crate::configuration::RunFactoryInputs>(
            crate::configuration::RUN_FACTORY_INPUTS_SCHEMA_URI,
        ),
    );
    documents.insert(
        "emulator-rehearsal.generated.schema.json",
        schema::<crate::rehearsal::Rehearsal>(crate::rehearsal::REHEARSAL_SCHEMA_URI),
    );
    documents.insert(
        "catalog-annotations.generated.schema.json",
        schema::<crate::provenance::CatalogAnnotations>(crate::provenance::ANNOTATIONS_SCHEMA_URI),
    );
    documents.insert(
        "emulator-hardware-inputs.generated.schema.json",
        schema::<crate::configuration::HardwareInputs>(
            crate::configuration::HARDWARE_INPUTS_SCHEMA_URI,
        ),
    );
    documents.insert(
        "emulator-firmware-inputs.generated.schema.json",
        schema::<crate::configuration::RunFirmwareInputs>(
            crate::configuration::RUN_FIRMWARE_INPUTS_SCHEMA_URI,
        ),
    );
    documents.insert(
        "settings.generated.schema.json",
        schema::<crate::settings::FirmwareSettings>(crate::settings::SETTINGS_SCHEMA_URI),
    );
    documents.insert(
        "bulk.generated.schema.json",
        schema::<crate::bulk::FirmwareBulk>(crate::bulk::BULK_SCHEMA_URI),
    );
    documents.insert(
        "first-start.generated.schema.json",
        schema::<crate::first_start::FirstStartMenu>(crate::first_start::FIRST_START_SCHEMA_URI),
    );
    documents.insert(
        "soc.schema.json",
        schema::<crate::reference::Soc>("https://fujimod.com/schemas/soc.schema.json"),
    );
    documents.insert(
        "part.schema.json",
        schema::<crate::reference::Part>("https://fujimod.com/schemas/part.schema.json"),
    );
    documents.insert(
        "body.schema.json",
        schema::<crate::reference::Body>("https://fujimod.com/schemas/body.schema.json"),
    );
    documents.insert(
        "firmware.schema.json",
        schema::<crate::reference::Firmware>("https://fujimod.com/schemas/firmware.schema.json"),
    );
    documents.insert(
        "emulator-run-configuration.schema.json",
        schema::<crate::configuration::RunConfiguration>(
            crate::configuration::RUN_CONFIGURATION_SCHEMA_URI,
        ),
    );
    documents.insert(
        "emulator-run-overlay.schema.json",
        schema::<crate::configuration::RunOverlay>(crate::configuration::RUN_OVERLAY_SCHEMA_URI),
    );
    documents.insert(
        "resolved-selection.schema.json",
        schema::<crate::reference::ResolvedSelection>(
            "https://fujimod.com/schemas/resolved-selection.schema.json",
        ),
    );
    documents.insert(
        "resolved-configuration.schema.json",
        schema::<crate::configuration::ResolvedConfiguration>(
            "https://fujimod.com/schemas/resolved-configuration.schema.json",
        ),
    );
    documents.insert(
        "remote-firmware.generated.schema.json",
        schema::<crate::remote::FirmwareFacts>(crate::remote::FIRMWARE_FACTS_SCHEMA_URI),
    );
    documents.insert(
        "build-menu.generated.schema.json",
        schema::<crate::remote::BuildMenuManifest>(crate::remote::MENU_MANIFEST_SCHEMA_URI),
    );
    documents.insert(
        "remote-facts.generated.schema.json",
        schema::<crate::remote::Facts>(
            "https://fujimod.com/schemas/remote-facts.generated.schema.json",
        ),
    );
    documents.insert(
        "remote-menus.generated.schema.json",
        schema::<crate::remote::MenuTables>(
            "https://fujimod.com/schemas/remote-menus.generated.schema.json",
        ),
    );
    documents.insert(
        "remote-resources.generated.schema.json",
        schema::<crate::remote::Resources>(
            "https://fujimod.com/schemas/remote-resources.generated.schema.json",
        ),
    );
    documents.insert(
        "remote-scanout.generated.schema.json",
        schema::<crate::remote::Scanout>(
            "https://fujimod.com/schemas/remote-scanout.generated.schema.json",
        ),
    );
    documents.insert(
        "remote-state.generated.schema.json",
        schema::<crate::reference::FirmwareObservation>(
            "https://fujimod.com/schemas/remote-state.generated.schema.json",
        ),
    );
    documents.insert(
        "remote-debug-keys.generated.schema.json",
        schema::<Vec<crate::remote::DebugKey>>(
            "https://fujimod.com/schemas/remote-debug-keys.generated.schema.json",
        ),
    );
    documents.insert(
        "physical-controls.schema.json",
        schema::<crate::controls::PhysicalControls>(crate::controls::PHYSICAL_CONTROLS_SCHEMA_URI),
    );
    documents.insert(
        "control-ids.schema.json",
        schema::<crate::controls::FirmwareControlIds>(crate::controls::CONTROL_IDS_SCHEMA_URI),
    );
    documents
}

fn schema<T: JsonSchema>(id: &str) -> Value {
    let mut document =
        serde_json::to_value(schemars::schema_for!(T)).expect("schemars schema serializes to JSON");
    let object = document
        .as_object_mut()
        .expect("schemars root schema is a JSON object");
    object.insert("$id".into(), Value::String(id.into()));
    if let Some(Value::Object(properties)) = object.get_mut("properties") {
        if properties.contains_key("$schema") {
            properties.insert("$schema".into(), schema_uri_property(id));
        }
    }
    document
}

fn schema_uri_property(id: &str) -> Value {
    let mut property = Map::new();
    property.insert("type".into(), Value::String("string".into()));
    property.insert("const".into(), Value::String(id.into()));
    property.insert(
        "description".into(),
        Value::String("Stable schema identifier for this document.".into()),
    );
    Value::Object(property)
}

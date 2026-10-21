//! JSON Schema documents generated from the public catalog Rust types.

use schemars::JsonSchema;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Build the committed schema documents from the serde source types.
pub fn generated_documents() -> BTreeMap<&'static str, Value> {
    let mut documents = BTreeMap::new();
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

use std::{env, fs, path::PathBuf};
fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let path = root.join("../../catalog/recovery/recipes.generated.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let bytes = fs::read(&path).expect("bundled recovery recipes");
    fs::write(out.join("recipes.generated.json"), bytes).unwrap();

    // The XP5 guest is a prebuilt binary asset; its layout becomes constants.
    let image = root.join("assets/xp5-guest.generated.bin");
    let layout = root.join("assets/xp5-guest.generated.json");
    println!("cargo:rerun-if-changed={}", image.display());
    println!("cargo:rerun-if-changed={}", layout.display());
    let layout: serde_json::Value =
        serde_json::from_slice(&fs::read(&layout).expect("XP5 guest layout")).unwrap();
    assert_eq!(layout["schema"], 1, "XP5 guest layout schema");
    let image_bytes = fs::metadata(&image).expect("XP5 guest image").len();
    assert_eq!(
        layout["image_bytes"].as_u64(),
        Some(image_bytes),
        "XP5 guest image size"
    );
    let mut rust = format!(
        "pub const IMAGE: &[u8] = include_bytes!({:?});\npub const PRELUDE_STACK_MAX: usize = {};\n",
        image.display().to_string(),
        layout["prelude_stack_max"].as_u64().unwrap()
    );
    for (name, offset) in layout["symbols"].as_object().unwrap() {
        rust += &format!(
            "pub const {}: usize = {:#x};\n",
            name.to_uppercase(),
            offset.as_u64().unwrap()
        );
    }
    rust += "pub const EXTERNALS: &[(usize, u32, &str)] = &[\n";
    for external in layout["externals"].as_array().unwrap() {
        rust += &format!(
            "    ({:#x}, {}, {:?}),\n",
            external["offset"].as_u64().unwrap(),
            external["kind"].as_u64().unwrap(),
            external["symbol"].as_str().unwrap()
        );
    }
    rust += "];\n";
    fs::write(out.join("xp5_guest.rs"), rust).unwrap();
}

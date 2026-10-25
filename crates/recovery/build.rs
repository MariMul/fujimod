use std::{env, fs, path::PathBuf};
fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let path = root.join("../../catalog/recovery/recipes.generated.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let bytes = fs::read(&path).expect("bundled recovery recipes");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("recipes.generated.json"),
        bytes,
    )
    .unwrap();
}

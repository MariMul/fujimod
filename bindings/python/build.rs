fn main() {
    let root = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    fujimod_bindgen::Api::build(&root, &out);
}

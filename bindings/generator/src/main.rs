use std::path::PathBuf;
fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args
        .next()
        .expect("usage: fujimod-bindgen --out|--python-types DIRECTORY");
    assert!(
        matches!(mode.as_str(), "--out" | "--python-types"),
        "unknown output kind"
    );
    let out = PathBuf::from(args.next().expect("output directory"));
    assert!(args.next().is_none(), "unexpected argument");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    fujimod_bindgen::Api::read(&root)
        .and_then(|api| {
            if mode == "--python-types" {
                api.write_python_types(&out)
            } else {
                api.write_web(&out)
            }
        })
        .unwrap_or_else(|e| panic!("{e}"));
}

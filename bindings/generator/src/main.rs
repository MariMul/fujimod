use std::path::PathBuf;
fn main() {
    let mut args = std::env::args().skip(1);
    assert_eq!(
        args.next().as_deref(),
        Some("--out"),
        "usage: fujimod-bindgen --out DIRECTORY"
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
        .and_then(|api| api.write_web(&out))
        .unwrap_or_else(|e| panic!("{e}"));
}

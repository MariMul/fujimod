use std::{env, path::PathBuf, process::Command};

fn main() {
    for source in [
        "tsconfig.json",
        "peer.ts",
        "wire.ts",
        "pipeline.ts",
        "share-traffic.ts",
        "usb-exporter.ts",
        "exporter.ts",
        "types.ts",
        "webusb.ts",
    ] {
        println!("cargo:rerun-if-changed=web/{source}");
    }
    let wasm = env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32");
    let relay = env::var_os("CARGO_FEATURE_RELAY").is_some();
    if !wasm && !relay {
        return;
    }
    for output in ["peer", "wire", "pipeline", "share-traffic", "usb-exporter", "exporter"] {
        println!("cargo:rerun-if-changed=web/build/{output}.js");
    }
    println!("cargo:rerun-if-changed=../../node_modules/typescript/package.json");
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let compiler = manifest.join("../../node_modules/typescript/bin/tsc");
    assert!(
        compiler.is_file(),
        "USB browser assets require the declared TypeScript compiler; run npm ci at the repository root"
    );
    let status = Command::new("node")
        .arg(compiler)
        .arg("--project")
        .arg(manifest.join("web/tsconfig.json"))
        .status()
        .expect("USB browser assets require Node.js; install the toolchain declared in mise.toml");
    assert!(
        status.success(),
        "USB browser TypeScript compilation failed"
    );
}

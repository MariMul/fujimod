fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten") {
        let directory = std::env::var("CAMERA_UNICORN_STATIC_DIR")
            .expect("CAMERA_UNICORN_STATIC_DIR must name the Emscripten Unicorn build");
        println!("cargo:rustc-link-search=native={directory}");
        for name in ["unicorn", "arm-softmmu", "unicorn-common"] {
            println!("cargo:rustc-link-lib=static={name}");
            println!("cargo:rerun-if-changed={directory}/lib{name}.a");
        }
    }
    println!("cargo:rerun-if-env-changed=CAMERA_UNICORN_STATIC_DIR");
    println!("cargo:rerun-if-changed=build.rs");
}

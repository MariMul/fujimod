fn main() {
    #[cfg(feature = "emulator")]
    build_jpeg(std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten"));
    let root = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    fujimod_bindgen::Api::build(&root, &out);
    println!("cargo:rerun-if-changed=build.rs");
}

#[cfg(feature = "emulator")]
fn build_jpeg(wasm: bool) {
    let mut jpeg = cc::Build::new();
    jpeg.file("src/emulator/jpeg_scanlines.c").warnings(true).pic(true);
    if wasm {
        jpeg.flag("-sMAIN_MODULE=2")
            .flag("-sUSE_LIBJPEG=1")
            .flag("-fwasm-exceptions")
            .flag("-sSUPPORT_LONGJMP=wasm");
    } else {
        for path in pkg_config::Config::new()
            .cargo_metadata(false)
            .probe("libjpeg")
            .expect("libjpeg development library")
            .include_paths
        {
            jpeg.include(path);
        }
    }
    jpeg.compile("fujimod-jpeg-scanlines");
    if wasm {
        let archive = std::process::Command::new("emcc")
            .args([
                "-sMAIN_MODULE=2",
                "-sUSE_LIBJPEG=1",
                "--print-file-name=libjpeg.a",
            ])
            .output()
            .expect("Emscripten libjpeg archive");
        assert!(archive.status.success());
        let archive = std::path::PathBuf::from(String::from_utf8(archive.stdout).unwrap().trim());
        println!(
            "cargo:rustc-link-search=native={}",
            archive
                .parent()
                .expect("libjpeg archive directory")
                .display()
        );
        println!("cargo:rustc-link-lib=static=jpeg");
    } else {
        pkg_config::Config::new()
            .probe("libjpeg")
            .expect("libjpeg development library");
    }
    println!("cargo:rerun-if-changed=src/emulator/jpeg_scanlines.c");
}

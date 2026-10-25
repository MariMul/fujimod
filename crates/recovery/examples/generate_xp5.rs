//! Generate a relocatable Marble loader from guest/xp5, without camera data.
use object::{Object, ObjectSection, ObjectSymbol};
use std::{env, fs, path::PathBuf, process::Command};
fn run(program: &str, args: &[&str], work: &std::path::Path) {
    let output = Command::new(program)
        .args(args)
        .current_dir(work)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let work = root.join("../../target/recovery-xp5-template");
    fs::create_dir_all(&work).unwrap();
    for entry in fs::read_dir(root.join("guest/xp5")).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), work.join(entry.file_name())).unwrap();
    }
    fs::write(
        work.join("bindings.c"),
        "#include \"board.h\"\nconst struct setup_sd_board setup_sd_board = {0};\n",
    )
    .unwrap();
    let cc = env::var("XP5_CC").expect("set XP5_CC to GNU AArch64 GCC 9.2.1");
    let version = Command::new(&cc).arg("-dumpfullversion").output().unwrap();
    assert_eq!(String::from_utf8_lossy(&version.stdout).trim(), "9.2.1");
    let mut objects = Vec::new();
    for source in [
        "bindings.c",
        "loader.c",
        "board.c",
        "sd_host.c",
        "sd_card.c",
        "sd_pio.c",
        "fat32.c",
        "crc32.c",
        "boot.S",
    ] {
        let output = format!("{source}.o");
        run(
            &cc,
            &[
                "-ffreestanding",
                "-fno-pic",
                "-fno-pie",
                "-fno-stack-protector",
                "-fno-lto",
                "-fno-common",
                "-ffunction-sections",
                "-fdata-sections",
                "-fno-asynchronous-unwind-tables",
                "-O1",
                "-mcpu=cortex-a53",
                "-fno-omit-frame-pointer",
                "-mgeneral-regs-only",
                "-c",
                source,
                "-o",
                &output,
            ],
            &work,
        );
        objects.push(output);
    }
    fs::write(
        work.join("loader.ld"),
        r#"
ENTRY(setup_sd_initialize)
SECTIONS {
 .text 0x100000 : { *(.text .text.*) }
 .rodata : { *(.rodata .rodata.*) }
 .data : { *(.data .data.*) }
 .bss : { *(.bss .bss.* COMMON) }
 /DISCARD/ : { *(.comment) *(.note.GNU-stack) *(.ARM.attributes) *(.eh_frame*) }
}
PROVIDE(boot_setup_initialize = 0x0c0000);
PROVIDE(boot_setup_delay = 0x0c0200);
PROVIDE(boot_setup_ram_entry_resume = 0x0c0400);
"#,
    )
    .unwrap();
    let mut args = vec![
        "-T",
        "loader.ld",
        "--gc-sections",
        "--emit-relocs",
        "-o",
        "loader.elf",
    ];
    args.extend(objects.iter().map(String::as_str));
    run(&env::var("XP5_LD").expect("set XP5_LD"), &args, &work);
    let bytes = fs::read(work.join("loader.elf")).unwrap();
    let elf = object::File::parse(bytes.as_slice()).unwrap();
    let symbols: std::collections::BTreeMap<_, _> = elf
        .symbols()
        .filter_map(|s| Some((s.name().ok()?.to_string(), s.address())))
        .collect();
    let mut image = Vec::new();
    let base = 0x100000;
    for name in [".text", ".rodata", ".data", ".bss"] {
        if let Some(section) = elf.section_by_name(name) {
            let offset = (section.address() - base) as usize;
            image.resize(offset + section.size() as usize, 0);
            let data = section.data().unwrap();
            image[offset..offset + data.len()].copy_from_slice(data);
        }
    }
    let mut out = String::from(
        "// Generated from guest/xp5 by cargo run -p fujimod-recovery --example generate_xp5.\n",
    );
    for name in [
        "setup_sd_initialize",
        "setup_sd_request",
        "setup_sd_progress",
        "setup_sd_board",
        "a64_boot_ready",
        "a64_boot_zero_gate",
    ] {
        out += &format!(
            "pub const {}: usize = {:#x};\n",
            name.to_uppercase(),
            symbols[name] - base
        );
    }
    out += "pub const IMAGE: &[u8] = &[\n";
    for chunk in image.chunks(24) {
        out += "    ";
        for byte in chunk {
            out += &format!("{byte},");
        }
        out += "\n";
    }
    out += "];\n";
    out += "// External instruction relocations: (offset, ARM64 kind, stock symbol).\npub const EXTERNALS: &[(usize,u32,&str)] = &[\n";
    for section in elf.sections() {
        for (at, rel) in section.relocations() {
            let object::RelocationTarget::Symbol(index) = rel.target() else {
                continue;
            };
            let symbol = elf.symbol_by_index(index).unwrap();
            let name = symbol.name().unwrap();
            if !name.starts_with("boot_setup_") {
                continue;
            }
            let object::RelocationFlags::Elf { r_type } = rel.flags() else {
                panic!("ELF relocation expected");
            };
            assert!(
                [282, 283].contains(&r_type),
                "Unexpected external relocation {r_type}"
            );
            out += &format!("    ({:#x},{r_type},{name:?}),\n", at - base);
        }
    }
    out += "];\n";
    let target = root.join("src/xp5.generated.rs");
    if env::args().any(|s| s == "--check") {
        assert_eq!(fs::read_to_string(target).unwrap(), out);
    } else {
        let temporary = target.with_extension(format!("rs.{}.tmp", std::process::id()));
        fs::write(&temporary, out).unwrap();
        if let Err(error) = fs::rename(&temporary, target) {
            let _ = fs::remove_file(temporary);
            panic!("Cannot install generated template: {error}");
        }
    }
}

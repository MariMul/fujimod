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
        "flash_boot.c",
        "flash_entry.c",
        "flash_entry.S",
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
                "-O2",
                "-mstrict-align",
                "-fstack-usage",
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
ENTRY(xp5_flash_entry)
SECTIONS {
 .text 0x100000 : { KEEP(*flash_entry.S.o(.text)) KEEP(*loader.c.o(.text.setup_sd_initialize)) *(.text .text.*) }
 .rodata : { *(.rodata .rodata.*) }
 .data : { *(.data .data.*) }
 .bss : { *(.bss .bss.* COMMON) }
 /DISCARD/ : { *(.comment) *(.note.GNU-stack) *(.ARM.attributes) *(.eh_frame*) }
}
PROVIDE(boot_setup_initialize = 0x0c0000);
PROVIDE(boot_setup_delay = 0x0c0200);
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
    // Verify the pre-setup C graph from our compiled guest, including GCC's
    // stack reports. The success hold may call only the located setup timer
    // after flash preparation; other stock calls belong to the post-DDR loader.
    let functions: std::collections::BTreeMap<_, _> = elf
        .symbols()
        .filter(|s| s.kind() == object::SymbolKind::Text && s.size() > 0 && s.address() >= base)
        .map(|s| (s.address(), (s.name().unwrap().to_string(), s.size())))
        .collect();
    let mut frames = std::collections::BTreeMap::new();
    for source in ["flash_boot.c", "flash_entry.c"] {
        for line in fs::read_to_string(work.join(format!("{source}.su")))
            .unwrap()
            .lines()
        {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields[2], "static", "Unbounded prelude stack");
            frames.insert(
                fields[0].rsplit(':').next().unwrap().to_string(),
                fields[1].parse::<usize>().unwrap(),
            );
        }
    }
    fn stack_depth(
        at: u64,
        base: u64,
        image: &[u8],
        functions: &std::collections::BTreeMap<u64, (String, u64)>,
        frames: &std::collections::BTreeMap<String, usize>,
        path: &mut Vec<u64>,
    ) -> usize {
        assert!(!path.contains(&at), "Recursive prelude");
        let (name, size) = functions.get(&at).expect("Prelude has an external call");
        path.push(at);
        let mut child = 0;
        for (n, bytes) in image[(at - base) as usize..(at + size - base) as usize]
            .chunks_exact(4)
            .enumerate()
        {
            let word = u32::from_le_bytes(bytes.try_into().unwrap());
            assert!(
                ![0xd61f0000, 0xd63f0000, 0xd69f0000].contains(&(word & 0xfffffc1f)),
                "Indirect prelude transfer"
            );
            if word & 0x7c000000 == 0x14000000 {
                let target = (at as i64
                    + n as i64 * 4
                    + ((((word & 0x3ffffff) ^ 0x2000000) as i64 - 0x2000000) * 4))
                    as u64;
                if word & 0x80000000 != 0 || !(at..at + size).contains(&target) {
                    child = child.max(stack_depth(target, base, image, functions, frames, path));
                }
            }
        }
        path.pop();
        frames.get(name).expect("Missing GCC stack report") + child
    }
    let maximum = stack_depth(
        symbols["xp5_flash_prepare"],
        base,
        &image,
        &functions,
        &frames,
        &mut Vec::new(),
    );
    assert!(
        maximum + 512 <= (symbols["xp5_flash_stack_top"] - symbols["xp5_flash_stack"]) as usize,
        "Prelude stack overflow"
    );
    for section in elf.sections() {
        for (_, rel) in section.relocations() {
            let object::RelocationFlags::Elf { r_type } = rel.flags() else {
                panic!("ELF relocation expected")
            };
            assert!(
                [261, 274, 275, 277, 278, 282, 283, 284, 285, 286, 299].contains(&r_type),
                "Nonportable guest relocation {r_type}"
            );
        }
    }
    let mut out = String::from(
        "// Generated from guest/xp5 by cargo run -p fujimod-recovery --example generate_xp5.\n",
    );
    out += &format!("pub const PRELUDE_STACK_MAX: usize = {maximum};\n");
    for name in [
        "xp5_flash_entry",
        "xp5_flash_configuration",
        "xp5_flash_setup_branch",
        "xp5_flash_stack",
        "xp5_flash_stack_top",
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

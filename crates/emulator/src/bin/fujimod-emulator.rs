//! Native boot command; every binary input is selected by path.
use fujimod_emulator::boot::{self, Options};
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, process::ExitCode};

const HELP: &str = "Usage: fujimod-emulator boot --body BODY --firmware FIRMWARE \\\n  --update UPDATE.DAT --boot-rom ROM.BIN --unicorn LIBRARY \\\n  [--nand IMAGE] [--settings MCU.BIN] [--screenshot LCD.PNG] \\\n  [--instruction-limit COUNT]\n\n\
Supported boot: x70 / x70-110. The ROM dump is required.\n\
Without --nand or --settings, persistent settings start blank.\n\
NAND accepts a committed emulator image or a full raw medium matching the update.\n\
Settings accepts a sixteen-byte MCU persistent packet.\n\
First-start menus are accepted through MENU/OK ADC input.\n\
Prints a JSON boot report; exits 0 at REC, 2 at a stopped frontier.\n";

fn parse(
    args: impl Iterator<Item = OsString>,
) -> Result<Option<(Options, Option<PathBuf>, u64)>, String> {
    let mut args = args.peekable();
    if args.peek().is_some_and(|a| a == "--help" || a == "-h") {
        return Ok(None);
    }
    if args.next().as_deref() != Some(std::ffi::OsStr::new("boot")) {
        return Err(HELP.into());
    }
    let mut values = BTreeMap::new();
    while let Some(key) = args.next() {
        let key = key.to_str().ok_or("option name is not UTF-8")?.to_owned();
        if ![
            "--body",
            "--firmware",
            "--update",
            "--boot-rom",
            "--unicorn",
            "--nand",
            "--settings",
            "--screenshot",
            "--instruction-limit",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("unknown option: {key}"));
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {key}"))?;
        if values.insert(key.clone(), value).is_some() {
            return Err(format!("duplicate option: {key}"));
        }
    }
    let mut take = |key: &str| {
        values
            .remove(key)
            .ok_or_else(|| format!("required option: {key}"))
    };
    let options = Options {
        body: take("--body")?
            .into_string()
            .map_err(|_| "body must be UTF-8")?,
        firmware: take("--firmware")?
            .into_string()
            .map_err(|_| "firmware must be UTF-8")?,
        update: take("--update")?.into(),
        boot_rom: take("--boot-rom")?.into(),
        unicorn: take("--unicorn")?.into(),
        nand: None,
        settings: None,
    };
    let options = Options {
        nand: values.remove("--nand").map(PathBuf::from),
        settings: values.remove("--settings").map(PathBuf::from),
        ..options
    };
    let screenshot = values.remove("--screenshot").map(PathBuf::from);
    let limit = values
        .remove("--instruction-limit")
        .map(|v| {
            v.to_str()
                .ok_or("instruction limit must be UTF-8")?
                .parse::<u64>()
                .map_err(|_| "instruction limit must be an unsigned integer")
        })
        .transpose()?
        .unwrap_or(2_000_000_000);
    if limit == 0 {
        return Err("instruction limit must be positive".into());
    }
    Ok(Some((options, screenshot, limit)))
}
fn run() -> Result<bool, String> {
    let Some((options, path, limit)) = parse(std::env::args_os().skip(1))? else {
        print!("{HELP}");
        return Ok(true);
    };
    let mut session = boot::open(options)?;
    let report = boot::to_rec(&mut session, limit)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    if let Some(path) = path {
        std::fs::write(&path, boot::screenshot(&session)?)
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(report.status == "rec")
}
fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(2),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_inputs_and_unique_options_are_required() {
        let args = |args: &[&str]| {
            args.iter()
                .map(OsString::from)
                .collect::<Vec<_>>()
                .into_iter()
        };
        assert!(parse(args(&["--help"])).unwrap().is_none());
        assert!(parse(args(&["boot", "--body", "x70"])).is_err());
        assert!(parse(args(&["boot", "--body", "x70", "--body", "x70"])).is_err());
        assert!(
            parse(args(&[
                "boot",
                "--body",
                "x70",
                "--firmware",
                "x70-110",
                "--update",
                "a.dat",
                "--boot-rom",
                "rom.bin",
                "--unicorn",
                "lib.so",
                "--instruction-limit",
                "0"
            ]))
            .is_err()
        );
        let (options, _, limit) = parse(args(&[
            "boot",
            "--body",
            "x70",
            "--firmware",
            "x70-110",
            "--update",
            "a.dat",
            "--boot-rom",
            "rom.bin",
            "--unicorn",
            "lib.so",
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(options.boot_rom, PathBuf::from("rom.bin"));
        assert_eq!(limit, 2_000_000_000);
        assert!(options.nand.is_none());
    }
}

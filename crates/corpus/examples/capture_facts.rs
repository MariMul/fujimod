//! Batch JSON-lines locator driver. Inputs name the index, hardware and decoded
//! update packaging; stdout contains facts or per-locator diagnostics.
use fujimod_corpus::{Store, locators};
use serde::Deserialize;
use serde_json::json;
use std::io::{self, BufRead};
#[derive(Deserialize)]
struct Input {
    firmware_id: String,
    processor: String,
    jedec_id: Option<u32>,
    update_path: Option<String>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open(std::env::args().nth(1).ok_or("index filename required")?)?;
    for line in io::stdin().lock().lines() {
        let input: Input = serde_json::from_str(&line?)?;
        let flash = (|| -> Result<_, String> {
            if !matches!(input.processor.as_str(), "xp5" | "exr2") {
                return locators::flash(
                    &store,
                    &input.firmware_id,
                    &input.processor,
                    input.jedec_id,
                    0,
                    0,
                );
            }
            let path = input.update_path.as_ref().ok_or("update input missing")?;
            let mut bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let origin = fujimod_firmware::outer_header(&bytes)
                .map_err(|e| e.to_string())?
                .payload_offset as u64;
            for byte in &mut bytes {
                *byte ^= 255;
            }
            if input.processor == "exr2" {
                let control = locators::exr2_control(&bytes[origin as usize..])?;
                return locators::exr2_flash(&store, &input.firmware_id, origin, control);
            }
            let (origin, control) =
                locators::image_offsets(&store, &input.firmware_id, &bytes, origin)?;
            let value = locators::flash(
                &store,
                &input.firmware_id,
                &input.processor,
                input.jedec_id,
                origin,
                control,
            )?;
            let control = value["control"]["offset"]
                .as_u64()
                .ok_or("missing flash control")? as usize;
            if bytes
                .get(origin as usize + control..origin as usize + control + 32768)
                .is_none()
            {
                return Err("truncated flash comparison control".into());
            }
            if value["device"]["kind"] == "serial-nand"
                && bytes.get(origin as usize + control..origin as usize + control + 8)
                    != Some(&[0x50, 1, 0, 0, 0x80, 0, 0, 0])
            {
                return Err("NAND controller parameter page differs".into());
            }
            Ok(value)
        })();
        let rom = locators::boot_rom(&input.processor);
        println!(
            "{}",
            json!({"firmware_id":input.firmware_id,"flash":flash.as_ref().ok(),"flash_error":flash.as_ref().err(),"boot_rom":rom.as_ref().ok(),"boot_rom_error":rom.as_ref().err()})
        );
    }
    Ok(())
}

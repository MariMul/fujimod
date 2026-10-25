//! Offline construction and catalog inspection. No device access.
use fujimod_recovery::{Recipe, availability};
use std::{env, fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("list") => {
            let recipes: Vec<Recipe> = serde_json::from_str(include_str!(concat!(
                env!("OUT_DIR"),
                "/recipes.generated.json"
            )))?;
            let rows = recipes
                .iter()
                .map(|r| availability(&r.firmware_id))
                .collect::<Result<Vec<_>, _>>()?;
            println!("{}", serde_json::to_string_pretty(&rows)?);
        }
        Some(command @ ("build" | "startup" | "stage-one")) if args.len() == 4 => {
            let recipe = Recipe::bundled(&args[1])?.ok_or("Missing recovery recipe")?;
            let source = fs::read(&args[2])?;
            let files = if command == "build" {
                recipe.build(&source)?
            } else if command == "stage-one" {
                recipe.build_stage_one(&source)?
            } else {
                recipe.build_startup(&source)?
            };
            let output = PathBuf::from(&args[3]);
            fs::create_dir(&output)?;
            let mut rows = Vec::new();
            for file in files {
                fs::write(output.join(&file.name), &file.bytes)?;
                rows.push(serde_json::json!({"name":file.name,"size":file.bytes.len(),"sha256":fujimod_firmware::sha256(&file.bytes)}));
            }
            println!("{}", serde_json::to_string_pretty(&rows)?);
        }
        _ => {
            return Err(
                "Usage: fujimod-recovery list | build|startup|stage-one FIRMWARE DAT NEW_DIRECTORY"
                    .into(),
            );
        }
    }
    Ok(())
}

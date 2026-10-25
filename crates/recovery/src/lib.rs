//! SD card construction from explicit source identities and located recipes.
#![forbid(unsafe_code)]
pub mod image;
mod xp4;
use image::{File, Packaging, Plan, Result, require};
use serde::{Deserialize, Serialize};
pub use xp4::{ColdStart, FileApi, Setup};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Generation {
    Exr2,
    Xp4,
    Xp5,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub firmware_id: String,
    pub source_size: usize,
    pub source_sha256: String,
    pub update_filename: String,
    pub generation: Generation,
    pub plan: Option<Plan>,
    pub packaging: Option<Packaging>,
    pub setup: Option<Setup>,
    pub cold_start: Option<ColdStart>,
    pub file_api: Option<FileApi>,
    pub sd_boot: Option<serde_json::Value>,
    pub firmware_update: Option<serde_json::Value>,
    pub missing: Vec<String>,
}
impl Recipe {
    pub fn bundled(firmware_id: &str) -> Result<Option<Self>> {
        let recipes: Vec<Self> = serde_json::from_str(include_str!(concat!(
            env!("OUT_DIR"),
            "/recipes.generated.json"
        )))
        .map_err(|e| image::Error::Invalid(e.to_string()))?;
        Ok(recipes.into_iter().find(|r| r.firmware_id == firmware_id))
    }
    /// Boot files retain the stock updater and storage drivers. DAT is copied
    /// verbatim to its required root filename, including signed containers.
    pub fn build(&self, source: &[u8]) -> Result<Vec<File>> {
        require(
            self.missing.is_empty(),
            &format!("Missing recovery facts: {}", self.missing.join(", ")),
        )?;
        self.build_startup(source)
    }
    /// Construct established startup files independently of updater readiness.
    pub fn build_startup(&self, source: &[u8]) -> Result<Vec<File>> {
        require(
            source.len() == self.source_size,
            "Update size differs from recipe",
        )?;
        image::hash(source, &self.source_sha256)?;
        let inverted = fujimod_firmware::invert(source);
        let plan = self
            .plan
            .as_ref()
            .ok_or_else(|| image::Error::Invalid("Missing RAM image plan".into()))?;
        let mut files =
            match self.generation {
                Generation::Exr2 => {
                    let packaging = self.packaging.as_ref().ok_or_else(|| {
                        image::Error::Invalid("Missing EXR II SD packaging".into())
                    })?;
                    packaging.package(&inverted, plan, image::transform(&inverted, plan)?)?
                }
                Generation::Xp4 => {
                    let cold = self
                        .cold_start
                        .as_ref()
                        .ok_or_else(|| image::Error::Invalid("Missing XP4 cold startup".into()))?;
                    let setup = self
                        .setup
                        .as_ref()
                        .ok_or_else(|| image::Error::Invalid("Missing XP4 DDR setup".into()))?;
                    let api = self.file_api.as_ref().ok_or_else(|| {
                        image::Error::Invalid("Missing Topaz filesystem ABI".into())
                    })?;
                    let plan = xp4::cold_plan(&inverted, plan, cold)?;
                    let ram = image::transform(&inverted, &plan)?;
                    let init = xp4::ffinit(&inverted, setup, api, &ram, api.load_address)?;
                    vec![
                        File::new("ffinit.bin", init)?,
                        File::new("ffboot.bin", ram)?,
                    ]
                }
                Generation::Xp5 => {
                    return Err(image::Error::Invalid(
                        "Missing XP5 post-setup SD loader and handoff facts".into(),
                    ));
                }
            };
        require(
            !files
                .iter()
                .any(|f| f.name.eq_ignore_ascii_case(&self.update_filename)),
            "DAT filename collides with boot files",
        )?;
        files.push(File::new(&self.update_filename, source.to_vec())?);
        Ok(files)
    }
}
#[derive(Debug, Serialize)]
pub struct Availability {
    pub firmware_id: String,
    pub generation: Option<Generation>,
    pub can_build: bool,
    pub missing: Vec<String>,
    pub output_filenames: Vec<String>,
    pub sd_boot: Option<serde_json::Value>,
    pub firmware_update: Option<serde_json::Value>,
}
pub fn availability(firmware_id: &str) -> Result<Availability> {
    let id = firmware_id;
    let recipe = Recipe::bundled(id)?;
    let sd_boot = recipe.as_ref().and_then(|r| r.sd_boot.clone());
    let firmware_update = recipe.as_ref().and_then(|r| r.firmware_update.clone());
    let mut missing = recipe
        .as_ref()
        .map(|r| r.missing.clone())
        .unwrap_or_else(|| vec!["recovery recipe".into()]);
    if sd_boot.is_none() {
        missing.push("SD-boot button combination".into());
    }
    if firmware_update.is_none() {
        missing.push("firmware-update button combination".into());
    }
    let mut output_filenames = Vec::new();
    if let Some(recipe) = &recipe {
        match recipe.generation {
            Generation::Exr2 => {
                if let Some(packaging) = &recipe.packaging {
                    output_filenames.extend([
                        packaging.header_filename.clone(),
                        packaging.image_filename.clone(),
                    ]);
                }
            }
            Generation::Xp4 => output_filenames.extend(["ffinit.bin".into(), "ffboot.bin".into()]),
            Generation::Xp5 => {
                output_filenames.extend(["marble_init.bin".into(), "ffboot.bin".into()])
            }
        }
        output_filenames.push(recipe.update_filename.clone());
    }
    Ok(Availability {
        firmware_id: id.into(),
        generation: recipe.map(|r| r.generation),
        can_build: missing.is_empty(),
        missing,
        output_filenames,
        sd_boot,
        firmware_update,
    })
}

#[cfg(test)]
mod tests;

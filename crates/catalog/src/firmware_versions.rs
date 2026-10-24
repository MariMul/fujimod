//! Public releases and the internal version returned by the service interface.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Stable schema identity for firmware version mappings.
pub const SCHEMA_URI: &str = "https://fujimod.com/schemas/firmware-versions.generated.schema.json";

/// One known firmware release, independent of emulation or bulk-read support.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    all(target_arch = "wasm32", not(target_os = "emscripten")),
    derive(tsify::Tsify)
)]
pub struct FirmwareVersion {
    /// Stable identity of the firmware release.
    pub firmware_id: String,
    /// Camera model reported by the service interface.
    pub product: String,
    /// Public release version shown on Fujifilm's download page.
    pub version: String,
    /// Internal version returned by the service interface for this release.
    pub internal_version: String,
    /// SHA-256 of the official update from which the mapping was extracted.
    pub update_sha256: String,
    /// Official camera firmware page with current releases and update instructions.
    pub download_url: String,
}

/// Generated mappings for known updates, including bodies without emulator profiles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FirmwareVersions {
    /// Schema identity.
    #[serde(rename = "$schema")]
    pub schema_uri: String,
    /// Document version.
    pub schema: u32,
    /// Known releases. Internal versions are comparable only within one product.
    pub releases: Vec<FirmwareVersion>,
}

fn version(value: &str) -> Option<(u8, u8)> {
    let (major, minor) = value.split_once('.')?;
    if major.is_empty()
        || minor.len() != 2
        || !major
            .bytes()
            .chain(minor.bytes())
            .all(|v| v.is_ascii_digit())
    {
        return None;
    }
    let major = major.parse::<u8>().ok()?;
    let minor = minor.parse::<u8>().ok()?;
    (major <= 99).then_some((major, minor))
}

impl FirmwareVersions {
    /// Decode and check identities, version ordering and official download links.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let document: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if document.schema_uri != SCHEMA_URI || document.schema != 1 {
            return Err("firmware versions schema differs".into());
        }
        let mut ids = BTreeSet::new();
        let mut products: BTreeMap<&str, BTreeMap<(u8, u8), (u8, u8)>> = BTreeMap::new();
        for release in &document.releases {
            fujimod_catalog_identity::validate_id(&release.firmware_id)?;
            fujimod_catalog_identity::validate_sha256(&release.update_sha256)?;
            let public = version(&release.version).ok_or("invalid public firmware version")?;
            let internal =
                version(&release.internal_version).ok_or("invalid internal firmware version")?;
            if release.product.trim().is_empty()
                || release.product.trim() != release.product
                || !ids.insert(&release.firmware_id)
                || !release
                    .download_url
                    .starts_with("https://www.fujifilm-x.com/")
                || !release
                    .download_url
                    .contains("/support/download/firmware/cameras/")
            {
                return Err(format!(
                    "invalid firmware version identity {}",
                    release.firmware_id
                ));
            }
            if products
                .entry(&release.product)
                .or_default()
                .insert(public, internal)
                .is_some()
            {
                return Err(format!(
                    "repeated public firmware version for {}",
                    release.product
                ));
            }
        }
        for (product, releases) in products {
            let mut previous = None;
            for internal in releases.into_values() {
                if previous.is_some_and(|value| value > internal) {
                    return Err(format!("internal firmware versions decrease for {product}"));
                }
                previous = Some(internal);
            }
        }
        Ok(document)
    }

    /// The same mappings compiled into Rust and WebAssembly consumers.
    pub fn bundled() -> Result<Self, String> {
        Self::decode(include_bytes!(
            "../../../catalog/firmware-versions.generated.json"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_map_covers_known_models_and_distinguishes_internal_from_public_versions() {
        let map = FirmwareVersions::bundled().unwrap();
        for (id, internal, public) in [
            ("xt1-552", "2.39", "5.52"),
            ("xh1-215", "1.90", "2.15"),
            ("xm1-101", "1.61", "1.01"),
        ] {
            let entry = map.releases.iter().find(|r| r.firmware_id == id).unwrap();
            assert_eq!(
                (&*entry.internal_version, &*entry.version),
                (internal, public)
            );
        }
    }

    #[test]
    fn only_same_product_ordering_is_checked_and_decreasing_internal_versions_fail() {
        let mut document = FirmwareVersions::bundled().unwrap();
        let entry = document
            .releases
            .iter_mut()
            .find(|r| r.firmware_id == "xm1-150")
            .unwrap();
        entry.internal_version = "1.60".into();
        assert!(
            FirmwareVersions::decode(&serde_json::to_vec(&document).unwrap())
                .unwrap_err()
                .contains("decrease for X-M1")
        );
        assert_eq!(version("1.09"), Some((1, 9)));
        for text in ["1.9", "1.90beta", "nan", "1.100", "-1.00"] {
            assert_eq!(version(text), None);
        }
    }
}

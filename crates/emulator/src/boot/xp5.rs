//! XP5 device topology resolved from the selected SoC profile.
//!
//! The profile owns device addresses and options. Run inputs own the external
//! values supplied to those devices; neither the caller nor a board runner
//! needs a second copy of the register-window map.

use fujimod_catalog::{
    profiles::RegisterWindow,
    reference::{ResolvedSelection, SocBlockKind},
};
use fujimod_emulator_devices::graph::DeviceWindow;

fn attachment_kind(
    selection: &ResolvedSelection,
    window: &RegisterWindow,
) -> Result<&'static str, String> {
    let attachment = window.attachment.as_deref().ok_or_else(|| {
        format!(
            "XP5 window {} has no device kind or attachment",
            window.name
        )
    })?;
    let block = selection
        .soc
        .blocks
        .iter()
        .find(|block| block.id == attachment)
        .ok_or_else(|| {
            format!(
                "XP5 window {} attaches to missing SoC block {attachment}",
                window.name
            )
        })?;
    match block.kind {
        SocBlockKind::Timer => Ok("timer"),
        SocBlockKind::SerialController => Ok("spi-controller"),
        kind => Err(format!(
            "XP5 window {} cannot attach SoC block {} of kind {:?}",
            window.name, block.id, kind
        )),
    }
}

/// Convert one catalog window into the device graph's checked register window.
fn device_window(
    selection: &ResolvedSelection,
    window: &RegisterWindow,
) -> Result<DeviceWindow, String> {
    let kind = window
        .kind
        .as_deref()
        .map(str::to_owned)
        .map(Ok)
        .unwrap_or_else(|| attachment_kind(selection, window).map(str::to_owned))?;
    let options = window
        .options
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| format!("XP5 window {} options: {error}", window.name))?
        .map(|value| {
            toml::Value::try_from(value)
                .map_err(|error| format!("XP5 window {} options: {error}", window.name))
        })
        .transpose()?
        .and_then(|value| value.as_table().cloned())
        .unwrap_or_default();
    Ok(DeviceWindow {
        base: window.base,
        size: window.size,
        kind,
        options,
    })
}

/// Resolve the selected XP5 profile's explicit device windows.
pub fn windows(
    selection: &ResolvedSelection,
    profile_id: &str,
) -> Result<Vec<DeviceWindow>, String> {
    let profile = selection
        .soc
        .hardware
        .as_ref()
        .and_then(|hardware| hardware.profiles.get(profile_id))
        .ok_or_else(|| {
            format!(
                "SoC {} has no hardware profile {profile_id}",
                selection.soc.id
            )
        })?;
    let windows = profile
        .windows
        .as_ref()
        .ok_or_else(|| format!("SoC profile {profile_id} declares no register windows"))?;
    windows
        .iter()
        .map(|window| device_window(selection, window))
        .collect()
}

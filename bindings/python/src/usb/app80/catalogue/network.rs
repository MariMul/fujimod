//! Value conversion only; all command layout and validation are in the core.
use super::*;

#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
pub(super) enum RadioAction {
    Primary {
        mode: RadioMode,
        channel: RadioChannel,
        power: RadioPower,
        option: values::Toggle,
    },
    Secondary {
        mode: RadioMode,
        channel: RadioChannel,
    },
}
impl From<RadioAction> for core::RadioAction {
    fn from(value: RadioAction) -> Self {
        match value {
            RadioAction::Primary {
                mode,
                channel,
                power,
                option,
            } => Self::Primary {
                mode: mode.into(),
                channel: channel.inner,
                power: power.inner,
                option: option.into(),
            },
            RadioAction::Secondary { mode, channel } => Self::Secondary {
                mode: mode.into(),
                channel: channel.inner,
            },
        }
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<RadioAction>()?;

    Ok(())
}

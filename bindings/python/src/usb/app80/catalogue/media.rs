//! Media enum/value conversion only; validation and codecs live in Rust USB.
use super::*;

#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
pub(super) enum DisplayOptionAction {
    IdlePins {},
    ForwardPair { first: u8, second: u8 },
}
impl From<DisplayOptionAction> for core::DisplayOptionAction {
    fn from(value: DisplayOptionAction) -> Self {
        match value {
            DisplayOptionAction::IdlePins {} => Self::IdlePins,
            DisplayOptionAction::ForwardPair { first, second } => {
                Self::ForwardPair { first, second }
            }
        }
    }
}

#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
pub(super) enum SoundAction {
    Clip {
        clip: SoundClip,
        format: SoundFormat,
    },
    Track {
        track: SoundTrack,
        operation: SoundOperation,
        format: SoundFormat,
    },
    Noop {},
}
impl From<SoundAction> for core::SoundAction {
    fn from(value: SoundAction) -> Self {
        match value {
            SoundAction::Clip { clip, format } => Self::Clip {
                clip: clip.inner,
                format: format.into(),
            },
            SoundAction::Track {
                track,
                operation,
                format,
            } => Self::Track {
                track: track.into(),
                operation: operation.into(),
                format: format.into(),
            },
            SoundAction::Noop {} => Self::Noop,
        }
    }
}

#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
pub(super) enum HdmiAction {
    ResetStub {},
    Packet {
        packet: HdmiPacket,
    },
    Triplet {
        first: HdmiValue,
        second: HdmiValue,
        third: HdmiValue,
    },
}
impl From<HdmiAction> for core::HdmiAction {
    fn from(value: HdmiAction) -> Self {
        match value {
            HdmiAction::ResetStub {} => Self::ResetStub,
            HdmiAction::Packet { packet } => Self::Packet(packet.inner),
            HdmiAction::Triplet {
                first,
                second,
                third,
            } => Self::Triplet {
                first: first.inner,
                second: second.inner,
                third: third.inner,
            },
        }
    }
}
#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
pub(super) enum ChargeAction {
    Control { enabled: Toggle },
    EnableAndSetFlag { enabled: Toggle },
}
impl From<ChargeAction> for core::ChargeAction {
    fn from(value: ChargeAction) -> Self {
        match value {
            ChargeAction::Control { enabled } => Self::Control(enabled.into()),
            ChargeAction::EnableAndSetFlag { enabled } => Self::EnableAndSetFlag(enabled.into()),
        }
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<DisplayOptionAction>()?;

    module.add_class::<SoundAction>()?;

    module.add_class::<HdmiAction>()?;
    module.add_class::<ChargeAction>()?;

    Ok(())
}

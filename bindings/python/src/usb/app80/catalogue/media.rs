//! Media enum/value conversion only; validation and codecs live in Rust USB.
use super::*;
macro_rules! choice {
 ($name:ident,{$($variant:ident),+})=>{
  #[pyclass(eq,from_py_object,module="fujimod.usb.app80")]
  #[derive(Clone,Copy,Debug,PartialEq)]
  pub(super) enum $name {$($variant),+}
  impl From<$name> for core::$name {fn from(value:$name)->Self {match value {$($name::$variant=>Self::$variant),+}}}
 };
}
choice!(Toggle,{Disabled,Enabled});
choice!(DisplayMode,{Option0,Option1});
choice!(SoundFormat,{Format1,Format2,Format3});
choice!(SoundTrack,{Track2,Track3,Track4});
choice!(SoundOperation,{First,Second,Third});
choice!(ChargeQuery,{Presence,State});
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
pub(super) struct SoundClip {
    inner: core::SoundClip,
}
#[pymethods]
impl SoundClip {
    #[new]
    fn new(py: Python<'_>, value: u8) -> PyResult<Self> {
        core::SoundClip::new(value)
            .map(|inner| Self { inner })
            .map_err(|e| command_error(py, e))
    }
    #[getter]
    fn value(&self) -> u8 {
        self.inner.value()
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
pub(super) struct HdmiValue {
    inner: core::HdmiValue,
}
#[pymethods]
impl HdmiValue {
    #[new]
    fn new(py: Python<'_>, value: u16) -> PyResult<Self> {
        core::HdmiValue::new(value)
            .map(|inner| Self { inner })
            .map_err(|e| command_error(py, e))
    }
    #[getter]
    fn value(&self) -> u16 {
        self.inner.value()
    }
}
#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
pub(super) struct HdmiPacket {
    inner: core::HdmiPacket,
}
#[pymethods]
impl HdmiPacket {
    #[new]
    fn new(py: Python<'_>, data: Vec<u8>) -> PyResult<Self> {
        core::HdmiPacket::new(&data)
            .map(|inner| Self { inner })
            .map_err(|e| command_error(py, e))
    }
    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.inner.data())
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
macro_rules! result {
 ($name:ident,{$($variant:ident),+})=>{
  #[pyclass(frozen,from_py_object,module="fujimod.usb.app80")]
  #[derive(Clone)]
  pub(super) enum $name {$($variant{}),+,Unknown{raw_status:u8}}
  impl From<core::$name> for $name {fn from(value:core::$name)->Self {match value {$(core::$name::$variant=>Self::$variant{}),+,core::$name::Unknown(raw_status)=>Self::Unknown{raw_status}}}}
 };
}
result!(ChargeResult,{Success,Failed});
result!(ChargePresence,{Failed,Absent,Present});
result!(ReportedChargeState,{Failed,Value0,Values2To5,Value6,OtherValue});
choice!(AudioRoute,{Request82,Request81});
choice!(AudioPlayback,{AllLanes,ZeroSecondLane,ZeroFirstLane});
choice!(StorageErase,{Kind1Regions,Kind0And2Regions,Kind0And1And2Regions,AuxiliaryRegion,ResetProfiles});
result!(AudioResult,{Success,Failed});
result!(StorageEraseResult,{Success,Failed});
result!(StorageStatus,{Zero,Value4,Value8,Value16,Value32});
#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
pub(super) struct AudioCount {
    pub(super) inner: core::AudioCount,
}
#[pymethods]
impl AudioCount {
    #[new]
    fn new(py: Python<'_>, value: u8) -> PyResult<Self> {
        core::AudioCount::new(value)
            .map(|inner| Self { inner })
            .map_err(|e| command_error(py, e))
    }
    #[getter]
    fn value(&self) -> u8 {
        self.inner.value()
    }
}
pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<AudioCount>()?;
    module.add_class::<AudioRoute>()?;
    module.add_class::<AudioPlayback>()?;
    module.add_class::<AudioResult>()?;
    module.add_class::<StorageErase>()?;
    module.add_class::<StorageEraseResult>()?;
    module.add_class::<StorageStatus>()?;
    module.add_class::<Toggle>()?;
    module.add_class::<DisplayMode>()?;
    module.add_class::<DisplayOptionAction>()?;
    module.add_class::<SoundClip>()?;
    module.add_class::<SoundFormat>()?;
    module.add_class::<SoundTrack>()?;
    module.add_class::<SoundOperation>()?;
    module.add_class::<SoundAction>()?;
    module.add_class::<HdmiValue>()?;
    module.add_class::<HdmiPacket>()?;
    module.add_class::<HdmiAction>()?;
    module.add_class::<ChargeAction>()?;
    module.add_class::<ChargeQuery>()?;
    module.add_class::<ChargeResult>()?;
    module.add_class::<ChargePresence>()?;
    module.add_class::<ReportedChargeState>()?;
    Ok(())
}

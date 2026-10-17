//! Value conversion only; all command layout and validation are in the core.
use super::*;
macro_rules! choice {
 ($name:ident,{$($variant:ident),+})=>{
  #[pyclass(eq,from_py_object,module="fujimod.usb.app80")]
  #[derive(Clone,Copy,Debug,PartialEq)] pub(super) enum $name {$($variant),+}
  impl From<$name> for core::$name {fn from(value:$name)->Self {match value {$($name::$variant=>Self::$variant),+}}}
 };
}
choice!(RadioMode,{Mode0,Mode1});
choice!(NandSpare,{Omit,Include});
macro_rules! quantity {
    ($name:ident) => {
        #[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
        #[derive(Clone)]
        pub(super) struct $name {
            inner: core::$name,
        }
        #[pymethods]
        impl $name {
            #[new]
            fn new(py: Python<'_>, value: u8) -> PyResult<Self> {
                core::$name::new(value)
                    .map(|inner| Self { inner })
                    .map_err(|e| command_error(py, e))
            }
            #[getter]
            fn value(&self) -> u8 {
                self.inner.value()
            }
        }
    };
}
quantity!(RadioChannel);
quantity!(RadioPower);
#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
pub(super) struct PeerAddress {
    pub(super) inner: core::PeerAddress,
}
#[pymethods]
impl PeerAddress {
    #[new]
    fn new(py: Python<'_>, data: Vec<u8>) -> PyResult<Self> {
        core::PeerAddress::new(&data)
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
pub(super) enum RadioAction {
    Primary {
        mode: RadioMode,
        channel: RadioChannel,
        power: RadioPower,
        option: media::Toggle,
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
macro_rules! result {
 ($name:ident,{$($variant:ident),+})=>{
  #[pyclass(frozen,from_py_object,module="fujimod.usb.app80")]
  #[derive(Clone)] pub(super) enum $name {$($variant{}),+,Unknown{raw_status:u8}}
  impl From<core::$name> for $name {fn from(value:core::$name)->Self {match value {$(core::$name::$variant=>Self::$variant{}),+,core::$name::Unknown(raw_status)=>Self::Unknown{raw_status}}}}
 };
}
result!(NandReadResult,{Success,RejectedOrWaitError,OtherError});
result!(Option41State,{Zero,NonZero});
pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NandSpare>()?;
    module.add_class::<NandReadResult>()?;
    module.add_class::<RadioMode>()?;
    module.add_class::<RadioChannel>()?;
    module.add_class::<RadioPower>()?;
    module.add_class::<RadioAction>()?;
    module.add_class::<PeerAddress>()?;
    module.add_class::<Option41State>()?;
    Ok(())
}

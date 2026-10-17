//! Typed input factories; encoding stays in fujimod-usb.
use super::*;
macro_rules! choice {
 ($name:ident,{$($variant:ident),+})=>{
  #[wasm_bindgen] #[derive(Clone,Copy)] pub enum $name {$($variant),+}
  impl $name {pub(super) fn parse(value:JsValue)->Result<core::$name,JsValue>{
   let value=integer(value,u32::MAX)?;
   $(if value==Self::$variant as u32 {return Ok(core::$name::$variant);})+
   Err(fail(ErrorKind::InvalidArgument,"unknown radio/storage enum choice"))
  }}
 };
}
choice!(NandSpare,{Omit,Include});
choice!(RadioMode,{Mode0,Mode1});
macro_rules! quantity {
    ($name:ident) => {
        #[wasm_bindgen]
        pub struct $name {
            inner: core::$name,
        }
        #[wasm_bindgen]
        impl $name {
            #[wasm_bindgen(constructor)]
            pub fn new(
                #[wasm_bindgen(unchecked_param_type = "number")] value: JsValue,
            ) -> Result<Self, JsValue> {
                core::$name::new(integer(value, u8::MAX.into())? as u8)
                    .map(|inner| Self { inner })
                    .map_err(invalid)
            }
            #[wasm_bindgen(getter)]
            pub fn value(&self) -> u8 {
                self.inner.value()
            }
        }
    };
}
quantity!(RadioChannel);
quantity!(RadioPower);
#[wasm_bindgen]
pub struct PeerAddress {
    pub(super) inner: core::PeerAddress,
}
#[wasm_bindgen]
impl PeerAddress {
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "Uint8Array")] data: JsValue,
    ) -> Result<Self, JsValue> {
        core::PeerAddress::new(&error::buffer(data)?)
            .map(|inner| Self { inner })
            .map_err(invalid)
    }
    #[wasm_bindgen(getter, unchecked_return_type = "Uint8Array")]
    pub fn data(&self) -> JsValue {
        error::bytes(self.inner.data())
    }
}
#[wasm_bindgen]
pub struct RadioAction {
    pub(super) inner: core::RadioAction,
}
#[wasm_bindgen]
impl RadioAction {
    pub fn primary(
        #[wasm_bindgen(unchecked_param_type = "RadioMode")] mode: JsValue,
        channel: &RadioChannel,
        power: &RadioPower,
        #[wasm_bindgen(unchecked_param_type = "Toggle")] option: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::RadioAction::Primary {
                mode: RadioMode::parse(mode)?,
                channel: channel.inner,
                power: power.inner,
                option: media::Toggle::parse(option)?,
            },
        })
    }
    pub fn secondary(
        #[wasm_bindgen(unchecked_param_type = "RadioMode")] mode: JsValue,
        channel: &RadioChannel,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::RadioAction::Secondary {
                mode: RadioMode::parse(mode)?,
                channel: channel.inner,
            },
        })
    }
}
macro_rules! result {
 ($kind:ident,$name:ident,{$($variant:ident),+})=>{
  #[wasm_bindgen] #[derive(Clone,Copy)] pub enum $kind {$($variant),+,Unknown}
  impl $kind {pub(super) fn value(value:core::$name)->JsValue {match value {
   $(core::$name::$variant=>record(Self::$variant as u32,&[])),+,
   core::$name::Unknown(raw)=>record(Self::Unknown as u32,&[("rawStatus",raw.into())]),
  }}}
 };
}
result!(NandReadResultKind,NandReadResult,{Success,RejectedOrWaitError,OtherError});
result!(Option41StateKind,Option41State,{Zero,NonZero});

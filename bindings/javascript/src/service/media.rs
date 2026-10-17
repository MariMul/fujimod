//! Media choices and value factories. No protocol encoding in this binding.
use super::*;
macro_rules! choice {
 ($name:ident,{$($variant:ident),+})=>{
  #[wasm_bindgen] #[derive(Clone,Copy)] pub enum $name {$($variant),+}
  impl $name {pub(super) fn parse(value:JsValue)->Result<core::$name,JsValue>{
   let value=integer(value,u32::MAX)?;
   $(if value==Self::$variant as u32 {return Ok(core::$name::$variant);})+
   Err(fail(ErrorKind::InvalidArgument,"unknown media enum choice"))
  }}
 };
}
choice!(Toggle,{Disabled,Enabled});
choice!(DisplayMode,{Option0,Option1});
choice!(SoundFormat,{Format1,Format2,Format3});
choice!(SoundTrack,{Track2,Track3,Track4});
choice!(SoundOperation,{First,Second,Third});
choice!(ChargeQuery,{Presence,State});
#[wasm_bindgen]
pub struct DisplayOptionAction {
    pub(super) inner: core::DisplayOptionAction,
}
#[wasm_bindgen]
impl DisplayOptionAction {
    #[wasm_bindgen(js_name=idlePins)]
    pub fn idle_pins() -> Self {
        Self {
            inner: core::DisplayOptionAction::IdlePins,
        }
    }
    #[wasm_bindgen(js_name=forwardPair)]
    pub fn forward_pair(
        #[wasm_bindgen(unchecked_param_type = "number")] first: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] second: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::DisplayOptionAction::ForwardPair {
                first: integer(first, u8::MAX.into())? as u8,
                second: integer(second, u8::MAX.into())? as u8,
            },
        })
    }
}
#[wasm_bindgen]
pub struct SoundClip {
    inner: core::SoundClip,
}
#[wasm_bindgen]
impl SoundClip {
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "number")] value: JsValue,
    ) -> Result<Self, JsValue> {
        core::SoundClip::new(integer(value, u8::MAX.into())? as u8)
            .map(|inner| Self { inner })
            .map_err(invalid)
    }
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> u8 {
        self.inner.value()
    }
}
#[wasm_bindgen]
pub struct SoundAction {
    pub(super) inner: core::SoundAction,
}
#[wasm_bindgen]
impl SoundAction {
    pub fn clip(
        clip: &SoundClip,
        #[wasm_bindgen(unchecked_param_type = "SoundFormat")] format: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::SoundAction::Clip {
                clip: clip.inner,
                format: SoundFormat::parse(format)?,
            },
        })
    }
    pub fn track(
        #[wasm_bindgen(unchecked_param_type = "SoundTrack")] track: JsValue,
        #[wasm_bindgen(unchecked_param_type = "SoundOperation")] operation: JsValue,
        #[wasm_bindgen(unchecked_param_type = "SoundFormat")] format: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::SoundAction::Track {
                track: SoundTrack::parse(track)?,
                operation: SoundOperation::parse(operation)?,
                format: SoundFormat::parse(format)?,
            },
        })
    }
    pub fn noop() -> Self {
        Self {
            inner: core::SoundAction::Noop,
        }
    }
}
#[wasm_bindgen]
pub struct HdmiValue {
    inner: core::HdmiValue,
}
#[wasm_bindgen]
impl HdmiValue {
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "number")] value: JsValue,
    ) -> Result<Self, JsValue> {
        core::HdmiValue::new(integer(value, u16::MAX.into())? as u16)
            .map(|inner| Self { inner })
            .map_err(invalid)
    }
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> u16 {
        self.inner.value()
    }
}
#[wasm_bindgen]
pub struct HdmiPacket {
    inner: core::HdmiPacket,
}
#[wasm_bindgen]
impl HdmiPacket {
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "Uint8Array")] data: JsValue,
    ) -> Result<Self, JsValue> {
        core::HdmiPacket::new(&error::buffer(data)?)
            .map(|inner| Self { inner })
            .map_err(invalid)
    }
    #[wasm_bindgen(getter, unchecked_return_type = "Uint8Array")]
    pub fn data(&self) -> JsValue {
        error::bytes(self.inner.data())
    }
}
#[wasm_bindgen]
pub struct HdmiAction {
    pub(super) inner: core::HdmiAction,
}
#[wasm_bindgen]
impl HdmiAction {
    #[wasm_bindgen(js_name=resetStub)]
    pub fn reset_stub() -> Self {
        Self {
            inner: core::HdmiAction::ResetStub,
        }
    }
    pub fn packet(packet: &HdmiPacket) -> Self {
        Self {
            inner: core::HdmiAction::Packet(packet.inner),
        }
    }
    pub fn triplet(first: &HdmiValue, second: &HdmiValue, third: &HdmiValue) -> Self {
        Self {
            inner: core::HdmiAction::Triplet {
                first: first.inner,
                second: second.inner,
                third: third.inner,
            },
        }
    }
}
#[wasm_bindgen]
pub struct ChargeAction {
    pub(super) inner: core::ChargeAction,
}
#[wasm_bindgen]
impl ChargeAction {
    pub fn control(
        #[wasm_bindgen(unchecked_param_type = "Toggle")] enabled: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::ChargeAction::Control(Toggle::parse(enabled)?),
        })
    }
    #[wasm_bindgen(js_name=enableAndSetFlag)]
    pub fn enable_and_set_flag(
        #[wasm_bindgen(unchecked_param_type = "Toggle")] enabled: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::ChargeAction::EnableAndSetFlag(Toggle::parse(enabled)?),
        })
    }
}
macro_rules! result {
 ($kind:ident,$core:ident,{$($variant:ident),+})=>{
  #[wasm_bindgen] #[derive(Clone,Copy)] pub enum $kind {$($variant),+,Unknown}
  impl $kind {pub(super) fn value(value:core::$core)->JsValue {match value {
   $(core::$core::$variant=>record(Self::$variant as u32,&[])),+,
   core::$core::Unknown(raw)=>record(Self::Unknown as u32,&[("rawStatus",raw.into())]),
  }}}
 };
}
result!(ChargeResultKind,ChargeResult,{Success,Failed});
result!(ChargePresenceKind,ChargePresence,{Failed,Absent,Present});
result!(ReportedChargeStateKind,ReportedChargeState,{Failed,Value0,Values2To5,Value6,OtherValue});

//! Media choices and value factories. No protocol encoding in this binding.
use super::*;

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

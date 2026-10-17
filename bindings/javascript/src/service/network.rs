//! Typed input factories; encoding stays in fujimod-usb.
use super::*;

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
                option: values::Toggle::parse(option)?,
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

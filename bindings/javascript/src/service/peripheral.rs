//! Peripheral enum factories; all argument layouts remain in the Rust core.
use super::*;

#[wasm_bindgen]
pub struct RegisterAction {
    pub(super) inner: core::RegisterAction,
}
#[wasm_bindgen]
impl RegisterAction {
    #[wasm_bindgen(js_name=audioValue)]
    pub fn audio_value(
        #[wasm_bindgen(unchecked_param_type = "number")] value: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::RegisterAction::AudioValue {
                value: integer(value, u8::MAX.into())? as u8,
            },
        })
    }
    #[wasm_bindgen(js_name=displayFirst)]
    pub fn display_first(
        #[wasm_bindgen(unchecked_param_type = "number")] selector: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] value: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::RegisterAction::DisplayFirst {
                selector: integer(selector, u8::MAX.into())? as u8,
                value: integer(value, u16::MAX.into())? as u16,
            },
        })
    }
    #[wasm_bindgen(js_name=displayStub)]
    pub fn display_stub(
        #[wasm_bindgen(unchecked_param_type = "number")] selector: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] value: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::RegisterAction::DisplayStub {
                selector: integer(selector, u8::MAX.into())? as u8,
                value: integer(value, u16::MAX.into())? as u16,
            },
        })
    }
    #[wasm_bindgen(js_name=displaySecond)]
    pub fn display_second(
        #[wasm_bindgen(unchecked_param_type = "number")] selector: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] value: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::RegisterAction::DisplaySecond {
                selector: integer(selector, u8::MAX.into())? as u8,
                value: integer(value, u16::MAX.into())? as u16,
            },
        })
    }
    #[wasm_bindgen(js_name=powerWrite)]
    pub fn power_write(
        #[wasm_bindgen(unchecked_param_type = "number")] register: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] value: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::RegisterAction::PowerWrite {
                register: integer(register, u16::MAX.into())? as u16,
                value: integer(value, u8::MAX.into())? as u8,
            },
        })
    }
    #[wasm_bindgen(js_name=positionPair)]
    pub fn position_pair(
        #[wasm_bindgen(unchecked_param_type = "number")] first: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] second: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::RegisterAction::PositionPair {
                first: integer(first, u16::MAX.into())? as u16,
                second: integer(second, u16::MAX.into())? as u16,
            },
        })
    }
    #[wasm_bindgen(js_name=noopFirst)]
    pub fn noop_first() -> Self {
        Self {
            inner: core::RegisterAction::NoopFirst,
        }
    }
    #[wasm_bindgen(js_name=noopSecond)]
    pub fn noop_second() -> Self {
        Self {
            inner: core::RegisterAction::NoopSecond,
        }
    }
}

#[wasm_bindgen]
pub struct I2cAction {
    pub(super) inner: core::I2cAction,
}
#[wasm_bindgen]
impl I2cAction {
    pub fn read(
        #[wasm_bindgen(unchecked_param_type = "I2cPort")] port: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] command: JsValue,
        length: &I2cLength,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::I2cAction::Read {
                port: I2cPort::parse(port)?,
                command: integer(command, u8::MAX.into())? as u8,
                length: length.inner,
            },
        })
    }
    pub fn write(
        #[wasm_bindgen(unchecked_param_type = "I2cPort")] port: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] command: JsValue,
        payload: &I2cPayload,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::I2cAction::Write {
                port: I2cPort::parse(port)?,
                command: integer(command, u8::MAX.into())? as u8,
                payload: payload.inner,
            },
        })
    }
    #[wasm_bindgen(js_name=writeSingle)]
    pub fn write_single(
        #[wasm_bindgen(unchecked_param_type = "number")] command: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] value: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::I2cAction::WriteSingle {
                command: integer(command, u8::MAX.into())? as u8,
                value: integer(value, u8::MAX.into())? as u8,
            },
        })
    }
}

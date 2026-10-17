//! Typed App80 values and conversions. Wire codecs live in fujimod-usb.
use super::*;
use defs::{ServiceCodec, ServiceDefinition, ServiceModes};
use error::{integer, property, record};
use usb::app80::catalogue as core;
mod peripheral;

#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum CommandFailureKind {
    Unsupported,
    Ambiguous,
    Unavailable,
    Argument,
    OversizeArguments,
}
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum ArgumentKind {
    PinBank,
    ProfileIndex,
    WordAlignment,
    WordAddress,
    I2cLength,
}
impl From<core::ArgumentKind> for ArgumentKind {
    fn from(value: core::ArgumentKind) -> Self {
        match value {
            core::ArgumentKind::PinBank => Self::PinBank,
            core::ArgumentKind::ProfileIndex => Self::ProfileIndex,
            core::ArgumentKind::WordAlignment => Self::WordAlignment,
            core::ArgumentKind::WordAddress => Self::WordAddress,
            core::ArgumentKind::I2cLength => Self::I2cLength,
        }
    }
}
fn invalid(source: core::CommandError) -> JsValue {
    use CommandFailureKind as K;
    use core::CommandError as E;
    let error = fail(ErrorKind::ServiceCommand, &source.to_string());
    let reason = match source {
        E::Unsupported(codec) => record(
            K::Unsupported as u32,
            &[("codec", (ServiceCodec::from(codec) as u32).into())],
        ),
        E::Ambiguous(codec) => record(
            K::Ambiguous as u32,
            &[("codec", (ServiceCodec::from(codec) as u32).into())],
        ),
        E::Unavailable(code) => record(K::Unavailable as u32, &[("code", code.0.into())]),
        E::Argument(kind) => record(
            K::Argument as u32,
            &[("argument", (ArgumentKind::from(kind) as u32).into())],
        ),
        E::OversizeArguments => record(K::OversizeArguments as u32, &[]),
    };
    property(&error, "reason", reason);
    error
}
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
quantity!(PinBank);
quantity!(ProfileIndex);
#[wasm_bindgen]
pub struct WordAddress {
    inner: core::WordAddress,
}
#[wasm_bindgen]
impl WordAddress {
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "number")] value: JsValue,
    ) -> Result<Self, JsValue> {
        core::WordAddress::new(usb::ff80::commands::memory::Address(integer(
            value,
            u32::MAX,
        )?))
        .map(|inner| Self { inner })
        .map_err(invalid)
    }
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> u32 {
        self.inner.address().0
    }
}
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum ProfileBank {
    First,
    Second,
}
impl ProfileBank {
    fn parse(value: JsValue) -> Result<core::ProfileBank, JsValue> {
        let value = integer(value, u32::MAX)?;
        if value == Self::First as u32 {
            Ok(core::ProfileBank::First)
        } else if value == Self::Second as u32 {
            Ok(core::ProfileBank::Second)
        } else {
            Err(fail(ErrorKind::InvalidArgument, "unknown profile bank"))
        }
    }
}
#[wasm_bindgen]
pub struct ProfileAction {
    inner: core::ProfileAction,
}
#[wasm_bindgen]
impl ProfileAction {
    pub fn read(
        #[wasm_bindgen(unchecked_param_type = "ProfileBank")] bank: JsValue,
        index: &ProfileIndex,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::ProfileAction::Read {
                bank: ProfileBank::parse(bank)?,
                index: index.inner,
            },
        })
    }
    pub fn write(
        #[wasm_bindgen(unchecked_param_type = "ProfileBank")] bank: JsValue,
        index: &ProfileIndex,
        #[wasm_bindgen(unchecked_param_type = "number")] value: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::ProfileAction::Write {
                bank: ProfileBank::parse(bank)?,
                index: index.inner,
                value: integer(value, u8::MAX.into())? as u8,
            },
        })
    }
    pub fn reset() -> Self {
        Self {
            inner: core::ProfileAction::Reset,
        }
    }
}
#[wasm_bindgen]
pub struct App80Command {
    inner: core::Command,
}
#[wasm_bindgen]
impl App80Command {
    #[wasm_bindgen(js_name=registerControl)]
    pub fn register_control(action: &peripheral::RegisterAction) -> Self {
        Self {
            inner: core::Command::RegisterControl(action.inner),
        }
    }
    pub fn i2c(action: &peripheral::I2cAction) -> Self {
        Self {
            inner: core::Command::I2c(action.inner),
        }
    }
    pub fn accumulator(
        #[wasm_bindgen(unchecked_param_type = "AccumulatorAction")] action: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::Command::Accumulator(peripheral::AccumulatorAction::parse(action)?),
        })
    }
    #[wasm_bindgen(js_name=peripheralReport)]
    pub fn peripheral_report() -> Self {
        Self {
            inner: core::Command::PeripheralReport,
        }
    }
    #[wasm_bindgen(js_name=writePins)]
    pub fn write_pins(
        bank: &PinBank,
        #[wasm_bindgen(unchecked_param_type = "number")] levels: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] exclude: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::Command::WritePins {
                bank: bank.inner,
                levels: integer(levels, u16::MAX.into())? as u16,
                exclude: integer(exclude, u16::MAX.into())? as u16,
            },
        })
    }
    #[wasm_bindgen(js_name=readPins)]
    pub fn read_pins(bank: &PinBank) -> Self {
        Self {
            inner: core::Command::ReadPins { bank: bank.inner },
        }
    }
    #[wasm_bindgen(js_name=readChannel)]
    pub fn read_channel(
        #[wasm_bindgen(unchecked_param_type = "number")] channel: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::Command::ReadChannel {
                channel: integer(channel, u8::MAX.into())? as u8,
            },
        })
    }
    #[wasm_bindgen(js_name=writeChannel)]
    pub fn write_channel(
        #[wasm_bindgen(unchecked_param_type = "number")] channel: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] value: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::Command::WriteChannel {
                channel: integer(channel, u8::MAX.into())? as u8,
                value: integer(value, u16::MAX.into())? as u16,
            },
        })
    }
    #[wasm_bindgen(js_name=profileByte)]
    pub fn profile_byte(action: &ProfileAction) -> Self {
        Self {
            inner: core::Command::ProfileByte(action.inner),
        }
    }
    #[wasm_bindgen(js_name=downloadStatus)]
    pub fn download_status() -> Self {
        Self {
            inner: core::Command::DownloadStatus,
        }
    }
    #[wasm_bindgen(js_name=copyConfiguration)]
    pub fn copy_configuration(
        #[wasm_bindgen(unchecked_param_type = "number")] page: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::Command::CopyConfiguration {
                page: integer(page, u8::MAX.into())? as u8,
            },
        })
    }
    #[wasm_bindgen(js_name=flushConfiguration)]
    pub fn flush_configuration() -> Self {
        Self {
            inner: core::Command::FlushConfiguration,
        }
    }
    #[wasm_bindgen(js_name=readWord)]
    pub fn read_word(address: &WordAddress) -> Self {
        Self {
            inner: core::Command::ReadWord {
                address: address.inner,
            },
        }
    }
    #[wasm_bindgen(js_name=writeWord)]
    pub fn write_word(
        address: &WordAddress,
        #[wasm_bindgen(unchecked_param_type = "number")] value: JsValue,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            inner: core::Command::WriteWord {
                address: address.inner,
                value: integer(value, u32::MAX)?,
            },
        })
    }
}
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum App80ReplyKind {
    NoPayload,
    Pins,
    Channel,
    ProfileByte,
    DownloadStatus,
    ConfigurationSaved,
    ReadWord,
    WriteWord,
    Research,
    I2cData,
    Accumulated,
    PeripheralReport,
}
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum SaveStatusKind {
    Success,
    Failed,
    Unknown,
}
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum WordReadKind {
    Value,
    Rejected,
    Unknown,
}
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum WordWriteKind {
    Confirmed,
    Rejected,
    Unknown,
}
#[wasm_bindgen]
pub struct App80CommandReply {
    pub(crate) inner: core::CommandReply,
}
#[wasm_bindgen]
impl App80CommandReply {
    #[wasm_bindgen(getter, unchecked_return_type = "Uint8Array")]
    pub fn raw(&self) -> JsValue {
        error::bytes(self.inner.raw().raw())
    }
    #[wasm_bindgen(getter, unchecked_return_type = "App80ReplyValue")]
    pub fn value(&self) -> JsValue {
        use App80ReplyKind as K;
        use core::ReplyValue as V;
        match self.inner.value() {
            V::NoPayload => record(K::NoPayload as u32, &[]),
            V::Pins(levels) => record(K::Pins as u32, &[("levels", (*levels).into())]),
            V::Channel(value) => record(K::Channel as u32, &[("value", (*value).into())]),
            V::ProfileByte(value) => record(K::ProfileByte as u32, &[("value", (*value).into())]),
            V::DownloadStatus { raw_status } => record(
                K::DownloadStatus as u32,
                &[("rawStatus", (*raw_status).into())],
            ),
            V::ConfigurationSaved(status) => {
                let status = match status {
                    core::SaveStatus::Success => record(SaveStatusKind::Success as u32, &[]),
                    core::SaveStatus::Failed => record(SaveStatusKind::Failed as u32, &[]),
                    core::SaveStatus::Unknown(raw) => record(
                        SaveStatusKind::Unknown as u32,
                        &[("rawStatus", (*raw).into())],
                    ),
                };
                record(K::ConfigurationSaved as u32, &[("status", status)])
            }
            V::ReadWord(result) => {
                let result = match result {
                    core::WordReadResult::Value(value) => {
                        record(WordReadKind::Value as u32, &[("value", (*value).into())])
                    }
                    core::WordReadResult::Rejected => record(WordReadKind::Rejected as u32, &[]),
                    core::WordReadResult::Unknown(raw) => record(
                        WordReadKind::Unknown as u32,
                        &[("rawStatus", (*raw).into())],
                    ),
                };
                record(K::ReadWord as u32, &[("result", result)])
            }
            V::WriteWord(result) => {
                let result = match result {
                    core::WordWriteResult::Confirmed => {
                        record(WordWriteKind::Confirmed as u32, &[])
                    }
                    core::WordWriteResult::Rejected => record(WordWriteKind::Rejected as u32, &[]),
                    core::WordWriteResult::Unknown(raw) => record(
                        WordWriteKind::Unknown as u32,
                        &[("rawStatus", (*raw).into())],
                    ),
                };
                record(K::WriteWord as u32, &[("result", result)])
            }
            V::Research => record(K::Research as u32, &[]),
            V::I2cData(data) => record(K::I2cData as u32, &[("data", error::bytes(data))]),
            V::Accumulated(values) => {
                let array: js_sys::Array =
                    values.iter().map(|&value| JsValue::from(value)).collect();
                record(
                    K::Accumulated as u32,
                    &[("values", js_sys::Object::freeze(&array).into())],
                )
            }
            V::PeripheralReport(data) => {
                record(K::PeripheralReport as u32, &[("data", error::bytes(data))])
            }
        }
    }
}
#[wasm_bindgen]
#[derive(Clone)]
pub struct App80Invocation {
    pub(crate) inner: usb::operations::app80::Invocation,
}
#[wasm_bindgen]
impl App80Invocation {
    #[wasm_bindgen(getter, unchecked_return_type = "Uint8Array")]
    pub fn raw(&self) -> JsValue {
        error::bytes(self.inner.command().request().encode())
    }
    #[wasm_bindgen(getter,js_name=configurationLimit)]
    pub fn configuration_limit(&self) -> u32 {
        self.inner.configuration_limit().get()
    }
    pub fn decode(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Uint8Array")] raw: JsValue,
    ) -> Result<App80CommandReply, JsValue> {
        usb::app80::Reply::decode(&error::buffer(raw)?)
            .and_then(|reply| self.inner.command().decode(reply))
            .map(|inner| App80CommandReply { inner })
            .map_err(error)
    }
}
#[wasm_bindgen]
pub struct App80Service {
    inner: usb::operations::app80::Service,
}
#[wasm_bindgen]
impl App80Service {
    #[wasm_bindgen(constructor)]
    pub fn new(definition: &ServiceDefinition) -> Result<Self, JsValue> {
        usb::operations::app80::Service::new(definition.inner.clone())
            .map(|inner| Self { inner })
            .map_err(error)
    }
    pub fn prepare(
        &self,
        modes: &ServiceModes,
        command: &App80Command,
    ) -> Result<App80Invocation, JsValue> {
        self.inner
            .prepare(modes.inner, command.inner)
            .map(|inner| App80Invocation { inner })
            .map_err(invalid)
    }
    #[wasm_bindgen(js_name=prepareResearch)]
    pub fn prepare_research(
        &self,
        code: &requests::App80CommandCode,
        modes: &ServiceModes,
        #[wasm_bindgen(unchecked_param_type = "Uint8Array")] arguments: JsValue,
    ) -> Result<App80Invocation, JsValue> {
        self.inner
            .prepare_research(
                usb::app80::CommandCode(code.value()),
                modes.inner,
                &error::buffer(arguments)?,
            )
            .map(|inner| App80Invocation { inner })
            .map_err(invalid)
    }
}
#[wasm_bindgen(typescript_custom_section)]
const TYPES: &str = r#"
export type CommandFailure =
  | Readonly<{ kind: CommandFailureKind.Unsupported | CommandFailureKind.Ambiguous; codec: ServiceCodec }>
  | Readonly<{ kind: CommandFailureKind.Unavailable; code: number }>
  | Readonly<{ kind: CommandFailureKind.Argument; argument: ArgumentKind }>
  | Readonly<{ kind: CommandFailureKind.OversizeArguments }>;
export type SaveStatus =
  | Readonly<{ kind: SaveStatusKind.Success | SaveStatusKind.Failed }>
  | Readonly<{ kind: SaveStatusKind.Unknown; rawStatus: number }>;
export type WordReadResult =
  | Readonly<{ kind: WordReadKind.Value; value: number }>
  | Readonly<{ kind: WordReadKind.Rejected }>
  | Readonly<{ kind: WordReadKind.Unknown; rawStatus: number }>;
export type WordWriteResult =
  | Readonly<{ kind: WordWriteKind.Confirmed | WordWriteKind.Rejected }>
  | Readonly<{ kind: WordWriteKind.Unknown; rawStatus: number }>;
export type App80ReplyValue =
  | Readonly<{ kind: App80ReplyKind.NoPayload | App80ReplyKind.Research }>
  | Readonly<{ kind: App80ReplyKind.Pins; levels: number }>
  | Readonly<{ kind: App80ReplyKind.Channel | App80ReplyKind.ProfileByte; value: number }>
  | Readonly<{ kind: App80ReplyKind.DownloadStatus; rawStatus: number }>
  | Readonly<{ kind: App80ReplyKind.ConfigurationSaved; status: SaveStatus }>
  | Readonly<{ kind: App80ReplyKind.ReadWord; result: WordReadResult }>
  | Readonly<{ kind: App80ReplyKind.WriteWord; result: WordWriteResult }>
  | Readonly<{ kind: App80ReplyKind.I2cData | App80ReplyKind.PeripheralReport; data: Uint8Array }>
  | Readonly<{ kind: App80ReplyKind.Accumulated; values: readonly [number, number, number] }>;
"#;

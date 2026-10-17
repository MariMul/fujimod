//! Firmware-selected App80 codecs. No USB carrier or model defaults.
use super::{CommandCode, Reply, Request};
use crate::{Result as UsbResult, error::ProtocolError, ff80::commands::memory::Address};
use fujimod_defs::service::{Codec, Definition, Entry, ServiceModes};
mod media;
pub use media::{
    AudioCount, AudioPlayback, AudioResult, AudioRoute, ChargeAction, ChargePresence, ChargeQuery,
    ChargeResult, DisplayMode, DisplayOptionAction, HdmiAction, HdmiPacket, HdmiValue,
    ReportedChargeState, SoundAction, SoundClip, SoundFormat, SoundOperation, SoundTrack,
    StorageErase, StorageEraseResult, StorageStatus, Toggle,
};
mod peripheral;
pub use peripheral::{
    AccumulatorAction, I2cAction, I2cLength, I2cPayload, I2cPort, RegisterAction,
};

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("the selected firmware/modes do not define {0:?}")]
    Unsupported(Codec),
    #[error("multiple command identifiers implement {0:?} in the selected modes")]
    Ambiguous(Codec),
    #[error("command {0:?} is not enabled in the selected service modes")]
    Unavailable(CommandCode),
    #[error("{0} is outside its supported argument range")]
    Argument(ArgumentKind),
    #[error("research arguments must fit the 32-byte App80 mailbox")]
    OversizeArguments,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentKind {
    PinBank,
    ProfileIndex,
    WordAlignment,
    WordAddress,
    I2cLength,
    SoundClip,
    HdmiValue,
    HdmiPacketLength,
    AudioCount,
}
impl std::fmt::Display for ArgumentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::PinBank => "pin bank",
            Self::ProfileIndex => "profile index",
            Self::WordAlignment => "word alignment",
            Self::WordAddress => "service word address",
            Self::I2cLength => "I2C payload length (1..=10 bytes)",
            Self::SoundClip => "sound clip index (0..=22)",
            Self::HdmiValue => "HDMI value (0..=4095)",
            Self::HdmiPacketLength => "HDMI packet length (16 bytes)",
            Self::AudioCount => "audio chunk count (0..=10)",
        })
    }
}
macro_rules! nibble_type {
    ($name:ident,$kind:ident) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $name(u8);
        impl $name {
            pub fn new(value: u8) -> Result<Self, CommandError> {
                if value < 16 {
                    Ok(Self(value))
                } else {
                    Err(CommandError::Argument(ArgumentKind::$kind))
                }
            }
            pub fn value(self) -> u8 {
                self.0
            }
        }
    };
}
nibble_type!(PinBank, PinBank);
nibble_type!(ProfileIndex, ProfileIndex);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileBank {
    First,
    Second,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileAction {
    Read {
        bank: ProfileBank,
        index: ProfileIndex,
    },
    Write {
        bank: ProfileBank,
        index: ProfileIndex,
        value: u8,
    },
    Reset,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WordAddress(Address);
impl WordAddress {
    pub fn new(address: Address) -> Result<Self, CommandError> {
        if !address.0.is_multiple_of(4) {
            return Err(CommandError::Argument(ArgumentKind::WordAlignment));
        }
        Ok(Self(address))
    }
    pub fn address(self) -> Address {
        self.0
    }
}
/// Selector choices are variants. Integers represent quantities, not commands
/// or completion instructions; command bytes come only from the definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    WritePins {
        bank: PinBank,
        levels: u16,
        exclude: u16,
    },
    ReadPins {
        bank: PinBank,
    },
    ReadChannel {
        channel: u8,
    },
    WriteChannel {
        channel: u8,
        value: u16,
    },
    ProfileByte(ProfileAction),
    DownloadStatus,
    CopyConfiguration {
        page: u8,
    },
    FlushConfiguration,
    ReadWord {
        address: WordAddress,
    },
    WriteWord {
        address: WordAddress,
        value: u32,
    },
    RegisterControl(RegisterAction),
    I2c(I2cAction),
    Accumulator(AccumulatorAction),
    PeripheralReport,
    DisplayOption(DisplayOptionAction),
    DisplayMode(DisplayMode),
    Sound(SoundAction),
    Hdmi(HdmiAction),
    ParameterService(Toggle),
    ChargeControl(ChargeAction),
    ChargeStatus(ChargeQuery),
    AudioCapture {
        count: AudioCount,
        route: AudioRoute,
    },
    AudioPlayback(AudioPlayback),
    EraseStorage(StorageErase),
    StorageStatus,
    FormatStorage,
    DisplaySetup,
}
impl Command {
    pub fn codec(self) -> Codec {
        match self {
            Self::WritePins { .. } => Codec::WritePins,
            Self::ReadPins { .. } => Codec::ReadPins,
            Self::ReadChannel { .. } => Codec::ReadChannel,
            Self::WriteChannel { .. } => Codec::WriteChannel,
            Self::ProfileByte(_) => Codec::ProfileByte,
            Self::DownloadStatus => Codec::DownloadStatus,
            Self::CopyConfiguration { .. } => Codec::CopyConfiguration,
            Self::FlushConfiguration => Codec::FlushConfiguration,
            Self::ReadWord { .. } | Self::WriteWord { .. } => Codec::MemoryWord,
            Self::RegisterControl(_) => Codec::RegisterControl,
            Self::I2c(_) => Codec::I2c,
            Self::Accumulator(_) => Codec::Accumulator,
            Self::PeripheralReport => Codec::PeripheralReport,
            Self::DisplayOption(_) => Codec::DisplayOption,
            Self::DisplayMode(_) => Codec::DisplayMode,
            Self::Sound(_) => Codec::Sound,
            Self::Hdmi(_) => Codec::Hdmi,
            Self::ParameterService(_) => Codec::ParameterService,
            Self::ChargeControl(_) => Codec::ChargeControl,
            Self::ChargeStatus(_) => Codec::ChargeStatus,
            Self::AudioCapture { .. } => Codec::AudioCapture,
            Self::AudioPlayback(_) => Codec::AudioPlayback,
            Self::EraseStorage(_) => Codec::EraseStorage,
            Self::StorageStatus => Codec::StorageStatus,
            Self::FormatStorage => Codec::FormatStorage,
            Self::DisplaySetup => Codec::DisplaySetup,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveStatus {
    Success,
    Failed,
    Unknown(u8),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WordReadResult {
    Value(u32),
    Rejected,
    Unknown(u8),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WordWriteResult {
    Confirmed,
    Rejected,
    Unknown(u8),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplyValue {
    NoPayload,
    Pins(u16),
    Channel(u16),
    ProfileByte(u8),
    DownloadStatus { raw_status: u8 },
    ConfigurationSaved(SaveStatus),
    ReadWord(WordReadResult),
    WriteWord(WordWriteResult),
    Research,
    I2cData(Vec<u8>),
    Accumulated([u16; 3]),
    PeripheralReport([u8; 8]),
    ChargeApplied(ChargeResult),
    ChargePresence(ChargePresence),
    ChargeState(ReportedChargeState),
    AudioCaptured(AudioResult),
    AudioPlayed(AudioResult),
    StorageErased(StorageEraseResult),
    StorageStatus(StorageStatus),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandReply {
    value: ReplyValue,
    raw: Reply,
}
impl CommandReply {
    pub fn value(&self) -> &ReplyValue {
        &self.value
    }
    pub fn raw(&self) -> &Reply {
        &self.raw
    }
}
#[derive(Clone, Debug)]
enum Decode {
    Typed(Command),
    Research,
}
#[derive(Clone, Debug)]
pub struct PreparedCommand {
    request: Request,
    slots: Vec<u8>,
    decode: Decode,
}
impl PreparedCommand {
    pub fn request(&self) -> &Request {
        &self.request
    }
    pub fn decode(&self, reply: Reply) -> UsbResult<CommandReply> {
        let Decode::Typed(command) = self.decode else {
            return Ok(CommandReply {
                value: ReplyValue::Research,
                raw: reply,
            });
        };
        // These reviewed codecs preserve the published command byte. Research
        // replies remain opaque, including firmware that changes that byte.
        if reply.raw()[0] != self.request.code().0 {
            return Err(ProtocolError::CommandEcho {
                expected: self.request.code().0.into(),
                actual: reply.raw()[0].into(),
            }
            .into());
        }
        let byte = |index: usize| reply.raw()[1 + usize::from(self.slots[index])];
        let half = |a, b| u16::from_be_bytes([byte(a), byte(b)]);
        let word = || u32::from_be_bytes([byte(6), byte(7), byte(8), byte(9)]);
        let value = match command {
            Command::ReadPins { .. } => ReplyValue::Pins(half(1, 2)),
            Command::ReadChannel { .. } => ReplyValue::Channel(half(1, 2)),
            Command::ProfileByte(ProfileAction::Read { .. }) => ReplyValue::ProfileByte(byte(2)),
            Command::DownloadStatus => ReplyValue::DownloadStatus {
                raw_status: byte(0),
            },
            Command::FlushConfiguration => ReplyValue::ConfigurationSaved(match byte(0) {
                0 => SaveStatus::Success,
                1 => SaveStatus::Failed,
                n => SaveStatus::Unknown(n),
            }),
            Command::ReadWord { .. } => ReplyValue::ReadWord(match byte(1) {
                1 => WordReadResult::Value(word()),
                0 => WordReadResult::Rejected,
                n => WordReadResult::Unknown(n),
            }),
            Command::WriteWord { .. } => ReplyValue::WriteWord(match byte(1) {
                1 => WordWriteResult::Confirmed,
                0 => WordWriteResult::Rejected,
                n => WordWriteResult::Unknown(n),
            }),
            Command::I2c(I2cAction::Read { length, .. }) => ReplyValue::I2cData(
                (0..usize::from(length.value()))
                    .map(|index| byte(4 + index))
                    .collect(),
            ),
            Command::Accumulator(AccumulatorAction::StopAndRead) => {
                ReplyValue::Accumulated([half(1, 2), half(3, 4), half(5, 6)])
            }
            Command::PeripheralReport => ReplyValue::PeripheralReport(std::array::from_fn(byte)),
            Command::ChargeControl(_) => ReplyValue::ChargeApplied(ChargeResult::decode(byte(2))),
            Command::ChargeStatus(ChargeQuery::Presence) => {
                ReplyValue::ChargePresence(ChargePresence::decode(byte(1)))
            }
            Command::ChargeStatus(ChargeQuery::State) => {
                ReplyValue::ChargeState(ReportedChargeState::decode(byte(1)))
            }
            Command::AudioCapture { .. } => ReplyValue::AudioCaptured(AudioResult::decode(byte(2))),
            Command::AudioPlayback(_) => ReplyValue::AudioPlayed(AudioResult::decode(byte(1))),
            Command::EraseStorage(_) => {
                ReplyValue::StorageErased(StorageEraseResult::decode(byte(1)))
            }
            Command::StorageStatus => ReplyValue::StorageStatus(StorageStatus::decode(byte(0))),
            Command::FormatStorage | Command::DisplaySetup => ReplyValue::NoPayload,
            Command::WritePins { .. }
            | Command::WriteChannel { .. }
            | Command::ProfileByte(ProfileAction::Write { .. } | ProfileAction::Reset)
            | Command::CopyConfiguration { .. }
            | Command::RegisterControl(
                RegisterAction::AudioValue { .. }
                | RegisterAction::DisplayFirst { .. }
                | RegisterAction::DisplayStub { .. }
                | RegisterAction::DisplaySecond { .. }
                | RegisterAction::PowerWrite { .. }
                | RegisterAction::PositionPair { .. }
                | RegisterAction::NoopFirst
                | RegisterAction::NoopSecond,
            )
            | Command::I2c(I2cAction::Write { .. } | I2cAction::WriteSingle { .. })
            | Command::Accumulator(AccumulatorAction::Start)
            | Command::DisplayOption(
                DisplayOptionAction::IdlePins | DisplayOptionAction::ForwardPair { .. },
            )
            | Command::DisplayMode(DisplayMode::Option0 | DisplayMode::Option1)
            | Command::Sound(
                SoundAction::Clip { .. } | SoundAction::Track { .. } | SoundAction::Noop,
            )
            | Command::Hdmi(
                HdmiAction::ResetStub | HdmiAction::Packet(_) | HdmiAction::Triplet { .. },
            )
            | Command::ParameterService(Toggle::Disabled | Toggle::Enabled) => {
                ReplyValue::NoPayload
            }
        };
        Ok(CommandReply { value, raw: reply })
    }
}
#[derive(Clone, Debug)]
pub struct Catalogue {
    definition: Definition,
}
impl Catalogue {
    pub fn new(definition: Definition) -> Self {
        Self { definition }
    }
    pub fn definition(&self) -> &Definition {
        &self.definition
    }
    pub fn prepare(
        &self,
        modes: ServiceModes,
        command: Command,
    ) -> Result<PreparedCommand, CommandError> {
        let codec = command.codec();
        // Resolve the same first matching row as firmware's dispatch. A later
        // duplicate with a different codec must never override the active row.
        let mut matches = self.definition.entries().iter().filter(|entry| {
            entry.codec() == codec
                && self
                    .definition
                    .select(entry.code(), modes)
                    .is_some_and(|selected| std::ptr::eq(selected, *entry))
        });
        let entry = matches.next().ok_or(CommandError::Unsupported(codec))?;
        if matches.next().is_some() {
            return Err(CommandError::Ambiguous(codec));
        }
        Self::encode(entry, command)
    }
    pub fn prepare_research(
        &self,
        code: CommandCode,
        modes: ServiceModes,
        arguments: &[u8],
    ) -> Result<PreparedCommand, CommandError> {
        self.definition
            .select(code.0, modes)
            .ok_or(CommandError::Unavailable(code))?;
        let request = Request::new(code, arguments).map_err(|_| CommandError::OversizeArguments)?;
        Ok(PreparedCommand {
            request,
            slots: vec![],
            decode: Decode::Research,
        })
    }
    fn encode(entry: &Entry, command: Command) -> Result<PreparedCommand, CommandError> {
        let mut arguments = [0; 32];
        let mut put =
            |index: usize, value: u8| arguments[usize::from(entry.slots()[index])] = value;
        match command {
            Command::WritePins {
                bank,
                levels,
                exclude,
            } => {
                let [hi, lo] = levels.to_be_bytes();
                let [ex_hi, ex_lo] = exclude.to_be_bytes();
                for (i, v) in [bank.value(), hi, lo, ex_hi, ex_lo].into_iter().enumerate() {
                    put(i, v);
                }
            }
            Command::ReadPins { bank } => put(0, bank.value()),
            Command::ReadChannel { channel } => put(0, channel),
            Command::WriteChannel { channel, value } => {
                let [hi, lo] = value.to_be_bytes();
                for (i, v) in [channel, hi, lo].into_iter().enumerate() {
                    put(i, v);
                }
            }
            Command::ProfileByte(action) => match action {
                ProfileAction::Read { bank, index } => {
                    put(
                        0,
                        match bank {
                            ProfileBank::First => 1,
                            ProfileBank::Second => 2,
                        },
                    );
                    put(1, index.value());
                }
                ProfileAction::Write { bank, index, value } => {
                    put(
                        0,
                        match bank {
                            ProfileBank::First => 3,
                            ProfileBank::Second => 4,
                        },
                    );
                    put(1, index.value());
                    put(2, value);
                }
                ProfileAction::Reset => put(0, 5),
            },
            Command::CopyConfiguration { page } => put(0, page),
            // The handler writes 0 or 1. Start with an unknown sentinel so
            // an unchanged output can never be reported as a successful save.
            Command::FlushConfiguration => put(0, u8::MAX),
            Command::DownloadStatus => (),
            Command::RegisterControl(action) => action.encode(&mut put),
            Command::I2c(action) => action.encode(&mut put),
            Command::Accumulator(action) => action.encode(&mut put),
            Command::PeripheralReport => (),
            Command::DisplayOption(action) => action.encode(&mut put),
            Command::DisplayMode(mode) => put(0, mode.wire()),
            Command::Sound(action) => action.encode(&mut put),
            Command::Hdmi(action) => action.encode(&mut put),
            Command::ParameterService(value) => put(0, value.wire()),
            Command::ChargeControl(action) => action.encode(&mut put),
            Command::ChargeStatus(query) => query.encode(&mut put),
            Command::AudioCapture { count, route } => {
                put(0, count.value());
                put(1, route.wire());
                put(2, 255);
            }
            Command::AudioPlayback(mode) => {
                put(0, mode.wire());
                put(1, 255);
            }
            Command::EraseStorage(action) => {
                put(0, action.wire());
                put(1, 255);
            }
            Command::StorageStatus => put(0, 255),
            Command::FormatStorage | Command::DisplaySetup => (),
            Command::ReadWord { address } | Command::WriteWord { address, .. } => {
                let value = address.address().0;
                if value > 0x1fff_ffff && value.wrapping_add(0xc000_0000) > 0x1fff_ffff {
                    return Err(CommandError::Argument(ArgumentKind::WordAddress));
                }
                let write = matches!(command, Command::WriteWord { .. });
                put(0, u8::from(write));
                for (i, v) in value.to_be_bytes().into_iter().enumerate() {
                    put(i + 2, v);
                }
                if let Command::WriteWord { value, .. } = command {
                    for (i, v) in value.to_be_bytes().into_iter().enumerate() {
                        put(i + 6, v);
                    }
                }
            }
        }
        Ok(PreparedCommand {
            request: Request::new(CommandCode(entry.code()), &arguments)
                .expect("fixed 32-byte argument buffer"),
            slots: entry.slots().to_vec(),
            decode: Decode::Typed(command),
        })
    }
}

#[cfg(test)]
mod peripheral_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod media_tests;

//! Single radio/network service requests. Stored credentials, connection
//! workflows and result polling are separate operations.
use super::{ArgumentKind, CommandError, Toggle};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadioMode {
    Mode0,
    Mode1,
}
impl RadioMode {
    fn wire(self) -> u8 {
        match self {
            Self::Mode0 => 0,
            Self::Mode1 => 1,
        }
    }
}
/// Firmware channel field (0..=14); no regulatory or connected-device policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RadioChannel(u8);
impl RadioChannel {
    pub fn new(value: u8) -> Result<Self, CommandError> {
        if value <= 14 {
            Ok(Self(value))
        } else {
            Err(CommandError::Argument(ArgumentKind::RadioChannel))
        }
    }
    pub fn value(self) -> u8 {
        self.0
    }
}
/// Seven-bit encoded power field. This is not a dBm value: firmware transforms
/// values above 11 differently in mode 1, and preserves the raw byte in mode 0.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RadioPower(u8);
impl RadioPower {
    pub fn new(value: u8) -> Result<Self, CommandError> {
        if value < 128 {
            Ok(Self(value))
        } else {
            Err(CommandError::Argument(ArgumentKind::RadioPower))
        }
    }
    pub fn value(self) -> u8 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadioAction {
    Primary {
        mode: RadioMode,
        channel: RadioChannel,
        power: RadioPower,
        option: Toggle,
    },
    Secondary {
        mode: RadioMode,
        channel: RadioChannel,
    },
}
impl RadioAction {
    pub(super) fn encode(self, put: &mut impl FnMut(usize, u8)) {
        match self {
            Self::Primary {
                mode,
                channel,
                power,
                option,
            } => {
                put(0, mode.wire());
                put(1, channel.value());
                put(2, power.value() | (option.wire() << 7));
                put(3, 0);
            }
            Self::Secondary { mode, channel } => {
                put(0, mode.wire());
                put(1, channel.value());
                put(3, 1);
            }
        }
    }
}
/// An exact six-byte peer address, not text interpreted or normalized by a host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerAddress([u8; 6]);
impl PeerAddress {
    pub fn new(bytes: &[u8]) -> Result<Self, CommandError> {
        bytes
            .try_into()
            .map(Self)
            .map_err(|_| CommandError::Argument(ArgumentKind::PeerAddressLength))
    }
    pub fn data(&self) -> &[u8; 6] {
        &self.0
    }
}
/// The command returns whether firmware option 0x41 equals zero. The inverse
/// bit is decoded here without inferring a physical drive's presence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Option41State {
    Zero,
    NonZero,
    Unknown(u8),
}
impl Option41State {
    pub(super) fn decode(raw: u8) -> Self {
        match raw {
            1 => Self::Zero,
            0 => Self::NonZero,
            n => Self::Unknown(n),
        }
    }
}

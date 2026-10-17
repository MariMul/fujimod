//! Direct firmware configuration requests. Read requests can first change flags;
//! returned bytes are raw snapshots, not verified settings or memory values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigurationBank {
    Bank0,
    Bank1,
}
impl ConfigurationBank {
    pub(super) fn wire(self) -> u8 {
        match self {
            Self::Bank0 => 0,
            Self::Bank1 => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigurationMode {
    Mode0,
    Mode1,
    Mode2,
    Mode3,
}
impl ConfigurationMode {
    pub(super) fn wire(self) -> u8 {
        match self {
            Self::Mode0 => 0,
            Self::Mode1 => 1,
            Self::Mode2 => 2,
            Self::Mode3 => 3,
        }
    }
}

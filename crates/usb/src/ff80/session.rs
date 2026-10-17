use super::{
    commands::{
        info::{DeviceInfo, InfoSelector},
        memory::{ReadMemory, WriteMemory},
    },
    framing::{RequestKind, envelope},
};
use crate::{
    Error, Result,
    connection::{Io, UsbIo},
    error::{ProtocolError, check_limit},
};
use std::num::NonZeroU32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    Ready,
    /// A transfer failed, its reply was malformed, or its future was dropped.
    /// The device may still be executing and this session cannot issue commands.
    CompletionUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionInfo {
    pub id: u16,
    pub advertised_packet_bytes: NonZeroU32,
    pub raw_open_reply: [u8; 8],
}

/// Exclusive owner of a connection's command/reply exchanges.
///
/// All operations borrow mutably. Cancellation or a malformed response leaves
/// this session unusable; dropping it releases host resources without requesting
/// a camera reset, firmware cleanup or service-mode transition.
pub struct Session {
    core: Core<UsbIo>,
}
impl Session {
    pub(crate) async fn open(io: UsbIo, max_reply_bytes: u32) -> Result<Self> {
        Ok(Self {
            core: Core::open(io, max_reply_bytes).await?,
        })
    }
    pub fn info(&self) -> SessionInfo {
        self.core.info
    }
    pub fn state(&self) -> SessionState {
        self.core.state
    }
    pub fn max_reply_bytes(&self) -> NonZeroU32 {
        NonZeroU32::new(self.core.max_reply_bytes).expect("validated before session open")
    }
    /// Returns the two raw ping reply bytes; their meaning is not inferred.
    pub async fn ping(&mut self) -> Result<[u8; 2]> {
        self.core.ping().await
    }
    pub async fn device_info(&mut self, selector: InfoSelector) -> Result<DeviceInfo> {
        self.core.device_info(selector).await
    }
    pub async fn read_memory(&mut self, command: ReadMemory) -> Result<Vec<u8>> {
        self.core.read_memory(command).await
    }
    pub async fn write_memory(&mut self, command: WriteMemory) -> Result<()> {
        self.core.write_memory(command).await
    }
    /// Sends C4 once. This consumes the session even if the close fails.
    pub async fn close(mut self) -> Result<[u8; 2]> {
        self.core.close().await
    }
}

pub(crate) struct Core<I> {
    io: I,
    info: SessionInfo,
    state: SessionState,
    max_reply_bytes: u32,
}

impl<I: Io> Core<I> {
    async fn open(mut io: I, max_reply_bytes: u32) -> Result<Self> {
        check_limit(8, max_reply_bytes)?;
        io.control(RequestKind::Open, envelope(RequestKind::Open, 8, 0))
            .await?;
        let bytes: [u8; 8] = exact(io.read(8).await?)?;
        let packet = NonZeroU32::new(u32::from_be_bytes(bytes[4..8].try_into().unwrap()))
            .ok_or(ProtocolError::ZeroPacketLimit)?;
        Ok(Self {
            io,
            info: SessionInfo {
                id: u16::from_le_bytes([bytes[0], bytes[1]]),
                advertised_packet_bytes: packet,
                raw_open_reply: bytes,
            },
            state: SessionState::Ready,
            max_reply_bytes,
        })
    }
    fn start(&mut self) -> Result<()> {
        if self.state != SessionState::Ready {
            return Err(Error::CompletionUnknown);
        }
        // Set before the first await; dropping any in-flight future preserves it.
        self.state = SessionState::CompletionUnknown;
        Ok(())
    }
    async fn lifecycle(&mut self, kind: RequestKind) -> Result<[u8; 2]> {
        self.start()?;
        self.io
            .control(kind, envelope(kind, 2, self.info.id))
            .await?;
        let reply = exact(self.io.read(2).await?)?;
        self.state = SessionState::Ready;
        Ok(reply)
    }
    async fn ping(&mut self) -> Result<[u8; 2]> {
        self.lifecycle(RequestKind::Ping).await
    }
    async fn close(&mut self) -> Result<[u8; 2]> {
        self.lifecycle(RequestKind::Close).await
    }

    async fn command<T>(
        &mut self,
        bytes: Vec<u8>,
        reply_size: u32,
        decode: impl FnOnce(Vec<u8>) -> Result<T>,
    ) -> Result<T> {
        check_limit(reply_size, self.max_reply_bytes)?;
        let size = u32::try_from(bytes.len()).map_err(|_| Error::InvalidLength)?;
        self.start()?;
        self.io
            .control(
                RequestKind::WriteCommand,
                envelope(RequestKind::WriteCommand, size, self.info.id),
            )
            .await?;
        self.io.write(&bytes).await?;
        self.io
            .control(
                RequestKind::ReadCommand,
                envelope(RequestKind::ReadCommand, reply_size, self.info.id),
            )
            .await?;
        let response = self.io.read(reply_size as usize).await?;
        if response.len() > reply_size as usize {
            return Err(ProtocolError::ReplyLength {
                expected: reply_size as usize,
                actual: response.len(),
            }
            .into());
        }
        let result = decode(response);
        // A framed device error is a completed reply, not an async disposition.
        if result.is_ok() || matches!(result, Err(Error::DeviceStatus { .. })) {
            self.state = SessionState::Ready;
        }
        result
    }
    async fn device_info(&mut self, selector: InfoSelector) -> Result<DeviceInfo> {
        self.command(selector.encode(), 128, |r| selector.decode(r))
            .await
    }
    async fn read_memory(&mut self, command: ReadMemory) -> Result<Vec<u8>> {
        let size = command.range().length().get();
        check_limit(size, self.info.advertised_packet_bytes.get())?;
        let reply_size = size.checked_add(36).ok_or(Error::InvalidLength)?;
        self.command(command.encode(), reply_size, |r| command.decode(r))
            .await
    }
    async fn write_memory(&mut self, command: WriteMemory) -> Result<()> {
        check_limit(
            command.range().length().get(),
            self.info.advertised_packet_bytes.get(),
        )?;
        self.command(command.encode(), 36, |r| command.decode(r))
            .await
    }
}

fn exact<const N: usize>(data: Vec<u8>) -> Result<[u8; N]> {
    let actual = data.len();
    data.try_into().map_err(|_| {
        ProtocolError::ReplyLength {
            expected: N,
            actual,
        }
        .into()
    })
}

#[cfg(test)]
mod tests;

mod commands;

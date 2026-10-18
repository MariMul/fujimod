//! Adapt remote USB completions to the existing ff80 engine, without framing in
//! the proxy crate. Native and Wasm controllers use this identical adapter.
use crate::{Error, Result, connection::Io, error::TransferStage, ff80::framing::RequestKind};
use fujimod_usb_proxy::{Client, Operation, Setup};
use std::time::Duration;

pub(crate) struct ProxyIo {
    pub client: Client,
    pub device: String,
    pub input: u8,
    pub output: u8,
    pub packet_size: usize,
    pub timeout: Duration,
}
impl ProxyIo {
    pub(crate) async fn notify_owner(
        &mut self,
        frame: fujimod_usb_proxy::protocol::Frame,
    ) -> Result<()> {
        Ok(self.client.notify_owner(frame).await?)
    }
    async fn exchange(
        &mut self,
        operation: Operation,
        stage: TransferStage,
    ) -> Result<(Vec<u8>, usize)> {
        let completion = self
            .client
            .request(&self.device, operation, self.timeout)
            .await?;
        let data = completion.data;
        if data.len() > fujimod_usb_proxy::protocol::MAX_TRANSFER {
            return Err(fujimod_usb_proxy::Error("oversized USB reply".into()).into());
        }
        if let Some(error) = completion.error {
            return Err(Error::Transfer {
                stage,
                source: match error.as_str() {
                    "stall" => nusb::transfer::TransferError::Stall,
                    "NotFoundError" => nusb::transfer::TransferError::Disconnected,
                    "babble" => nusb::transfer::TransferError::Fault,
                    _ => nusb::transfer::TransferError::Unknown(0),
                },
                transferred: Some(completion.transferred),
                partial: data,
            });
        }
        Ok((data, completion.transferred))
    }
}
impl Io for ProxyIo {
    async fn release(&mut self) -> Result<()> {
        let completion = self
            .client
            .request(&self.device, Operation::Close, self.timeout)
            .await?;
        if let Some(error) = completion.error {
            return Err(fujimod_usb_proxy::Error(error).into());
        }
        Ok(())
    }
    fn endpoint(&self, stage: TransferStage) -> Option<u8> {
        Some(match stage {
            TransferStage::Control(_) => 0,
            TransferStage::BulkIn => self.input,
            TransferStage::BulkOut => self.output,
        })
    }
    async fn control(&mut self, kind: RequestKind, envelope: [u8; 12]) -> Result<()> {
        let (_, transferred) = self
            .exchange(
                Operation::ControlOut {
                    setup: Setup {
                        request_type: "vendor".into(),
                        recipient: "other".into(),
                        request: 0,
                        value: 0,
                        index: 0,
                    },
                    data: envelope.to_vec(),
                },
                TransferStage::Control(kind),
            )
            .await?;
        if transferred != envelope.len() {
            return Err(Error::ShortTransfer {
                stage: TransferStage::Control(kind),
                expected: envelope.len(),
                actual: transferred,
            });
        }
        Ok(())
    }
    async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        let (_, transferred) = self
            .exchange(
                Operation::TransferOut {
                    endpoint: self.output,
                    data: bytes.to_vec(),
                },
                TransferStage::BulkOut,
            )
            .await?;
        if transferred != bytes.len() {
            return Err(Error::ShortTransfer {
                stage: TransferStage::BulkOut,
                expected: bytes.len(),
                actual: transferred,
            });
        }
        Ok(())
    }
    async fn read(&mut self, maximum: usize) -> Result<Vec<u8>> {
        let length = maximum
            .div_ceil(self.packet_size)
            .checked_mul(self.packet_size)
            .ok_or(Error::InvalidLength)?;
        let (data, transferred) = self
            .exchange(
                Operation::TransferIn {
                    endpoint: self.input,
                    length,
                },
                TransferStage::BulkIn,
            )
            .await?;
        if transferred != data.len() || data.len() > length {
            return Err(
                fujimod_usb_proxy::Error("inconsistent USB completion length".into()).into(),
            );
        }
        Ok(data)
    }
}

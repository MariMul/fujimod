//! Adapt remote USB completions to the existing ff80 engine, without framing in
//! the proxy crate. Native and Wasm controllers use this identical adapter.
//! Steps stream to the exporter as the engine issues them; only a read, which
//! the engine needs before it can continue, waits for results.
use crate::{Error, Result, connection::Io, error::TransferStage, ff80::framing::RequestKind};
use fujimod_usb_proxy::{Client, Message, Operation, Outcome, Recipient, RequestType, Setup};
use std::time::Duration;

pub(crate) struct ProxyIo {
    pub client: Client,
    pub device: String,
    pub input: u8,
    pub output: u8,
    pub packet_size: usize,
    pub timeout: Duration,
    /// Steps sent since the last sync, in order: sequence, stage, and the OUT
    /// length each must transfer (None for IN).
    pub unsynced: Vec<(u64, TransferStage, Option<usize>)>,
}
fn vendor_setup() -> Setup {
    Setup {
        request_type: RequestType::Vendor,
        recipient: Recipient::Other,
        request: 0,
        value: 0,
        index: 0,
    }
}
/// Map an exporter's failure text to the transfer error the engine reports.
fn transfer_error(stage: TransferStage, error: &str, transferred: usize, partial: Vec<u8>) -> Error {
    Error::Transfer {
        stage,
        source: match error {
            "stall" => nusb::transfer::TransferError::Stall,
            "NotFoundError" => nusb::transfer::TransferError::Disconnected,
            "babble" => nusb::transfer::TransferError::Fault,
            _ => nusb::transfer::TransferError::Unknown(0),
        },
        transferred: Some(transferred),
        partial,
    }
}
impl ProxyIo {
    pub(crate) async fn notify_owner(&mut self, message: Message) -> Result<()> {
        Ok(self.client.notify_owner(message).await?)
    }
    pub(crate) async fn owner_message(&mut self, operation: Operation) -> Result<()> {
        self.sync().await?;
        let completion = self.client.request(&self.device, operation, Duration::from_secs(30)).await?;
        if let Some(error) = completion.error {
            return Err(fujimod_usb_proxy::Error(error).into());
        }
        Ok(())
    }
    /// Send one USB step now; its result is checked at the next sync.
    async fn step(&mut self, operation: Operation, stage: TransferStage) -> Result<()> {
        let expected = match &operation {
            Operation::ControlOut { data, .. } | Operation::TransferOut { data, .. } => Some(data.len()),
            _ => None,
        };
        let seq = self.client.submit(&self.device, operation, self.timeout).await?;
        self.unsynced.push((seq, stage, expected));
        Ok(())
    }
    /// Wait for every step sent; returns each IN step's bytes, in order. OUT
    /// steps must transfer their whole payload. The first failing step is
    /// reported with its own stage; the exporter ran nothing after it.
    async fn sync(&mut self) -> Result<Vec<Vec<u8>>> {
        let steps = std::mem::take(&mut self.unsynced);
        if steps.is_empty() {
            return Ok(Vec::new());
        }
        let outcomes = self.client.sync().await?;
        if outcomes.len() != steps.len() {
            return Err(fujimod_usb_proxy::Error("inconsistent pipeline outcome".into()).into());
        }
        let mut replies = Vec::new();
        for ((seq, stage, expected), (done, outcome)) in steps.into_iter().zip(outcomes) {
            let Outcome::Completed(completion) = outcome else {
                return Err(fujimod_usb_proxy::Error("USB step not run".into()).into());
            };
            if seq != done {
                return Err(fujimod_usb_proxy::Error("inconsistent pipeline outcome".into()).into());
            }
            if let Some(error) = &completion.error {
                return Err(transfer_error(stage, error, completion.transferred, completion.data));
            }
            match expected {
                Some(length) if completion.transferred != length => {
                    return Err(Error::ShortTransfer { stage, expected: length, actual: completion.transferred });
                }
                Some(_) => (),
                None => replies.push(completion.data),
            }
        }
        Ok(replies)
    }
    /// Several framed commands streamed with one wait: each is control(write
    /// envelope), its bytes, control(read envelope), then its reply.
    pub(crate) async fn commands(
        &mut self,
        commands: Vec<([u8; 12], Vec<u8>, [u8; 12], usize)>,
    ) -> Result<Vec<Vec<u8>>> {
        let earlier = self.unsynced.len();
        for (write, bytes, read, reply) in commands {
            let length = self.read_length(reply)?;
            self.step(Operation::ControlOut { setup: vendor_setup(), data: write.to_vec() },
                      TransferStage::Control(RequestKind::WriteCommand)).await?;
            self.step(Operation::TransferOut { endpoint: self.output, data: bytes }, TransferStage::BulkOut).await?;
            self.step(Operation::ControlOut { setup: vendor_setup(), data: read.to_vec() },
                      TransferStage::Control(RequestKind::ReadCommand)).await?;
            self.step(Operation::TransferIn { endpoint: self.input, length }, TransferStage::BulkIn).await?;
        }
        let earlier_replies = self.unsynced[..earlier].iter().filter(|s| s.2.is_none()).count();
        let mut replies = self.sync().await?;
        Ok(replies.split_off(earlier_replies))
    }
    fn read_length(&self, maximum: usize) -> Result<usize> {
        maximum
            .div_ceil(self.packet_size)
            .checked_mul(self.packet_size)
            .ok_or(Error::InvalidLength)
    }
}
impl Io for ProxyIo {
    async fn release(&mut self) -> Result<()> {
        self.unsynced.clear();
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
        self.step(Operation::ControlOut { setup: vendor_setup(), data: envelope.to_vec() }, TransferStage::Control(kind))
            .await
    }
    async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.step(Operation::TransferOut { endpoint: self.output, data: bytes.to_vec() }, TransferStage::BulkOut)
            .await
    }
    async fn read(&mut self, maximum: usize) -> Result<Vec<u8>> {
        let length = self.read_length(maximum)?;
        self.step(Operation::TransferIn { endpoint: self.input, length }, TransferStage::BulkIn).await?;
        let data = self.sync().await?.pop().ok_or_else(|| fujimod_usb_proxy::Error("missing IN reply".into()))?;
        if data.len() > length {
            return Err(fujimod_usb_proxy::Error("inconsistent USB completion length".into()).into());
        }
        Ok(data)
    }
}

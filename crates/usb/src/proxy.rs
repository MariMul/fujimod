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
    /// OUT steps not yet sent: they travel with the next IN as one batch, so a
    /// command (control, data, control, reply) is one network round trip.
    pub pending: Vec<(Operation, TransferStage)>,
}
fn vendor_setup() -> Setup {
    Setup {
        request_type: "vendor".into(),
        recipient: "other".into(),
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
    pub(crate) async fn notify_owner(&mut self, frame: fujimod_usb_proxy::protocol::Frame) -> Result<()> {
        Ok(self.client.notify_owner(frame).await?)
    }
    pub(crate) async fn owner_message(&mut self, operation: Operation) -> Result<()> {
        let completion = self.client.request(&self.device, operation, Duration::from_secs(30)).await?;
        if let Some(error) = completion.error {
            return Err(fujimod_usb_proxy::Error(error).into());
        }
        Ok(())
    }
    /// Run `steps` in one round trip; returns each IN step's bytes, in order.
    /// OUT steps must transfer their whole payload. The first failing step is
    /// reported with its own stage; the exporter runs nothing after it.
    pub(crate) async fn batch(&mut self, steps: Vec<(Operation, TransferStage)>) -> Result<Vec<Vec<u8>>> {
        let expected: Vec<Option<usize>> = steps
            .iter()
            .map(|(step, _)| match step {
                Operation::ControlOut { data, .. } | Operation::TransferOut { data, .. } => Some(data.len()),
                _ => None,
            })
            .collect();
        let stages: Vec<TransferStage> = steps.iter().map(|(_, stage)| *stage).collect();
        let count = steps.len() as u32;
        let operation = Operation::batch(steps.into_iter().map(|(step, _)| step).collect());
        // Each step keeps its own deadline budget at the exporter's pace.
        let timeout = (self.timeout + Duration::from_millis(20) * count).min(Duration::from_secs(120));
        let completion = self.client.request(&self.device, operation, timeout).await?;
        if completion.steps.len() > stages.len() || (completion.error.is_none() && completion.steps.len() != stages.len()) {
            return Err(fujimod_usb_proxy::Error("inconsistent batch completion".into()).into());
        }
        let mut data = completion.data.as_slice();
        let mut replies = Vec::new();
        for (index, step) in completion.steps.iter().enumerate() {
            if step.length > data.len() {
                return Err(fujimod_usb_proxy::Error("inconsistent batch completion".into()).into());
            }
            let (bytes, rest) = data.split_at(step.length);
            data = rest;
            let stage = stages[index];
            if let Some(error) = &step.error {
                return Err(transfer_error(stage, error, step.transferred, bytes.to_vec()));
            }
            match expected[index] {
                Some(length) if step.transferred != length => {
                    return Err(Error::ShortTransfer { stage, expected: length, actual: step.transferred });
                }
                Some(_) => (),
                None => replies.push(bytes.to_vec()),
            }
        }
        if let Some(error) = completion.error {
            return Err(fujimod_usb_proxy::Error(error).into());
        }
        Ok(replies)
    }
    /// Send queued OUT steps that no IN followed (none in normal commands).
    async fn flush(&mut self) -> Result<()> {
        if !self.pending.is_empty() {
            let steps = std::mem::take(&mut self.pending);
            self.batch(steps).await?;
        }
        Ok(())
    }
    /// One round trip for several framed commands: each is control(write
    /// envelope), its bytes, control(read envelope), then its reply.
    pub(crate) async fn commands(
        &mut self,
        commands: Vec<([u8; 12], Vec<u8>, [u8; 12], usize)>,
    ) -> Result<Vec<Vec<u8>>> {
        self.flush().await?;
        let mut steps = Vec::with_capacity(commands.len() * 4);
        for (write, bytes, read, reply) in commands {
            steps.push((Operation::ControlOut { setup: vendor_setup(), data: write.to_vec() },
                        TransferStage::Control(RequestKind::WriteCommand)));
            steps.push((Operation::TransferOut { endpoint: self.output, data: bytes }, TransferStage::BulkOut));
            steps.push((Operation::ControlOut { setup: vendor_setup(), data: read.to_vec() },
                        TransferStage::Control(RequestKind::ReadCommand)));
            steps.push((Operation::TransferIn { endpoint: self.input, length: self.read_length(reply)? },
                        TransferStage::BulkIn));
        }
        self.batch(steps).await
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
        self.pending.clear();
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
        self.pending.push((
            Operation::ControlOut { setup: vendor_setup(), data: envelope.to_vec() },
            TransferStage::Control(kind),
        ));
        Ok(())
    }
    async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.pending.push((
            Operation::TransferOut { endpoint: self.output, data: bytes.to_vec() },
            TransferStage::BulkOut,
        ));
        Ok(())
    }
    async fn read(&mut self, maximum: usize) -> Result<Vec<u8>> {
        let length = self.read_length(maximum)?;
        let mut steps = std::mem::take(&mut self.pending);
        steps.push((Operation::TransferIn { endpoint: self.input, length }, TransferStage::BulkIn));
        let mut replies = self.batch(steps).await?;
        let data = replies.pop().ok_or_else(|| fujimod_usb_proxy::Error("missing IN reply".into()))?;
        if data.len() > length {
            return Err(fujimod_usb_proxy::Error("inconsistent USB completion length".into()).into());
        }
        Ok(data)
    }
}

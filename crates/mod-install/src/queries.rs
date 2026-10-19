use fujimod_usb::{
    Error, Result,
    ff80::{
        commands::{
            diagnostic::DiagnosticTag,
            memory::{Address, MemoryRange},
        },
        sequence::SequenceCommand,
    },
};
use std::num::NonZeroU32;

/// A checked borrow of one callback word and its output buffer. Callback
/// authorization and code/cache qualification remain the caller's policy.
#[derive(Clone)]
pub struct BorrowedQueries {
    callback: Address,
    stock: Address,
    output: MemoryRange,
    original: Vec<u8>,
    commands: Vec<SequenceCommand>,
}
impl BorrowedQueries {
    pub fn new(
        callback: Address,
        stock: Address,
        output: Address,
        original: Vec<u8>,
    ) -> Result<Self> {
        let length = u32::try_from(original.len())
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or(Error::InvalidLength)?;
        let output = MemoryRange::new(output, length)?;
        MemoryRange::new(callback, NonZeroU32::new(4).expect("word"))?;
        if u64::from(callback.0) < u64::from(output.address().0) + u64::from(length.get())
            && u64::from(output.address().0) < u64::from(callback.0) + 4
        {
            return Err(Error::OutsideDataWindow);
        }
        let commands = vec![
            Self::read(callback, 4, Some(stock.0.to_le_bytes().to_vec()))?,
            Self::read(output.address(), length.get(), Some(original.clone()))?,
        ];
        Ok(Self {
            callback,
            stock,
            output,
            original,
            commands,
        })
    }
    fn read(address: Address, length: u32, expected: Option<Vec<u8>>) -> Result<SequenceCommand> {
        Ok(SequenceCommand::Read {
            range: MemoryRange::new(
                address,
                NonZeroU32::new(length).ok_or(Error::InvalidLength)?,
            )?,
            expected,
        })
    }
    fn write(&mut self, address: Address, bytes: Vec<u8>) -> Result<()> {
        let length = bytes.len() as u32;
        self.commands.push(SequenceCommand::Write {
            address,
            bytes: bytes.clone(),
        });
        self.commands
            .push(Self::read(address, length, Some(bytes))?);
        Ok(())
    }
    /// Append one callback; return indices of its raw query reply and output.
    /// Restoration is appended once, by `commands()`, after the entire batch.
    pub fn call(
        &mut self,
        callback: Address,
        tag: DiagnosticTag,
        statuses: Vec<(u32, u32)>,
        output: Vec<u8>,
    ) -> Result<(usize, usize)> {
        if output.len() != self.original.len() || statuses.is_empty() || statuses.len() > 8 {
            return Err(Error::InvalidLength);
        }
        if self.commands.len() + 7 + 4 > 512 {
            return Err(Error::LengthLimit {
                requested: (self.commands.len() + 11) as u32,
                limit: 512,
            });
        }
        self.write(self.output.address(), output)?;
        self.write(self.callback, callback.0.to_le_bytes().to_vec())?;
        let query = self.commands.len();
        self.commands.push(SequenceCommand::Query { tag, statuses });
        self.commands.push(Self::read(
            self.callback,
            4,
            Some(callback.0.to_le_bytes().to_vec()),
        )?);
        let result = self.commands.len();
        self.commands.push(SequenceCommand::Read {
            range: self.output,
            expected: None,
        });
        Ok((query, result))
    }
    pub fn commands(&self) -> Vec<SequenceCommand> {
        if self.commands.len() == 2 {
            return Vec::new();
        }
        let mut plan = self.clone();
        plan.write(self.callback, self.stock.0.to_le_bytes().to_vec())
            .expect("validated callback range");
        plan.write(self.output.address(), self.original.clone())
            .expect("validated output range");
        plan.commands
    }
}

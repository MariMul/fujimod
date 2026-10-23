//! EXR II SD0 at FFF50000. SDHC default speed is modeled; MMC, erase, tuning,
//! and UHS refuse. Latencies and card identity are declared inputs.
//! SDMA writes bytes before status and IRQ; boundaries require W1C/resume.
use crate::{Cx, Device, bus::InterruptLine, ip::sdhci::Clocks, parts::sd_card::Card};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

pub const BASE: u64 = 0xfff50000;
/// Shared physical contacts, also readable by the board's GPIO input wiring.
#[derive(Clone)]
pub struct Contacts(Arc<AtomicU8>);
impl Contacts {
    pub fn checkpoint_level(&self) -> u8 {
        self.0.load(Ordering::Relaxed)
    }
    pub fn restore_level(&self, bits: u8) {
        self.0.store(bits, Ordering::Relaxed);
    }
    pub fn new(present: bool, protected: bool) -> Self {
        Self(Arc::new(AtomicU8::new(
            present as u8 | (protected as u8) << 1,
        )))
    }
    pub fn present(&self) -> bool {
        self.0.load(Ordering::Relaxed) & 1 != 0
    }
    pub fn protected(&self) -> bool {
        self.0.load(Ordering::Relaxed) & 2 != 0
    }
    pub fn set(&self, present: bool, protected: bool) {
        self.0
            .store(present as u8 | (protected as u8) << 1, Ordering::Relaxed);
    }
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    /// Both register-width interfaces on the same physical host and card.
    Sdhc,
    Application,
    Rom,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub protocol: Protocol,
    pub transfer_ticks: u64,
    pub clock_ticks: u64,
    pub rca: u16,
    pub ram_bytes: u64,
}
/// Direction of a multi-block data transfer on the card's data lines.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataDirection {
    Read,
    Write,
}
/// An external data CRC failure on the card's data lines. It never selects a
/// filesystem result: the host reports SDHCI data CRC error status and the
/// guest's own recovery runs.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DataFault {
    pub direction: DataDirection,
    /// Sector whose transfer fails; `None` selects each transfer's first sector.
    pub sector: Option<u32>,
    /// Sectors after `sector` that still transfer before the failure.
    pub after_sectors: u32,
    /// Matching multi-block transfers that fail, firmware retries included.
    pub occurrences: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DataFaultEvent {
    pub command: u8,
    pub sector: u32,
    pub transferred: usize,
}
const MAX_DATA_FAULTS: usize = 32;
const DATA_CRC_ERROR: u16 = 0x20;
const ERROR_INTERRUPT: u16 = 0x8000;
pub struct Wiring {
    pub card: Card,
    pub contacts: Contacts,
    pub irq: InterruptLine,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum State {
    Idle,
    Ready,
    Identification,
    Standby,
    Transfer,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
enum Payload {
    None,
    Metadata {
        #[serde(with = "crate::checkpoint::array")]
        bytes: [u8; 64],
        len: usize,
    },
    Read(u32),
    Write(u32),
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Transfer {
    Dma,
    PioRead,
    PioWrite,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Phase {
    Command,
    Ready,
    Sector,
    Boundary,
}
#[derive(Serialize, Deserialize)]
struct Request {
    state: State,
    app: bool,
    block_length: usize,
    bus_width: u8,
    response: [u8; 16],
    payload: Payload,
    transfer: Transfer,
    quantity: usize,
    cursor: usize,
    address: u32,
    phase: Phase,
    due: Option<u64>,
    fifo_position: usize,
    command: u8,
    fault: Option<usize>,
}
/// Fixed register storage shared with SDIO. No map allocation on MMIO.
#[derive(Serialize, Deserialize)]
pub(crate) struct Registers(#[serde(with = "crate::checkpoint::array")] pub [u16; 132]);
impl Registers {
    pub fn new() -> Self {
        Self([0; 132])
    }
    pub fn get(&self, offset: usize) -> u16 {
        self.0[offset / 2]
    }
    pub fn set(&mut self, offset: usize, value: u16) {
        self.0[offset / 2] = value;
    }
    pub fn word(&self, offset: usize) -> u32 {
        self.get(offset) as u32 | (self.get(offset + 2) as u32) << 16
    }
    pub fn put_word(&mut self, offset: usize, value: u32) {
        self.set(offset, value as u16);
        self.set(offset + 2, (value >> 16) as u16);
    }
    pub fn valid(offset: usize, size: u8) -> bool {
        size == 2
            && matches!(
                offset,
                4 | 6
                    | 8
                    | 10
                    | 12
                    | 14
                    | 16
                    | 18
                    | 20
                    | 22
                    | 24
                    | 26
                    | 28
                    | 30
                    | 0x28
                    | 0x2e
                    | 0x30
                    | 0x32
                    | 0x34
                    | 0x36
                    | 0x38
                    | 0x3a
                    | 0x3e
                    | 0x100
                    | 0x104
            )
            || size == 4 && matches!(offset, 0 | 12 | 0x20 | 0x24)
    }
    pub fn asserted(&self) -> bool {
        self.get(0x30) & self.get(0x34) & self.get(0x38) != 0
            || self.get(0x32) & self.get(0x36) & self.get(0x3a) != 0
    }
}
pub struct SdHost {
    options: Options,
    card: Card,
    contacts: Contacts,
    irq: InterruptLine,
    regs: Registers,
    clocks: Clocks,
    state: State,
    app: bool,
    block_length: usize,
    bus_width: u8,
    response: [u8; 16],
    request: Option<Request>,
    fifo: [u8; 512],
    scratch: Box<[u8]>,
    rom_fifo_position: usize,
    rom_fifo_ready: bool,
    faults: Vec<DataFault>,
    fault_history: Vec<DataFaultEvent>,
}
impl SdHost {
    pub fn new(options: Options, wiring: Wiring) -> Result<Self, String> {
        if options.transfer_ticks == 0
            || options.rca == 0
            || options.ram_bytes == 0
            || options.ram_bytes > 0x20000000
        {
            return Err("Invalid SD latency, RCA or DRAM extent".into());
        }
        let clocks = Clocks::new(&[(BASE + 0x2c, options.clock_ticks)])?;
        Ok(Self {
            options,
            card: wiring.card,
            contacts: wiring.contacts,
            irq: wiring.irq,
            regs: Registers::new(),
            clocks,
            state: State::Idle,
            app: false,
            block_length: 512,
            bus_width: 1,
            response: [0; 16],
            request: None,
            fifo: [0; 512],
            scratch: vec![0; 512 * 1024].into_boxed_slice(),
            rom_fifo_position: 0,
            rom_fifo_ready: false,
            faults: Vec::new(),
            fault_history: Vec::new(),
        })
    }
    /// Declare a data CRC failure for later multi-block transfers.
    pub fn inject_data_fault(&mut self, fault: DataFault) -> Result<(), String> {
        if self.busy() {
            return Err("configure SD data faults while the host is idle".into());
        }
        if fault.sector.is_some_and(|s| s >= self.card.sectors())
            || fault.after_sectors > 0xffff
            || !(1..=0x10000).contains(&fault.occurrences)
            || self.faults.len() >= MAX_DATA_FAULTS
        {
            return Err(
                "SD data fault sector, prefix, occurrences or rule count unsupported".into(),
            );
        }
        self.faults.push(fault);
        Ok(())
    }
    pub fn clear_data_faults(&mut self) -> Result<(), String> {
        if self.busy() {
            return Err("clear SD data faults while the host is idle".into());
        }
        self.faults.clear();
        Ok(())
    }
    pub fn data_faults(&self) -> &[DataFault] {
        &self.faults
    }
    pub fn fault_history(&self) -> &[DataFaultEvent] {
        &self.fault_history
    }
    fn select_fault(&mut self, r: &mut Request) -> Result<(), String> {
        let (direction, first) = match (r.command, r.payload) {
            (18, Payload::Read(first)) => (DataDirection::Read, first),
            (25, Payload::Write(first)) => (DataDirection::Write, first),
            _ => return Ok(()),
        };
        let Some((index, cursor)) = self.faults.iter().enumerate().find_map(|(index, f)| {
            let cursor = (i64::from(f.sector.unwrap_or(first)) - i64::from(first)
                + i64::from(f.after_sectors))
                * 512;
            (f.direction == direction && (0..r.quantity as i64).contains(&cursor))
                .then_some((index, cursor as usize))
        }) else {
            return Ok(());
        };
        if r.transfer != Transfer::Dma && cursor != 0 {
            return Err("partial PIO data CRC faults are not modeled".into());
        }
        r.fault = Some(cursor);
        let fault = &mut self.faults[index];
        fault.occurrences -= 1;
        if fault.occurrences == 0 {
            self.faults.remove(index);
        }
        Ok(())
    }
    /// Data CRC error status ends the transfer at the failing byte position.
    fn data_error(&mut self, cx: &mut Cx<'_>, r: Request) -> Result<(), String> {
        let sector = match r.payload {
            Payload::Read(first) | Payload::Write(first) => first,
            _ => unreachable!(),
        };
        if self.fault_history.len() == MAX_DATA_FAULTS {
            self.fault_history.remove(0);
        }
        self.fault_history.push(DataFaultEvent {
            command: r.command,
            sector,
            transferred: r.cursor,
        });
        self.regs.set(0x32, self.regs.get(0x32) | DATA_CRC_ERROR);
        self.finish(cx, r, ERROR_INTERRUPT)
    }
    pub fn card(&self) -> &Card {
        &self.card
    }
    /// Session construction assigns an external medium's initial identity.
    pub fn initialize_revision(&mut self, revision: u64) -> Result<(), String> {
        if self.busy() {
            return Err("SD medium is busy".into());
        }
        self.card.initialize_revision(revision);
        Ok(())
    }
    pub fn busy(&self) -> bool {
        self.request.is_some() || self.rom_fifo_ready
    }
    pub fn replace_card(&mut self, mut card: Card) -> Result<Card, String> {
        if self.busy() {
            return Err("SD medium is busy".into());
        }
        card.supersede(self.card.revision())?;
        Ok(std::mem::replace(&mut self.card, card))
    }
    /// A new, unexecuted board reconnects its external medium without editing
    /// content or manufacturing a revision. The session checks cold ownership.
    pub fn retain_card(&mut self, card: Card) -> Result<(), String> {
        if self.busy() || card.sectors() != self.card.sectors() {
            return Err("retained SD medium differs or host is busy".into());
        }
        self.card = card;
        Ok(())
    }
    pub fn files(&self, revision: Option<u64>) -> Result<crate::storage::fat::Reader<'_>, String> {
        if !self.contacts.present() {
            return Err("No SD card inserted".into());
        }
        if self.busy() {
            return Err("SD medium is busy".into());
        }
        if revision.is_some_and(|r| r != self.card.revision()) {
            return Err("SD medium revision changed".into());
        }
        crate::storage::fat::Reader::new(&self.card)
    }
    fn signal(&self, cx: &mut Cx<'_>) -> Result<(), String> {
        self.irq.drive(cx, self.regs.asserted())
    }
    fn r1(&self) -> u32 {
        let state = match self.state {
            State::Idle => 0,
            State::Ready => 1,
            State::Identification => 2,
            State::Standby => 3,
            State::Transfer => 4,
        };
        state << 9
            | if self.state == State::Transfer || self.options.protocol == Protocol::Rom {
                0x100
            } else {
                0
            }
    }
    fn physical(&self, address: u32, len: usize, rom: bool) -> Result<u64, String> {
        let a = address as u64;
        if rom && !(0x40000000..0x60000000).contains(&a) {
            return Err("ROM SDMA requires DRAM alias".into());
        }
        let physical = if (0x40000000..0x60000000).contains(&a) {
            a & 0x3fffffff
        } else {
            a
        };
        if address & 3 != 0
            || len == 0
            || len > 0x2000000
            || physical
                .checked_add(len as u64)
                .is_none_or(|end| end > self.options.ram_bytes)
        {
            return Err("SD DMA exceeds aligned DRAM".into());
        }
        // ROM must retain the alias while physical low RAM is shadowed by ROM.
        Ok(if rom { a } else { physical })
    }
    fn plan(&self, word: u32, argument: u32) -> Result<Request, String> {
        let command = (word >> 24 & 63) as u8;
        let rom = self.options.protocol == Protocol::Rom;
        let app = self.app && command != 55;
        let expected = if rom {
            (match command {
                0 => 0,
                8 | 55 | 3 | 16 | 6 => 0x1a,
                41 => 2,
                2 => 9,
                7 => 0x1b,
                17 => 0x3a,
                _ => 0xff,
            }) as u32
        } else if app {
            match command {
                6 => 0x061a0000,
                13 => 0x0d3a0011,
                41 => 0x29020000,
                42 => 0x2a1a0000,
                51 => 0x333a0011,
                _ => u32::MAX,
            }
        } else {
            match command {
                0 => 0,
                2 => 0x02090000,
                3 => 0x031a0000,
                7 => 0x071b0000,
                8 => 0x081a0000,
                9 => 0x09090000,
                12 => 0x0cdb0000,
                13 => 0x0d1a0000,
                16 => 0x101a0000,
                18 => 0x123a0037,
                25 => 0x193a0027,
                55 => 0x371a0000,
                6 => 0x063a0011,
                _ => u32::MAX,
            }
        };
        let single =
            self.options.protocol == Protocol::Sdhc && matches!(word, 0x113a0010 | 0x113a0011);
        let pio = !rom && (matches!(word, 0x193a0026 | 0x123a0036) || single && word & 1 == 0);
        if (rom && word >> 16 & 255 != expected) || (!rom && word != expected && !pio && !single) {
            return Err(format!("Unsupported SD command {command}/{word:#x}"));
        }
        let mut request = Request {
            state: self.state,
            app: false,
            block_length: self.block_length,
            bus_width: self.bus_width,
            response: [0; 16],
            payload: Payload::None,
            transfer: Transfer::Dma,
            quantity: 0,
            cursor: 0,
            address: self.regs.word(0),
            phase: Phase::Command,
            due: None,
            fifo_position: 0,
            command,
            fault: None,
        };
        let mut response = self.r1();
        let rca = (self.options.rca as u32) << 16;
        let bad = match command {
            0 => {
                request.state = State::Idle;
                request.block_length = 512;
                request.bus_width = 1;
                response = 0;
                argument != 0
            }
            8 => {
                response = 0x1aa;
                self.state != State::Idle || argument != 0x1aa
            }
            55 => {
                request.app = true;
                response |= 0x20;
                if rom {
                    argument >> 16
                        != if matches!(self.state, State::Standby | State::Transfer) {
                            self.options.rca as u32
                        } else {
                            0
                        }
                } else {
                    !matches!(self.state, State::Idle | State::Standby | State::Transfer)
                        || ![0, rca].contains(&argument)
                }
            }
            41 if app => {
                request.state = State::Ready;
                response =
                    if rom || self.options.protocol == Protocol::Sdhc && argument == 0x40100000 {
                        0xc0ff8000
                    } else {
                        0xc0100000
                    };
                self.state != State::Idle
                    || !(argument == if rom { 0x40100000 } else { 0x41100000 }
                        || self.options.protocol == Protocol::Sdhc && argument == 0x40100000)
            }
            2 => {
                request.state = State::Identification;
                if !rom {
                    let cid = u128::from_be_bytes(*b"\xfeMCVIRT0\x10\x12\x34\x56\x78\x01\xaa\xff");
                    request.response = (cid >> 8).to_le_bytes();
                }
                self.state != State::Ready || argument != 0
            }
            3 => {
                request.state = State::Standby;
                response = rca | if rom { 0x500 } else { 0x600 };
                self.state != State::Identification || argument != 0
            }
            9 if !rom => {
                let csd = 1u128 << 126
                    | 0x0eu128 << 112
                    | 0x32u128 << 96
                    | 0x515u128 << 84
                    | 9u128 << 80
                    | (self.card.sectors() as u128 / 1024 - 1) << 48
                    | 1u128 << 46
                    | 0x7fu128 << 39
                    | 9u128 << 22
                    | 1;
                request.response = (csd >> 8).to_le_bytes();
                self.state != State::Standby || argument != rca
            }
            7 => {
                request.state = if argument != 0 {
                    State::Transfer
                } else {
                    State::Standby
                };
                response = if argument != 0 { 0x900 } else { 0x600 };
                !matches!(self.state, State::Standby | State::Transfer)
                    || ![0, rca].contains(&argument)
                    || rom && argument == 0
            }
            _ if self.state != State::Transfer => true,
            16 => {
                request.block_length = argument as usize;
                ![8, 64, 512].contains(&argument) || rom && argument != 512
            }
            42 if app && !rom => argument != 0,
            6 if app => {
                request.bus_width = 4;
                argument != 2
            }
            13 if app && !rom => {
                let mut bytes = [0; 64];
                bytes[0] = 0x80;
                bytes[10] = 0x70;
                request.payload = Payload::Metadata { bytes, len: 64 };
                argument != 0 || self.block_length != 64 || self.bus_width != 4
            }
            51 if app && !rom => {
                let mut bytes = [0; 64];
                bytes[..2].copy_from_slice(&[2, 5]);
                request.payload = Payload::Metadata { bytes, len: 8 };
                argument != 0 || self.block_length != 8
            }
            6 if !rom => {
                let mut bytes = [0; 64];
                bytes[13] = 1;
                request.payload = Payload::Metadata { bytes, len: 64 };
                argument != 1 || self.block_length != 64
            }
            17 | 18 | 25 => {
                let count = if command == 17 {
                    1
                } else {
                    let count = self.regs.get(6);
                    if count == 0 { 65536 } else { count as u32 }
                };
                self.card.range(argument, count)?;
                request.payload = if command == 25 {
                    Payload::Write(argument)
                } else {
                    Payload::Read(argument)
                };
                request.quantity = count as usize * 512;
                if pio || rom && self.regs.get(12) == 0x10 {
                    request.transfer = if command == 25 {
                        Transfer::PioWrite
                    } else {
                        Transfer::PioRead
                    };
                }
                self.block_length != 512
                    || command == 25 && self.contacts.protected()
                    || (rom || single)
                        && (self.regs.get(6) != 1 || ![0x10, 0x11].contains(&self.regs.get(12)))
            }
            12 if !rom => argument != 0,
            13 if !rom => argument != rca,
            _ => true,
        };
        if bad {
            return Err(format!(
                "SD CMD{command} argument or card state unsupported"
            ));
        }
        if !matches!(command, 2 | 9) {
            request.response[..4].copy_from_slice(&response.to_le_bytes());
        }
        if let Payload::Metadata { len, .. } = request.payload {
            request.quantity = len;
        }
        Ok(request)
    }
    fn start(&mut self, cx: &mut Cx<'_>, word: u32) -> Result<(), String> {
        let rom = self.options.protocol == Protocol::Rom;
        if self.busy()
            || !self.contacts.present()
            || self.regs.get(0x30) != 0
            || self.regs.get(0x32) != 0
        {
            return Err("SD command requires present idle card and cleared status".into());
        }
        let clock = self.clocks.read(cx, BASE + 0x2c, 2);
        if clock & 7 != 7
            || self.regs.get(0x28) & 0x100 == 0 && !rom
            || rom && self.regs.get(0x28) >> 8 & 15 != 15
        {
            return Err("SD command before clock/power setup".into());
        }
        let argument = self.regs.word(8);
        let mut r = self.plan(word, argument)?;
        if rom && r.command != 17 && self.regs.get(12) != 0 {
            return Err("ROM non-data command has transfer mode bits".into());
        }
        if rom && self.regs.get(0x34) & 3 != 3 {
            return Err("ROM command requires status enabled".into());
        }
        if r.quantity != 0 {
            if self.regs.get(4) & 0xfff
                != if matches!(r.payload, Payload::Metadata { .. }) {
                    r.quantity as u16
                } else {
                    512
                }
                || self.regs.get(0x28) & 0x18 != 0
            {
                return Err("SD block-size or DMA mode unsupported".into());
            }
            if !rom && r.command != 17 && self.regs.get(0x34) & self.regs.get(0x38) & 2 == 0 {
                return Err("SD data-complete signal disabled".into());
            }
            if r.transfer == Transfer::Dma {
                self.physical(r.address, r.quantity, rom || r.command == 17)?;
                let boundary = 4096usize << (self.regs.get(4) >> 12 & 7);
                if !rom
                    && r.quantity > boundary - r.address as usize % boundary
                    && self.regs.get(0x34) & self.regs.get(0x38) & 8 == 0
                {
                    return Err("SDMA boundary interrupt disabled".into());
                }
                if matches!(r.payload, Payload::Write(_))
                    && r.address & if r.address & 0x40000000 != 0 { 7 } else { 31 } != 0
                {
                    return Err("SDMA write alignment unsupported".into());
                }
            } else if !rom && r.command != 17 {
                let signals = if r.transfer == Transfer::PioWrite {
                    0x13
                } else {
                    0x23
                };
                if self.regs.get(0x34) & self.regs.get(0x38) & signals != signals {
                    return Err("SD PIO directional signals disabled".into());
                }
            }
            if let Payload::Write(first) = r.payload {
                self.card.reserve_write(first, (r.quantity / 512) as u32)?;
            }
        }
        r.due = Some(
            cx.now()
                .checked_add(self.options.transfer_ticks)
                .ok_or("SD deadline overflow")?,
        );
        self.select_fault(&mut r)?;
        self.regs.put_word(12, word);
        self.request = Some(r);
        Ok(())
    }
    fn finish(&mut self, cx: &mut Cx<'_>, r: Request, status: u16) -> Result<(), String> {
        self.state = r.state;
        self.app = r.app;
        self.block_length = r.block_length;
        self.bus_width = r.bus_width;
        self.response = r.response;
        self.regs.set(0x30, self.regs.get(0x30) | status);
        self.signal(cx)
    }
    fn complete(&mut self, cx: &mut Cx<'_>) -> Result<(), String> {
        let Some(mut r) = self.request.take() else {
            return Ok(());
        };
        if r.due.is_none_or(|due| due > cx.now()) {
            self.request = Some(r);
            return Ok(());
        }
        let rom = self.options.protocol == Protocol::Rom;
        if !self.contacts.present()
            || matches!(r.payload, Payload::Write(_)) && self.contacts.protected()
        {
            self.request = Some(r);
            return Err("SD contacts changed during transfer".into());
        }
        r.due = None;
        self.response = r.response;
        if r.quantity == 0 {
            // A busy response (R1b) ends with transfer complete when enabled.
            let busy_end = if r.command == 7 && (rom || self.options.protocol == Protocol::Sdhc) {
                2 & self.regs.get(0x34)
            } else {
                0
            };
            let status = 1 | busy_end;
            return self.finish(
                cx,
                r,
                status & if rom { self.regs.get(0x34) } else { u16::MAX },
            );
        }
        if r.transfer != Transfer::Dma {
            if r.phase == Phase::Sector && r.transfer == Transfer::PioWrite {
                if let Payload::Write(first) = r.payload {
                    self.card
                        .write_bytes(first as u64 * 512 + r.cursor as u64, &self.fifo)?;
                }
                r.cursor += 512;
            }
            self.regs.set(6, ((r.quantity - r.cursor) / 512) as u16);
            if r.fault == Some(r.cursor) {
                return self.data_error(cx, r);
            }
            if r.cursor == r.quantity {
                return self.finish(cx, r, 2);
            }
            if r.transfer == Transfer::PioRead
                && let Payload::Read(first) = r.payload
            {
                self.card
                    .read_bytes(first as u64 * 512 + r.cursor as u64, &mut self.fifo)?;
            }
            if rom {
                self.rom_fifo_position = 0;
                self.rom_fifo_ready = true;
                return self.finish(cx, r, 0x21 & self.regs.get(0x34));
            }
            let status = if r.transfer == Transfer::PioWrite {
                0x10
            } else {
                0x20
            } | if r.phase == Phase::Command { 1 } else { 0 };
            r.phase = Phase::Ready;
            r.fifo_position = 0;
            self.request = Some(r);
            self.regs.set(0x30, self.regs.get(0x30) | status);
            return self.signal(cx);
        }
        if r.fault == Some(r.cursor) {
            return self.data_error(cx, r);
        }
        let boundary = 4096usize << (self.regs.get(4) >> 12 & 7);
        let address = r
            .address
            .checked_add(r.cursor as u32)
            .ok_or("SDMA address overflow")?;
        let end = r.fault.filter(|f| *f > r.cursor).unwrap_or(r.quantity);
        let length = (end - r.cursor).min(boundary - address as usize % boundary);
        let physical = self.physical(address, length, rom || r.command == 17)?;
        match r.payload {
            Payload::Write(first) => {
                cx.ram_read(physical, &mut self.scratch[..length])?;
                self.card.write_bytes(
                    first as u64 * 512 + r.cursor as u64,
                    &self.scratch[..length],
                )?;
            }
            Payload::Read(first) => {
                self.card.read_bytes(
                    first as u64 * 512 + r.cursor as u64,
                    &mut self.scratch[..length],
                )?;
                cx.ram_write(physical, &self.scratch[..length])?;
            }
            Payload::Metadata { bytes, .. } => {
                cx.ram_write(physical, &bytes[r.cursor..r.cursor + length])?
            }
            Payload::None => unreachable!(),
        }
        r.cursor += length;
        self.regs.set(
            6,
            (r.quantity.div_ceil(self.block_length) - r.cursor / self.block_length) as u16,
        );
        self.regs.put_word(0, r.address + r.cursor as u32);
        if r.fault == Some(r.cursor) {
            return self.data_error(cx, r);
        }
        if r.cursor < r.quantity {
            r.phase = Phase::Boundary;
            self.request = Some(r);
            self.regs.set(0x30, self.regs.get(0x30) | 9);
            self.signal(cx)
        } else {
            self.finish(cx, r, 3 & if rom { self.regs.get(0x34) } else { u16::MAX })
        }
    }
    fn pio(&mut self, cx: &mut Cx<'_>, write: Option<u32>) -> Result<u32, String> {
        if self.options.protocol == Protocol::Rom {
            if write.is_some() || !self.rom_fifo_ready {
                return Err("ROM SD PIO read without ready sector".into());
            }
            let value = u32::from_le_bytes(
                self.fifo[self.rom_fifo_position..self.rom_fifo_position + 4]
                    .try_into()
                    .unwrap(),
            );
            self.rom_fifo_position += 4;
            if self.rom_fifo_position == 512 {
                self.rom_fifo_ready = false;
                self.regs
                    .set(0x30, self.regs.get(0x30) | (2 & self.regs.get(0x34)));
            }
            return Ok(value);
        }
        let r = self.request.as_mut().ok_or("SD PIO without request")?;
        let is_write = r.transfer == Transfer::PioWrite;
        if r.phase != Phase::Ready
            || is_write != write.is_some()
            || r.command != 17 && self.regs.get(0x30) & if is_write { 0x10 } else { 0x20 } != 0
        {
            return Err("SD PIO requires ready buffer and W1C ACK".into());
        }
        let value = if let Some(value) = write {
            self.fifo[r.fifo_position..r.fifo_position + 4].copy_from_slice(&value.to_le_bytes());
            0
        } else {
            u32::from_le_bytes(
                self.fifo[r.fifo_position..r.fifo_position + 4]
                    .try_into()
                    .unwrap(),
            )
        };
        r.fifo_position += 4;
        if !is_write {
            r.cursor += 4;
        }
        if r.fifo_position == 512 {
            r.phase = Phase::Sector;
            r.due = Some(
                cx.now()
                    .checked_add(self.options.transfer_ticks)
                    .ok_or("SD PIO deadline overflow")?,
            );
        }
        Ok(value)
    }
    fn valid(&self, offset: usize, size: u8) -> bool {
        if self.options.protocol == Protocol::Sdhc {
            Registers::valid(offset, size)
                || matches!(
                    (offset, size),
                    (8, 4) | (16, 4) | (0x28, 1) | (0x29, 1) | (0x2e, 1) | (0x2f, 1)
                )
        } else if self.options.protocol == Protocol::Application {
            Registers::valid(offset, size)
        } else {
            matches!(
                (offset, size),
                (0, 4)
                    | (4, 2)
                    | (6, 2)
                    | (8, 4)
                    | (12, 2)
                    | (14, 2)
                    | (16, 4)
                    | (0x20, 4)
                    | (0x24, 4)
                    | (0x28, 1)
                    | (0x29, 1)
                    | (0x2e, 1)
                    | (0x2f, 1)
                    | (0x30, 2)
                    | (0x32, 2)
                    | (0x34, 2)
                    | (0x36, 2)
            )
        }
    }
    fn store(
        &mut self,
        cx: &mut Cx<'_>,
        offset: usize,
        size: u8,
        value: u32,
    ) -> Result<(), String> {
        let rom = self.options.protocol == Protocol::Rom;
        if offset == 0x24 || (16..32).contains(&offset) {
            return Err("SD response/present-state is read-only".into());
        }
        if offset == 0x20 {
            self.pio(cx, Some(value))?;
            return Ok(());
        }
        if matches!(offset, 0x30 | 0x32) {
            self.regs
                .set(offset, self.regs.get(offset) & !(value as u16));
            return self.signal(cx);
        }
        if offset == 0x2e || offset == 0x2f {
            if value & if offset == 0x2f { !1 } else { !0x70f } != 0 {
                return Err("Unsupported SD reset/timeout bits".into());
            }
            if value & if offset == 0x2f { 1 } else { 0x700 } != 0 {
                self.request = None;
                self.rom_fifo_ready = false;
                self.regs.set(0x30, 0);
                self.regs.set(0x32, 0);
                if rom || offset == 0x2f {
                    self.regs = Registers::new();
                    self.response = [0; 16];
                    self.clocks.write(cx, BASE + 0x2c, 2, 0);
                }
                self.signal(cx)?;
            }
            if offset == 0x2e {
                self.regs.set(offset, value as u16 & 15);
            }
            return Ok(());
        }
        if offset == 14 || offset == 12 && size == 4 {
            return self.start(
                cx,
                if size == 4 {
                    value
                } else {
                    (value << 16) | self.regs.get(12) as u32
                },
            );
        }
        if offset == 0 && self.request.is_some() {
            let r = self.request.as_mut().unwrap();
            if r.phase != Phase::Boundary
                || self.regs.get(0x30) & 8 != 0
                || value != self.regs.word(0)
            {
                return Err("SDMA resume requires boundary ACK and current address".into());
            }
            r.phase = Phase::Command;
            r.due = Some(
                cx.now()
                    .checked_add(self.options.transfer_ticks)
                    .ok_or("SD deadline overflow")?,
            );
            return Ok(());
        }
        if self.request.is_some() {
            return Err("SD configuration changed during command".into());
        }
        if offset == 0x28 && !rom && value & !0xfff != 0
            || offset == 4 && value & 0x8000 != 0
            || offset == 0x3e && value != 0
            || offset == 0x100 && value & !7 != 0
            || offset == 0x104 && value != 0
        {
            return Err("Unsupported SD host/UHS/tuning configuration".into());
        }
        match size {
            4 => self.regs.put_word(offset, value),
            2 => self.regs.set(offset, value as u16),
            1 => {
                let shift = (offset % 2) * 8;
                let old = self.regs.get(offset & !1);
                self.regs.set(
                    offset & !1,
                    (old & !(255 << shift)) | ((value as u16) << shift),
                );
            }
            _ => unreachable!(),
        }
        self.signal(cx)
    }
}
#[derive(Serialize, Deserialize)]
struct Snapshot {
    card: Card,
    contacts: u8,
    regs: Registers,
    clocks: Clocks,
    state: State,
    app: bool,
    block_length: usize,
    bus_width: u8,
    response: [u8; 16],
    request: Option<Request>,
    #[serde(with = "crate::checkpoint::array")]
    fifo: [u8; 512],
    #[serde(with = "crate::checkpoint::bytes")]
    scratch: Box<[u8]>,
    rom_fifo_position: usize,
    rom_fifo_ready: bool,
    faults: Vec<DataFault>,
    fault_history: Vec<DataFaultEvent>,
}
impl Device for SdHost {
    fn checkpoint(&self) -> Result<crate::checkpoint::Checkpoint, String> {
        #[derive(Serialize)]
        struct View<'a> {
            card: &'a Card,
            contacts: u8,
            regs: &'a Registers,
            clocks: &'a Clocks,
            state: State,
            app: bool,
            block_length: usize,
            bus_width: u8,
            response: [u8; 16],
            request: &'a Option<Request>,
            fifo: &'a [u8],
            #[serde(with = "crate::checkpoint::bytes")]
            scratch: &'a [u8],
            rom_fifo_position: usize,
            rom_fifo_ready: bool,
            faults: &'a [DataFault],
            fault_history: &'a [DataFaultEvent],
        }
        crate::checkpoint::Checkpoint::encode(
            self.name(),
            &View {
                card: &self.card,
                contacts: self.contacts.checkpoint_level(),
                regs: &self.regs,
                clocks: &self.clocks,
                state: self.state,
                app: self.app,
                block_length: self.block_length,
                bus_width: self.bus_width,
                response: self.response,
                request: &self.request,
                fifo: &self.fifo,
                scratch: &self.scratch,
                rom_fifo_position: self.rom_fifo_position,
                rom_fifo_ready: self.rom_fifo_ready,
                faults: &self.faults,
                fault_history: &self.fault_history,
            },
        )
    }
    fn validate_checkpoint(&self, saved: &crate::checkpoint::Checkpoint) -> Result<(), String> {
        let _: Snapshot = saved.decode(self.name())?;
        Ok(())
    }
    fn restore_checkpoint(&mut self, saved: &crate::checkpoint::Checkpoint) -> Result<(), String> {
        let s: Snapshot = saved.decode(self.name())?;
        self.card = s.card;
        self.contacts.restore_level(s.contacts);
        self.regs = s.regs;
        self.clocks = s.clocks;
        self.state = s.state;
        self.app = s.app;
        self.block_length = s.block_length;
        self.bus_width = s.bus_width;
        self.response = s.response;
        self.request = s.request;
        self.fifo = s.fifo;
        self.scratch = s.scratch;
        self.rom_fifo_position = s.rom_fifo_position;
        self.rom_fifo_ready = s.rom_fifo_ready;
        self.faults = s.faults;
        self.fault_history = s.fault_history;
        Ok(())
    }

    fn fingerprint(&self) -> Result<serde_json::Value, String> {
        let request = self.request.as_ref().map(|r| {
            let payload = match r.payload {
                Payload::None => serde_json::json!({"kind":"none"}),
                Payload::Read(lba) => serde_json::json!({"kind":"read","lba":lba}),
                Payload::Write(lba) => serde_json::json!({"kind":"write","lba":lba}),
                Payload::Metadata { bytes, len } => serde_json::json!({"kind":"metadata","bytes":crate::fingerprint::bytes(&bytes),"len":len}),
            };
            serde_json::json!({"state":r.state,"app":r.app,"block_length":r.block_length,"bus_width":r.bus_width,"response":r.response,
                "payload":payload,"transfer":r.transfer,"quantity":r.quantity,"cursor":r.cursor,"address":r.address,"phase":r.phase,
                "due":r.due,"fifo_position":r.fifo_position,"command":r.command,"fault":r.fault})
        });
        Ok(
            serde_json::json!({"card":self.card.fingerprint(),"contacts":[self.contacts.present(),self.contacts.protected()],"registers":self.regs.0.as_slice(),
            "clocks":self.clocks.save(),"state":self.state,"app":self.app,"block_length":self.block_length,"bus_width":self.bus_width,
            "response":self.response,"request":request,"fifo":crate::fingerprint::bytes(&self.fifo),"scratch":crate::fingerprint::bytes(&self.scratch),
            "rom_fifo_position":self.rom_fifo_position,"rom_fifo_ready":self.rom_fifo_ready,
            "faults":self.faults,"fault_history":self.fault_history}),
        )
    }
    fn name(&self) -> &'static str {
        "exr2-sdhc"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        if address == BASE + 0x2c && size == 2 {
            return self.clocks.read(cx, address, size);
        }
        let offset = address.wrapping_sub(BASE) as usize;
        if !self.valid(offset, size) {
            cx.unmodeled();
            return 0;
        }
        self.advance(cx);
        if cx.refusal().is_some() {
            return 0;
        }
        if offset == 0x20 {
            return match self.pio(cx, None) {
                Ok(v) => v as u64,
                Err(e) => {
                    cx.fail(e);
                    0
                }
            };
        }
        if offset == 0x24 {
            return 0x20000
                | if self.contacts.present() { 0x10000 } else { 0 }
                | self.request.as_ref().map_or(0, |r| {
                    if r.phase == Phase::Ready {
                        if r.transfer == Transfer::PioWrite {
                            0x403
                        } else {
                            0x803
                        }
                    } else if r.quantity != 0 || matches!(r.command, 7 | 12) {
                        3
                    } else {
                        1
                    }
                });
        }
        if (16..32).contains(&offset) {
            let count = size as usize;
            let mut bytes = [0; 4];
            bytes[..count].copy_from_slice(&self.response[offset - 16..offset - 16 + count]);
            return u32::from_le_bytes(bytes) as u64;
        }
        match size {
            4 => self.regs.word(offset) as u64,
            2 => self.regs.get(offset) as u64,
            _ => (self.regs.get(offset & !1) >> ((offset % 2) * 8) & 255) as u64,
        }
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        if address == BASE + 0x2c && size == 2 {
            if self.request.is_some() {
                cx.fail("SD clock changed during command");
            } else {
                self.clocks.write(cx, address, size, value);
            }
            return;
        }
        let offset = address.wrapping_sub(BASE) as usize;
        if !self.valid(offset, size)
            || value
                > if size == 4 {
                    u32::MAX as u64
                } else {
                    (1u64 << (size * 8)) - 1
                }
        {
            cx.unmodeled();
            return;
        }
        self.advance(cx);
        if cx.refusal().is_some() {
            return;
        }
        if let Err(reason) = self.store(cx, offset, size, value as u32) {
            cx.fail(reason);
        }
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        self.clocks.advance(cx);
        if let Err(reason) = self.complete(cx) {
            cx.fail(reason);
        }
    }
    fn event(&mut self, cx: &mut Cx<'_>, _: u64) {
        self.advance(cx);
    }
    fn next_deadline(&self, now: u64) -> Option<u64> {
        self.request
            .as_ref()
            .and_then(|r| r.due)
            .into_iter()
            .chain(self.clocks.next_deadline(now))
            .min()
    }
    fn save(&self) -> serde_json::Value {
        serde_json::json!({
            "card": {"present":self.contacts.present(), "protected":self.contacts.protected(),
                "sectors":self.card.sectors(), "revision":self.card.revision(),
                "dirty_sectors":self.card.dirty_sectors()},
            "busy":self.busy(), "response":self.response, "registers":self.regs.0.as_slice(),
            "request":self.request.as_ref().map(|r| serde_json::json!({"command":r.command,
                "cursor":r.cursor,"quantity":r.quantity,"deadline":r.due})),
            "clocks":self.clocks.save(), "data_faults":self.faults, "fault_history":self.fault_history
        })
    }
    fn pollable(&self, address: u64, size: u8) -> bool {
        address != BASE + 0x20
            && (self.valid(address.wrapping_sub(BASE) as usize, size)
                || address == BASE + 0x2c && size == 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        parts::sd_card,
        test_host::ram::{Effect, Ram},
    };
    fn host(protocol: Protocol) -> SdHost {
        SdHost::new(
            Options {
                protocol,
                transfer_ticks: 10,
                clock_ticks: 2,
                rca: 0x1234,
                ram_bytes: 0x10000,
            },
            Wiring {
                card: Card::new(sd_card::Options {
                    sectors: 0x800000,
                    max_dirty_sectors: 65536,
                })
                .unwrap(),
                contacts: Contacts::new(true, false),
                irq: InterruptLine::Shared(78),
            },
        )
        .unwrap()
    }
    #[test]
    fn fingerprint_distinguishes_media_with_the_same_diagnostic_summary() {
        let mut a = host(Protocol::Sdhc);
        let mut b = host(Protocol::Sdhc);
        a.card.write_bytes(512, &[1; 512]).unwrap();
        b.card.write_bytes(512, &[2; 512]).unwrap();
        assert_eq!(a.save(), b.save());
        assert_ne!(a.fingerprint().unwrap(), b.fingerprint().unwrap());
        let stable = a.fingerprint().unwrap();
        assert_eq!(a.fingerprint().unwrap(), stable);
        let mut bytes = [0; 512];
        a.card.read_bytes(512, &mut bytes).unwrap();
        assert_eq!(bytes, [1; 512]);
    }
    fn write(m: &mut SdHost, h: &mut Ram, o: u64, s: u8, v: u64) {
        let mut cx = h.cx();
        m.write(&mut cx, BASE + o, s, v);
        assert_eq!(cx.refusal(), None, "offset {o:x}");
    }
    fn read(m: &mut SdHost, h: &mut Ram, o: u64, s: u8) -> u64 {
        let mut cx = h.cx();
        let v = m.read(&mut cx, BASE + o, s);
        assert_eq!(cx.refusal(), None);
        v
    }
    fn setup(m: &mut SdHost, h: &mut Ram) {
        write(m, h, 0x2c, 2, 5);
        h.now = 2;
        m.advance(&mut h.cx());
        if m.options.protocol == Protocol::Rom {
            write(m, h, 0x29, 1, 15);
        } else {
            write(m, h, 0x28, 2, 0x100);
        }
        write(m, h, 0x34, 2, 0xffff);
        if m.options.protocol == Protocol::Application {
            write(m, h, 0x38, 2, 0xffff);
        }
    }
    fn command(m: &mut SdHost, h: &mut Ram, word: u32, arg: u32) {
        write(m, h, 0x30, 2, 0xffff);
        if m.options.protocol == Protocol::Rom {
            write(m, h, 8, 4, arg as u64);
            write(m, h, 14, 2, (word >> 16) as u64);
        } else {
            write(m, h, 8, 2, (arg & 65535) as u64);
            write(m, h, 10, 2, (arg >> 16) as u64);
            write(m, h, 12, 4, word as u64);
        }
    }
    fn due(m: &mut SdHost, h: &mut Ram) {
        h.now = m.next_deadline(h.now).unwrap();
        let mut cx = h.cx();
        m.advance(&mut cx);
        assert_eq!(cx.refusal(), None);
    }
    #[test]
    fn enumeration_deadlines_clock_and_held_irq() {
        let mut m = host(Protocol::Application);
        let mut h = Ram::new(0x10000);
        setup(&mut m, &mut h);
        for (word, arg) in [
            (0, 0),
            (0x081a0000, 0x1aa),
            (0x371a0000, 0),
            (0x29020000, 0x41100000),
            (0x02090000, 0),
            (0x031a0000, 0),
            (0x09090000, 0x12340000),
            (0x071b0000, 0x12340000),
        ] {
            command(&mut m, &mut h, word, arg);
            let before = m.response;
            let tick = m.next_deadline(h.now).unwrap();
            h.now = tick - 1;
            m.advance(&mut h.cx());
            assert_eq!(m.response, before);
            due(&mut m, &mut h);
            assert_eq!(read(&mut m, &mut h, 0x30, 2) & 1, 1);
            assert_eq!(h.effects.last(), Some(&Effect::Irq(78, true)));
            write(&mut m, &mut h, 0x30, 2, 0xffff);
            assert_eq!(h.effects.last(), Some(&Effect::Irq(78, false)));
        }
        assert_eq!(m.state, State::Transfer);
        let mut cx = h.cx();
        m.write(&mut cx, BASE + 12, 4, 0x26000000);
        assert!(cx.refusal().is_some());
        assert!(!m.busy());
    }
    #[test]
    fn dma_boundary_requires_ack_resume_and_writes_before_irq() {
        let mut m = host(Protocol::Application);
        let mut h = Ram::new(0x10000);
        setup(&mut m, &mut h);
        m.state = State::Transfer;
        m.card.write_bytes(0, &vec![0x5a; 1024]).unwrap();
        write(&mut m, &mut h, 0, 4, 0xf00);
        write(&mut m, &mut h, 4, 2, 512);
        write(&mut m, &mut h, 6, 2, 2);
        command(&mut m, &mut h, 0x123a0037, 0);
        due(&mut m, &mut h);
        assert_eq!(m.next_deadline(h.now), None);
        assert_eq!(read(&mut m, &mut h, 0, 4), 0x1000);
        assert_eq!(&h.bytes[0xf00..0x1000], &[0x5a; 256]);
        assert_eq!(read(&mut m, &mut h, 6, 2), 2);
        assert_eq!(
            &h.effects[h.effects.len() - 2..],
            &[Effect::Write(0xf00, 256), Effect::Irq(78, true)]
        );
        let mut cx = h.cx();
        m.write(&mut cx, BASE, 4, 0x1000);
        assert!(cx.refusal().is_some());
        write(&mut m, &mut h, 0x30, 2, 9);
        write(&mut m, &mut h, 0, 4, 0x1000);
        due(&mut m, &mut h);
        assert_eq!(&h.bytes[0xf00..0x1300], &[0x5a; 1024]);
        assert_eq!(read(&mut m, &mut h, 6, 2), 0);
        assert!(!m.busy());
    }
    fn fault(direction: DataDirection, sector: Option<u32>, occurrences: u32) -> DataFault {
        DataFault {
            direction,
            sector,
            after_sectors: 0,
            occurrences,
        }
    }
    #[test]
    fn sdma_read_crc_fault_delivers_prefix_then_error_status_and_irq() {
        let mut m = host(Protocol::Application);
        let mut h = Ram::new(0x10000);
        setup(&mut m, &mut h);
        write(&mut m, &mut h, 0x36, 2, 0xffff);
        write(&mut m, &mut h, 0x3a, 2, 0xffff);
        m.state = State::Transfer;
        m.card.write_bytes(512 * 8, &[0x5a; 1024]).unwrap();
        m.inject_data_fault(fault(DataDirection::Read, Some(9), 1))
            .unwrap();
        write(&mut m, &mut h, 0, 4, 0x1000);
        write(&mut m, &mut h, 4, 2, 512);
        write(&mut m, &mut h, 6, 2, 2);
        command(&mut m, &mut h, 0x123a0037, 8);
        assert!(m.data_faults().is_empty());
        let checkpoint = m.checkpoint().unwrap();
        due(&mut m, &mut h);
        assert!(!m.busy());
        assert_eq!(&h.bytes[0x1000..0x1200], &[0x5a; 512]);
        assert_eq!(&h.bytes[0x1200..0x1400], &[0; 512]);
        assert_eq!(read(&mut m, &mut h, 0x32, 2), 0x20);
        assert_eq!(read(&mut m, &mut h, 0x30, 2), 0x8000);
        assert_eq!(read(&mut m, &mut h, 6, 2), 1);
        assert_eq!(h.effects.last(), Some(&Effect::Irq(78, true)));
        assert_eq!(
            m.fault_history(),
            [DataFaultEvent {
                command: 18,
                sector: 8,
                transferred: 512
            }]
        );
        write(&mut m, &mut h, 0x32, 2, 0xffff);
        write(&mut m, &mut h, 0x30, 2, 0xffff);
        assert_eq!(h.effects.last(), Some(&Effect::Irq(78, false)));
        // A retry is a fresh transfer; the exhausted rule no longer applies.
        write(&mut m, &mut h, 0, 4, 0x1000);
        write(&mut m, &mut h, 6, 2, 2);
        command(&mut m, &mut h, 0x123a0037, 8);
        due(&mut m, &mut h);
        assert_eq!(&h.bytes[0x1000..0x1400], &[0x5a; 1024]);
        assert_eq!(read(&mut m, &mut h, 0x32, 2), 0);
        assert_eq!(read(&mut m, &mut h, 0x30, 2), 3);
        // The checkpoint retains the armed request and fault position.
        let mut restored = host(Protocol::Application);
        restored.restore_checkpoint(&checkpoint).unwrap();
        let mut g = Ram::new(0x10000);
        g.now = h.now;
        due(&mut restored, &mut g);
        assert_eq!(restored.regs.get(0x32), 0x20);
        assert_eq!(restored.fault_history().len(), 1);
    }
    #[test]
    fn write_crc_faults_apply_to_sdma_prefixes_and_pio_command_start() {
        let mut m = host(Protocol::Application);
        let mut h = Ram::new(0x10000);
        setup(&mut m, &mut h);
        m.state = State::Transfer;
        h.bytes[0x1000..0x1400].fill(0x77);
        m.inject_data_fault(DataFault {
            direction: DataDirection::Write,
            sector: None,
            after_sectors: 1,
            occurrences: 1,
        })
        .unwrap();
        m.inject_data_fault(fault(DataDirection::Write, None, 1))
            .unwrap();
        assert!(
            m.inject_data_fault(fault(DataDirection::Read, None, 0))
                .is_err()
        );
        write(&mut m, &mut h, 0, 4, 0x1000);
        write(&mut m, &mut h, 4, 2, 512);
        write(&mut m, &mut h, 6, 2, 2);
        command(&mut m, &mut h, 0x193a0027, 4);
        due(&mut m, &mut h);
        assert_eq!(m.card.sector(4).unwrap(), [0x77; 512]);
        assert_eq!(m.card.sector(5).unwrap(), [0; 512]);
        assert_eq!(read(&mut m, &mut h, 0x32, 2), 0x20);
        write(&mut m, &mut h, 0x32, 2, 0xffff);
        write(&mut m, &mut h, 6, 2, 1);
        command(&mut m, &mut h, 0x193a0026, 6);
        due(&mut m, &mut h);
        assert!(!m.busy());
        assert_eq!(read(&mut m, &mut h, 0x30, 2), 0x8000);
        assert_eq!(read(&mut m, &mut h, 0x32, 2), 0x20);
        assert_eq!(m.fault_history()[1].transferred, 0);
        assert!(m.data_faults().is_empty());
        // A PIO failure after a committed prefix is not modeled.
        write(&mut m, &mut h, 0x32, 2, 0xffff);
        write(&mut m, &mut h, 0x30, 2, 0xffff);
        m.inject_data_fault(fault(DataDirection::Write, Some(7), 1))
            .unwrap();
        write(&mut m, &mut h, 6, 2, 2);
        write(&mut m, &mut h, 8, 2, 6);
        write(&mut m, &mut h, 10, 2, 0);
        let mut cx = h.cx();
        m.write(&mut cx, BASE + 12, 4, 0x193a0026);
        assert!(cx.refusal().is_some());
    }
    #[test]
    fn pio_sector_latency_reset_and_write_protection() {
        let mut m = host(Protocol::Application);
        let mut h = Ram::new(0x10000);
        setup(&mut m, &mut h);
        m.state = State::Transfer;
        write(&mut m, &mut h, 4, 2, 512);
        write(&mut m, &mut h, 6, 2, 1);
        command(&mut m, &mut h, 0x193a0026, 4);
        due(&mut m, &mut h);
        let mut cx = h.cx();
        m.write(&mut cx, BASE + 0x20, 4, 0xaa);
        assert!(cx.refusal().is_some());
        write(&mut m, &mut h, 0x30, 2, 0x11);
        for _ in 0..128 {
            write(&mut m, &mut h, 0x20, 4, 0x12345678);
        }
        assert_eq!(m.card.sector(4).unwrap(), [0; 512]);
        due(&mut m, &mut h);
        assert_eq!(&m.card.sector(4).unwrap()[..4], &[0x78, 0x56, 0x34, 0x12]);
        write(&mut m, &mut h, 0x30, 2, 0xffff);
        write(&mut m, &mut h, 6, 2, 1);
        command(&mut m, &mut h, 0x123a0036, 4);
        due(&mut m, &mut h);
        write(&mut m, &mut h, 0x30, 2, 0x21);
        for _ in 0..128 {
            assert_eq!(read(&mut m, &mut h, 0x20, 4), 0x12345678);
        }
        assert!(m.busy());
        due(&mut m, &mut h);
        assert!(!m.busy());
        write(&mut m, &mut h, 0x30, 2, 0xffff);
        command(&mut m, &mut h, 0x193a0026, 4);
        write(&mut m, &mut h, 0x2e, 2, 0x700);
        assert_eq!(m.next_deadline(h.now), None);
        m.contacts.set(true, true);
        let mut cx = h.cx();
        m.write(&mut cx, BASE + 12, 4, 0x193a0026);
        assert!(cx.refusal().is_some());
    }
    #[test]
    fn combined_host_keeps_media_across_rom_and_application_register_accesses() {
        let mut m = host(Protocol::Sdhc);
        let mut h = Ram::new(0x10000);
        setup(&mut m, &mut h);
        write(&mut m, &mut h, 0x29, 1, 15);
        write(&mut m, &mut h, 0x38, 2, 0);
        for (command, argument) in [
            (0u16, 0u32),
            (0x081a, 0x1aa),
            (0x371a, 0),
            (0x2902, 0x40100000),
            (0x0209, 0),
            (0x031a, 0),
            (0x071b, 0x12340000),
        ] {
            write(&mut m, &mut h, 0x30, 2, 0xffff);
            write(&mut m, &mut h, 8, 4, argument as u64);
            write(&mut m, &mut h, 12, 2, 0);
            write(&mut m, &mut h, 14, 2, command as u64);
            due(&mut m, &mut h);
        }
        assert_eq!(m.state, State::Transfer);
        m.card.write_bytes(0, &[0x42; 512]).unwrap();
        write(&mut m, &mut h, 0x30, 2, 0xffff);
        write(&mut m, &mut h, 4, 2, 512);
        write(&mut m, &mut h, 6, 2, 1);
        write(&mut m, &mut h, 8, 4, 0);
        write(&mut m, &mut h, 12, 2, 0x10);
        write(&mut m, &mut h, 14, 2, 0x113a);
        due(&mut m, &mut h);
        for _ in 0..128 {
            assert_eq!(read(&mut m, &mut h, 0x20, 4), 0x42424242);
        }
        due(&mut m, &mut h);
        assert!(!m.busy());
        write(&mut m, &mut h, 0x38, 2, 0xffff);
        command(&mut m, &mut h, 0, 0);
        due(&mut m, &mut h);
        assert_eq!(m.state, State::Idle);
        assert_eq!(m.card.sector(0).unwrap(), [0x42; 512]);
    }
    #[test]
    fn busy_select_ends_with_enabled_transfer_complete() {
        for (enabled, status) in [(0xffff, 3), (0xfffd, 1)] {
            let mut m = host(Protocol::Sdhc);
            let mut h = Ram::new(0x10000);
            setup(&mut m, &mut h);
            write(&mut m, &mut h, 0x34, 2, enabled);
            m.state = State::Standby;
            command(&mut m, &mut h, 0x071b0000, 0x12340000);
            due(&mut m, &mut h);
            assert_eq!(read(&mut m, &mut h, 0x30, 2), status);
            assert_eq!(m.state, State::Transfer);
        }
    }
    #[test]
    fn rom_single_sector_pio_and_alias_dma() {
        let mut m = host(Protocol::Rom);
        let mut h = Ram::new(0x10000);
        setup(&mut m, &mut h);
        m.state = State::Transfer;
        m.card.write_bytes(0, &[0x7b; 512]).unwrap();
        write(&mut m, &mut h, 4, 2, 512);
        write(&mut m, &mut h, 6, 2, 1);
        write(&mut m, &mut h, 12, 2, 0x10);
        command(&mut m, &mut h, 0x113a0000, 0);
        due(&mut m, &mut h);
        assert_eq!(read(&mut m, &mut h, 0x30, 2), 0x21);
        for _ in 0..128 {
            assert_eq!(read(&mut m, &mut h, 0x20, 4), 0x7b7b7b7b);
        }
        assert_eq!(read(&mut m, &mut h, 0x30, 2), 0x23);
        write(&mut m, &mut h, 0, 4, 0x40001000);
        write(&mut m, &mut h, 12, 2, 0x11);
        command(&mut m, &mut h, 0x113a0000, 0);
        due(&mut m, &mut h);
        assert!(h.effects.contains(&Effect::Write(0x40001000, 512)));
        assert_eq!(&h.bytes[0x1000..0x1200], &[0x7b; 512]);
    }
}

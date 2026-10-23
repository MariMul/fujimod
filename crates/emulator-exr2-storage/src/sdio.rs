//! EXR II SDIO host1, FFF51000. Owns a one-function part; no memory outside
//! the programmed SDMA extent is inspected. Timing is declared.
use fujimod_emulator_devices::{Cx, Device, bus::InterruptLine};
use fujimod_emulator_parts_storage::wifi_module::{Operation, Protocol, WifiModule};
use fujimod_emulator_ip::sdhci::Clocks;
use crate::sdhc::Registers;
use serde::{Deserialize, Serialize};
pub const BASE: u64 = 0xfff51000;
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub transfer_ticks: u64,
    pub clock_ticks: u64,
    pub rca: u16,
    pub ram_bytes: u64,
}
pub struct Wiring {
    pub module: WifiModule,
    pub irq: InterruptLine,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum State {
    Idle,
    Ready,
    Standby,
    Transfer,
}
#[derive(Serialize, Deserialize)]
struct Request {
    state: State,
    response: u32,
    operation: Option<Operation>,
    due: Option<u64>,
    dma: Option<u64>,
    write: bool,
    quantity: usize,
    pio: bool,
    pio_reads: usize,
    gates: [u16; 4],
}
pub struct SdioHost {
    options: Options,
    module: WifiModule,
    irq: InterruptLine,
    clocks: Clocks,
    regs: Registers,
    response: [u8; 16],
    state: State,
    request: Option<Request>,
    scratch: [u8; 2560],
}
impl SdioHost {
    pub fn new(options: Options, wiring: Wiring) -> Result<Self, String> {
        if options.transfer_ticks == 0
            || options.rca == 0
            || options.ram_bytes == 0
            || options.ram_bytes > 0x20000000
        {
            return Err("Invalid SDIO timing, RCA or RAM extent".into());
        }
        let clocks = Clocks::new(&[(BASE + 0x2c, options.clock_ticks)])?;
        Ok(Self {
            options,
            module: wiring.module,
            irq: wiring.irq,
            clocks,
            regs: Registers::new(),
            response: [0; 16],
            state: State::Idle,
            request: None,
            scratch: [0; 2560],
        })
    }
    fn signal(&self, cx: &mut Cx<'_>) -> Result<(), String> {
        self.irq.drive(cx, self.regs.asserted())
    }
    fn plan(&mut self, cx: &mut Cx<'_>, word: u32, arg: u32) -> Result<Request, String> {
        let command = word >> 24 & 63;
        let rca = (self.options.rca as u32) << 16;
        let mut r = Request {
            state: self.state,
            response: 0,
            operation: None,
            due: None,
            dma: None,
            write: false,
            quantity: 0,
            pio: false,
            pio_reads: 0,
            gates: [
                self.regs.get(0x34),
                self.regs.get(0x36),
                self.regs.get(0x38),
                self.regs.get(0x3a),
            ],
        };
        match command {
            5 if word == 0x05020000
                && matches!(self.state, State::Idle | State::Ready)
                && [0, 0x200000].contains(&arg) =>
            {
                r.response = 0x10200000 | if arg != 0 { 0x80000000 } else { 0 };
                r.state = if arg != 0 { State::Ready } else { State::Idle };
            }
            3 if word == 0x031a0000 && self.state == State::Ready && arg == 0 => {
                r.response = rca;
                r.state = State::Standby;
            }
            7 if word == 0x071b0000
                && matches!(self.state, State::Standby | State::Transfer)
                && [0, rca].contains(&arg) =>
            {
                r.response = if arg != 0 { 0x900 } else { 0x600 };
                r.state = if arg != 0 {
                    State::Transfer
                } else {
                    State::Standby
                };
            }
            52 if self.state == State::Transfer
                && word
                    == if arg & 0x80000000 != 0 {
                        0x341a0000
                    } else {
                        0x341a0010
                    } =>
            {
                let op = self.module.direct(arg)?;
                r.response = op.response;
                r.operation = Some(op);
            }
            53 if self.state == State::Transfer
                && self.module.protocol() == Protocol::RomDownload =>
            {
                let write = arg & 0x80000000 != 0;
                let function = arg >> 28 & 7;
                let block = arg & 0x08000000 != 0;
                let increment = arg & 0x04000000 != 0;
                let address = arg >> 9 & 0x1ffff;
                let count = arg & 511;
                let pio = word == 0x353a0010;
                let expected = if block {
                    if write { 0x353a0023 } else { 0x353a0033 }
                } else if write {
                    0x353a0001
                } else {
                    0x353a0011
                };
                let quantity = if block {
                    count as usize * 512
                } else if count == 0 {
                    512
                } else {
                    count as usize
                };
                if function != 1
                    || increment
                    || pio && (write || block || address != 0x10 || count != 4)
                    || !pio && word != expected
                    || block
                        && (![3, 5].contains(&count)
                            || self.regs.get(4) != 512
                            || self.regs.get(6) as u32 != count
                            || self.module.block_size() != 512)
                    || !block && (count != 4 || self.regs.get(4) != 4 || self.regs.get(6) != 0)
                {
                    return Err("Unsupported Wi-Fi CMD53 grammar or blocks".into());
                }
                r.quantity = quantity;
                r.write = write;
                r.pio = pio;
                if pio {
                    if self.module.block_size() != 512 {
                        return Err("Wi-Fi PIO requires 512-byte function block".into());
                    }
                    r.operation = Some(self.module.vendor(address, false, &[], quantity)?);
                } else {
                    let address_dma = self.regs.word(0) as u64;
                    let physical = if (0x40000000..0x60000000).contains(&address_dma) {
                        address_dma & 0x3fffffff
                    } else {
                        address_dma
                    };
                    if address_dma & 31 != 0
                        || quantity > 2560
                        || physical + quantity as u64 > self.options.ram_bytes
                        || self.regs.get(0x28) & 0x18 != 0
                        || quantity > 4096 - physical as usize % 4096
                    {
                        return Err("Wi-Fi SDMA extent, alignment or boundary unsupported".into());
                    }
                    if write {
                        cx.ram_read(physical, &mut self.scratch[..quantity])?;
                    }
                    r.dma = Some(physical);
                    r.operation = Some(self.module.vendor(
                        address,
                        write,
                        &self.scratch[..if write { quantity } else { 0 }],
                        quantity,
                    )?);
                }
            }
            _ => {
                return Err(format!(
                    "Unsupported Wi-Fi SDIO command {command}/{word:#x}"
                ));
            }
        }
        Ok(r)
    }
    fn start(&mut self, cx: &mut Cx<'_>, word: u32) -> Result<(), String> {
        if self.request.is_some() || self.regs.get(0x30) != 0 || self.regs.get(0x32) != 0 {
            return Err("SDIO command requires idle host and ACKed status".into());
        }
        if self.clocks.read(cx, BASE + 0x2c, 2) & 7 != 7 || self.regs.get(0x2e) & 15 != 14 {
            return Err("SDIO command before clock/timeout setup".into());
        }
        let signal = if word >> 24 & 63 == 53 && word != 0x353a0010 {
            2
        } else {
            1
        };
        if self.regs.get(0x34) & signal == 0 {
            return Err("SDIO completion status disabled".into());
        }
        let mut r = self.plan(cx, word, self.regs.word(8))?;
        r.due = Some(
            cx.now()
                .checked_add(self.options.transfer_ticks)
                .ok_or("SDIO deadline overflow")?,
        );
        self.regs.put_word(12, word);
        self.request = Some(r);
        Ok(())
    }
    fn complete(&mut self, cx: &mut Cx<'_>) -> Result<(), String> {
        if self
            .request
            .as_ref()
            .is_none_or(|r| r.due.is_none_or(|due| due > cx.now()))
        {
            return Ok(());
        }
        let mut r = self.request.take().unwrap();
        r.due = None;
        if let Some(op) = r.operation.take() {
            if !r.write
                && let Some(address) = r.dma
            {
                cx.ram_write(address, &op.response.to_le_bytes())?;
            }
            if r.pio {
                r.operation = Some(op);
            } else {
                self.module.commit(op);
            }
        }
        self.state = r.state;
        self.response = [0; 16];
        self.response[..4].copy_from_slice(&r.response.to_le_bytes());
        if r.pio {
            self.request = Some(r);
            self.regs.set(0x30, self.regs.get(0x30) | 1);
        } else {
            if r.quantity != 0 {
                self.regs.set(6, 0);
            }
            self.regs.set(
                0x30,
                self.regs.get(0x30) | if r.quantity != 0 { 3 } else { 1 },
            );
        }
        self.signal(cx)
    }
    fn pio_read(&mut self, cx: &mut Cx<'_>) -> Result<u32, String> {
        let r = self.request.as_mut().ok_or("Wi-Fi PIO without request")?;
        if !r.pio
            || r.due.is_some()
            || r.pio_reads >= 128
            || self.regs.get(0x30) != if r.pio_reads == 0 { 0 } else { 2 }
            || self.regs.get(0x34) != 0xffff
            || self.regs.get(0x38) != 0
        {
            return Err("Wi-Fi PIO requires command ACK and polled data gates".into());
        }
        let value = if r.pio_reads == 0 {
            r.operation.as_ref().unwrap().response
        } else {
            0
        };
        r.pio_reads += 1;
        // A 512-byte FIFO drain returns four wire bytes and 127 empty-latch
        // words. This is an explicit unmeasured transport law.
        self.regs.set(0x30, self.regs.get(0x30) | 2);
        if r.pio_reads == 128 {
            let mut r = self.request.take().unwrap();
            self.module.commit(r.operation.take().unwrap());
            self.regs.set(6, 0);
        }
        self.signal(cx)?;
        Ok(value)
    }
    fn store(&mut self, cx: &mut Cx<'_>, o: usize, size: u8, value: u32) -> Result<(), String> {
        if o == 0x24 || (16..32).contains(&o) || o == 0x20 {
            return Err("SDIO response/data/present-state write unsupported".into());
        }
        if matches!(o, 0x30 | 0x32) {
            self.regs.set(o, self.regs.get(o) & !(value as u16));
            return self.signal(cx);
        }
        if o == 0x2e {
            if value & !0x70f != 0 {
                return Err("SDIO reset/timeout bits unsupported".into());
            }
            if value & 0x700 != 0 {
                self.request = None;
                self.regs.set(0x30, 0);
                self.regs.set(0x32, 0);
                self.response = [0; 16];
            }
            self.regs.set(o, value as u16 & 15);
            return self.signal(cx);
        }
        if o == 14 || o == 12 && size == 4 {
            return self.start(
                cx,
                if size == 4 {
                    value
                } else {
                    value << 16 | self.regs.get(12) as u32
                },
            );
        }
        if let Some(r) = &self.request {
            let [normal, error, signal, error_signal] = r.gates;
            let masked = normal & 0xfe3f & 0xffc4;
            let allowed = r.pio
                && r.due.is_none()
                && match o {
                    0x34 => [
                        0,
                        0x100,
                        0xffff,
                        normal,
                        normal & 0xfe3f,
                        masked,
                        masked | 0x100,
                    ]
                    .contains(&(value as u16)),
                    0x36 => [0, error].contains(&(value as u16)),
                    0x38 => [0, 0x100, signal].contains(&(value as u16)),
                    0x3a => [0, error_signal].contains(&(value as u16)),
                    _ => false,
                };
            if !allowed {
                return Err("SDIO configuration changed during command".into());
            }
        }
        if o == 0x28 && value & !0xfff != 0
            || o == 4 && value & 0x8000 != 0
            || o == 0x3e && value != 0x4000
            || o == 0x100 && value & !7 != 0
            || o == 0x104 && value != 0
        {
            return Err("Unsupported SDIO host configuration".into());
        }
        if size == 4 {
            self.regs.put_word(o, value);
        } else {
            self.regs.set(o, value as u16);
        }
        self.signal(cx)
    }
}
impl Device for SdioHost {
    fujimod_emulator_devices::checkpoint_fields!(module:WifiModule,clocks:Clocks,regs:Registers,response:[u8;16],state:State,request:Option<Request>,#[serde(with="fujimod_emulator_devices::checkpoint::array")] scratch:[u8;2560]);

    fn fingerprint(&self) -> Result<serde_json::Value, String> {
        Ok(
            serde_json::json!({"module":self.module.fingerprint(),"clocks":self.clocks.save(),"registers":self.regs.0.as_slice(),"response":self.response,"state":self.state,"request":self.request,"scratch":fujimod_emulator_devices::fingerprint::bytes(&self.scratch)}),
        )
    }

    fn name(&self) -> &'static str {
        "exr2-sdio"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        if address == BASE + 0x2c && size == 2 {
            return self.clocks.read(cx, address, size);
        }
        let o = address.wrapping_sub(BASE) as usize;
        if !Registers::valid(o, size) {
            cx.unmodeled();
            return 0;
        }
        self.advance(cx);
        if cx.refusal().is_some() {
            return 0;
        }
        if o == 0x20 {
            return match self.pio_read(cx) {
                Ok(v) => v as u64,
                Err(e) => {
                    cx.fail(e);
                    0
                }
            };
        }
        if o == 0x24 {
            return 0x30000
                | self.request.as_ref().map_or(0, |r| {
                    if r.pio && r.due.is_none() {
                        if r.pio_reads == 0 { 0x800 } else { 0 }
                    } else {
                        1
                    }
                });
        }
        if (16..32).contains(&o) {
            return u16::from_le_bytes(self.response[o - 16..o - 14].try_into().unwrap()) as u64;
        }
        if size == 4 {
            self.regs.word(o) as u64
        } else {
            self.regs.get(o) as u64
        }
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        if address == BASE + 0x2c && size == 2 {
            if self.request.is_some() {
                cx.fail("SDIO clock changed during command");
            } else {
                self.clocks.write(cx, address, size, value);
            }
            return;
        }
        let o = address.wrapping_sub(BASE) as usize;
        if !Registers::valid(o, size) || value > if size == 4 { u32::MAX as u64 } else { 65535 } {
            cx.unmodeled();
            return;
        }
        self.advance(cx);
        if cx.refusal().is_some() {
            return;
        }
        if let Err(e) = self.store(cx, o, size, value as u32) {
            cx.fail(e);
        }
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        self.clocks.advance(cx);
        if let Err(e) = self.complete(cx) {
            cx.fail(e);
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
    fn pollable(&self, address: u64, size: u8) -> bool {
        address != BASE + 0x20
            && (Registers::valid(address.wrapping_sub(BASE) as usize, size)
                || address == BASE + 0x2c && size == 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fujimod_emulator_parts_storage::wifi_module;
    use fujimod_emulator_devices::test_host::ram::{Effect, Ram};
    fn model(protocol: Protocol) -> SdioHost {
        SdioHost::new(
            Options {
                transfer_ticks: 10,
                clock_ticks: 2,
                rca: 0x1234,
                ram_bytes: 0x10000,
            },
            Wiring {
                module: WifiModule::new(wifi_module::Options {
                    protocol,
                    firmware_bytes: 64,
                })
                .unwrap(),
                irq: InterruptLine::Shared(79),
            },
        )
        .unwrap()
    }
    fn write(m: &mut SdioHost, h: &mut Ram, o: u64, v: u64) {
        let mut cx = h.cx();
        m.write(&mut cx, BASE + o, if o == 0 || o == 12 { 4 } else { 2 }, v);
        assert_eq!(cx.refusal(), None, "offset {o:x}");
    }
    fn setup(m: &mut SdioHost, h: &mut Ram) {
        write(m, h, 0x2c, 5);
        h.now = 2;
        m.advance(&mut h.cx());
        write(m, h, 0x2e, 14);
        write(m, h, 0x34, 0xffff);
        write(m, h, 0x38, 0xffff);
    }
    fn command(m: &mut SdioHost, h: &mut Ram, word: u32, arg: u32) {
        write(m, h, 0x30, 0xffff);
        write(m, h, 8, (arg & 65535) as u64);
        write(m, h, 10, (arg >> 16) as u64);
        write(m, h, 12, word as u64);
    }
    fn due(m: &mut SdioHost, h: &mut Ram) {
        h.now = m.next_deadline(h.now).unwrap();
        let mut cx = h.cx();
        m.advance(&mut cx);
        assert_eq!(cx.refusal(), None);
    }
    #[test]
    fn enumeration_cccr_deadline_and_reset() {
        let mut m = model(Protocol::Enumeration);
        let mut h = Ram::new(0x10000);
        setup(&mut m, &mut h);
        for (word, arg, response) in [
            (0x05020000, 0, 0x10200000),
            (0x05020000, 0x200000, 0x90200000),
            (0x031a0000, 0, 0x12340000),
            (0x071b0000, 0x12340000, 0x900),
            (0x341a0010, 0, 0x22),
            (0x341a0000, 0x80000402, 0),
            (0x341a0010, 0x600, 2),
        ] {
            command(&mut m, &mut h, word, arg);
            h.now += 9;
            m.advance(&mut h.cx());
            assert!(m.request.is_some());
            due(&mut m, &mut h);
            assert_eq!(
                u32::from_le_bytes(m.response[..4].try_into().unwrap()),
                response
            );
            assert_eq!(h.effects.last(), Some(&Effect::Irq(79, true)));
        }
        command(&mut m, &mut h, 0x341a0010, 0);
        write(&mut m, &mut h, 0x2e, 0x70e);
        assert_eq!(m.next_deadline(h.now), None);
        assert_eq!(m.regs.get(0x30), 0);
        assert_eq!(m.response, [0; 16]);
        write(&mut m, &mut h, 8, 0);
        let mut cx = h.cx();
        m.write(&mut cx, BASE + 12, 4, 0x353a0011);
        assert!(cx.refusal().is_some());
        assert!(m.request.is_none());
    }
    #[test]
    fn bootstrap_dma_is_delayed_and_precedes_irq() {
        let mut m = model(Protocol::RomDownload);
        let mut h = Ram::new(0x10000);
        setup(&mut m, &mut h);
        m.state = State::Transfer;
        write(&mut m, &mut h, 0, 0x1000);
        write(&mut m, &mut h, 4, 4);
        h.bytes[0x1000..0x1004].copy_from_slice(&0x8000u32.to_le_bytes());
        command(&mut m, &mut h, 0x353a0001, 0x90000000 | (0x8c << 9) | 4);
        due(&mut m, &mut h);
        h.bytes[0x1000..0x1004].fill(0);
        command(&mut m, &mut h, 0x353a0001, 0x90000000 | (0x8c << 9) | 4);
        due(&mut m, &mut h);
        command(&mut m, &mut h, 0x353a0011, 0x10000000 | (0x8c << 9) | 4);
        assert_eq!(&h.bytes[0x1000..0x1004], &[0; 4]);
        due(&mut m, &mut h);
        assert_eq!(&h.bytes[0x1000..0x1004], &0x800u32.to_le_bytes());
        assert_eq!(
            &h.effects[h.effects.len() - 2..],
            &[Effect::Write(0x1000, 4), Effect::Irq(79, true)]
        );
    }
}

//! X-Processor 5 register-level peripherals, shared by execution engines.
pub mod adc;
pub mod board;
pub mod clocks;
pub mod lz;
pub mod display;
pub mod dma_registers;
pub mod flash_storage;
pub mod flash_storage_controller;
pub mod flash_storage_dma;
pub mod gates;
pub mod general_dma;
pub mod gpio;
pub mod i2c;
pub mod imaging;
pub mod input_capture;
pub mod mailbox;
pub mod osd;
pub mod output_link;
pub mod panel_clock;
pub mod panel_serial;
pub mod renderer;
pub mod serial_sink;
pub mod spi;
pub mod temperature;
pub mod timer;

#[cfg(test)]
pub(crate) mod tests {
    use crate::{Host, Record};

    #[derive(Default)]
    pub struct TestHost {
        pub now: u64,
        pub events: Vec<(usize, u64, u64)>,
        pub irqs: Vec<(u32, bool)>,
        pub core_irqs: Vec<(usize, u32, bool)>,
        pub ram: Vec<u8>,
        pub irq: bool,
        pub ram_base: u64,
        pub resets: Vec<(usize, bool)>,
    }
    impl Host for TestHost {
        fn now(&self) -> u64 {
            self.now
        }
        fn schedule(&mut self, device: usize, at: u64, token: u64) {
            self.events.push((device, at, token));
        }
        fn set_irq(&mut self, line: u32, level: bool) {
            self.irqs.push((line, level));
            self.irq = level;
        }
        fn raise_irq(&mut self, _: usize, _: u32) -> Result<(), String> {
            Err("no per-core interrupt controller".into())
        }
        fn set_core_irq(&mut self, cpu: usize, line: u32, high: bool) -> Result<(), String> {
            self.core_irqs.push((cpu, line, high));
            Ok(())
        }
        fn reset_core(&mut self, cpu: usize, asserted: bool) -> Result<(), String> {
            self.resets.push((cpu, asserted));
            Ok(())
        }
        fn ram_read(&self, address: u64, out: &mut [u8]) -> Result<(), String> {
            let start = address.checked_sub(self.ram_base).ok_or("below RAM")? as usize;
            out.copy_from_slice(
                self.ram
                    .get(start..start + out.len())
                    .ok_or("outside RAM")?,
            );
            Ok(())
        }
        fn ram_write(&mut self, address: u64, bytes: &[u8]) -> Result<(), String> {
            let start = address.checked_sub(self.ram_base).ok_or("below RAM")? as usize;
            self.ram
                .get_mut(start..start + bytes.len())
                .ok_or("outside RAM")?
                .copy_from_slice(bytes);
            Ok(())
        }
        fn record(&mut self, _: Record) {}
    }
    impl TestHost {
        pub fn bytes(&self, address: u64, size: usize) -> Result<Vec<u8>, String> {
            let mut bytes = vec![0; size];
            self.ram_read(address, &mut bytes)?;
            Ok(bytes)
        }
    }

    /// Strict, CPU-free driver for exercising the shared models themselves.
    #[derive(Default)]
    pub struct TestBus {
        windows: Vec<(u64, u64, Box<dyn crate::Device>)>,
    }
    impl TestBus {
        pub fn register(
            &mut self,
            base: u64,
            size: u64,
            device: Box<dyn crate::Device>,
        ) -> Result<usize, String> {
            let end = base
                .checked_add(size)
                .filter(|_| size != 0)
                .ok_or("invalid test window")?;
            if self
                .windows
                .iter()
                .any(|(b, n, _)| base < b + n && *b < end)
            {
                return Err("overlapping test windows".into());
            }
            let index = self.windows.len();
            self.windows.push((base, size, device));
            Ok(index)
        }
        fn index(&self, address: u64, size: u8) -> Result<usize, String> {
            self.windows
                .iter()
                .position(|(base, len, _)| {
                    address >= *base
                        && address
                            .checked_add(size as u64)
                            .is_some_and(|end| end <= base + len)
                })
                .ok_or("unmapped test access".into())
        }
        pub fn read(
            &mut self,
            cx: &mut crate::Cx<'_>,
            address: u64,
            size: u8,
        ) -> Result<u64, String> {
            let index = self.index(address, size)?;
            cx.device = index;
            let result = self.windows[index].2.read(cx, address, size);
            cx.take_refusal().map_or(Ok(result), Err)
        }
        pub fn write(
            &mut self,
            cx: &mut crate::Cx<'_>,
            address: u64,
            size: u8,
            value: u64,
        ) -> Result<(), String> {
            let index = self.index(address, size)?;
            cx.device = index;
            self.windows[index].2.write(cx, address, size, value);
            cx.take_refusal().map_or(Ok(()), Err)
        }
        pub fn event(
            &mut self,
            cx: &mut crate::Cx<'_>,
            device: usize,
            token: u64,
        ) -> Result<(), String> {
            cx.device = device;
            self.windows[device].2.event(cx, token);
            cx.take_refusal().map_or(Ok(()), Err)
        }
        pub fn save(&self) -> serde_json::Value {
            serde_json::Value::Array(self.windows.iter().map(|(_, _, d)| d.save()).collect())
        }
        pub fn restore(&mut self, state: &serde_json::Value) -> Result<(), String> {
            let states = state
                .as_array()
                .filter(|s| s.len() == self.windows.len())
                .ok_or("invalid test checkpoint")?;
            for ((_, _, device), state) in self.windows.iter_mut().zip(states) {
                device.restore(state)?;
            }
            Ok(())
        }
    }
}

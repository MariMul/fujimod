//! Runtime connection for the vector engine: physical DMA, routed IRQs and
//! exact completion deadlines. The peripheral has no processor capability.
use super::{CommandProcessor, IRQ, Inputs, Profile};
use crate::{Action, Cx, Device, Unsupported};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub ram_bytes: u64,
    pub max_fill_bytes: u64,
    pub max_copy_bytes: u64,
    pub max_command_capture_bytes: u64,
    pub max_clear_bytes: u64,
    pub bytes_per_tick: u64,
    pub copy_profile: Option<Profile>,
    pub clear_profile: Option<Profile>,
}
pub struct Wiring {
    pub controllers: Vec<(usize, crate::exr2::irq::Inputs)>,
}
pub struct VectorGpu {
    state: CommandProcessor,
    controllers: [Option<crate::exr2::irq::Inputs>; 2],
    due: BTreeMap<u64, u64>,
}
impl VectorGpu {
    pub fn new(options: Options, wiring: Wiring) -> Result<Self, String> {
        let mut controllers: [Option<crate::exr2::irq::Inputs>; 2] = Default::default();
        for (core, inputs) in wiring.controllers {
            if core >= 2 || controllers[core].is_some() {
                return Err("invalid vector engine interrupt wiring".into());
            }
            controllers[core] = Some(inputs);
        }
        let capture_limit = if options.clear_profile == Some(Profile::GpuUiProfile) {
            0x80000
        } else {
            0x10000
        };
        if controllers.iter().any(Option::is_none)
            || options.ram_bytes == 0
            || options.ram_bytes > 0x40000000
            || [
                options.max_fill_bytes,
                options.max_copy_bytes,
                options.max_clear_bytes,
            ]
            .iter()
            .any(|&n| n == 0 || n > options.ram_bytes)
            || options.bytes_per_tick == 0
            || !(8..=capture_limit).contains(&options.max_command_capture_bytes)
            || options.max_command_capture_bytes & 7 != 0
            || !matches!(options.copy_profile, None | Some(Profile::GpuCopyProfile))
            || !matches!(
                options.clear_profile,
                None | Some(Profile::GpuClearProfile | Profile::GpuUiProfile)
            )
        {
            return Err("invalid vector engine configuration, bounds or IRQ connections".into());
        }
        Ok(Self {
            state: CommandProcessor {
                ram_bytes: options.ram_bytes,
                max_fill_bytes: options.max_fill_bytes,
                max_copy_bytes: options.max_copy_bytes,
                max_command_capture_bytes: options.max_command_capture_bytes,
                max_clear_bytes: options.max_clear_bytes,
                bytes_per_tick: options.bytes_per_tick,
                copy_profile: options.copy_profile,
                clear_profile: options.clear_profile,
                ..CommandProcessor::default()
            },
            controllers,
            due: BTreeMap::new(),
        })
    }
    fn actions(&mut self, cx: &mut Cx<'_>, actions: Vec<Action>) -> Result<(), String> {
        for action in actions {
            match action {
                Action::Log(record) | Action::Record(record) => cx.record(record),
                Action::QueueIrq(core, line) => cx.raise_irq(core.into(), line.into())?,
                Action::Schedule { delay, token } => {
                    let at = cx
                        .now()
                        .checked_add(delay)
                        .ok_or("vector engine completion time overflow")?;
                    if delay == 0 || self.due.insert(token, at).is_some() {
                        return Err("invalid vector engine completion token".into());
                    }
                }
            }
        }
        Ok(())
    }
    fn settle(&mut self, cx: &mut Cx<'_>) -> Result<(), String> {
        let mut ready: Vec<_> = self
            .due
            .iter()
            .filter(|(_, at)| **at <= cx.now())
            .map(|(&token, &at)| (at, token))
            .collect();
        ready.sort_unstable();
        for (_, token) in ready {
            let (_, outputs) = self.state.output(token).map_err(|e| e.0)?;
            for (address, bytes) in outputs {
                cx.ram_write(address, &bytes)?;
            }
            let mut actions = Vec::new();
            self.state
                .complete(token, cx.now(), &mut actions)
                .map_err(|e| e.0)?;
            self.due.remove(&token);
            self.actions(cx, actions)?;
        }
        Ok(())
    }
}
struct Connected<'a, 'b> {
    cx: &'a mut Cx<'b>,
    controllers: &'a [Option<crate::exr2::irq::Inputs>; 2],
    dma_failure: Option<String>,
}
impl Inputs for Connected<'_, '_> {
    fn has_reader(&self) -> bool {
        true
    }
    fn has_irq(&self) -> bool {
        true
    }
    fn routed(&self) -> bool {
        true
    }
    fn read(&mut self, _cpu: i64, address: u64, length: u64) -> Result<Vec<u8>, String> {
        let size =
            usize::try_from(length).map_err(|_| "vector DMA length exceeds host address space")?;
        match self.cx.ram_bytes(address, size) {
            Ok(bytes) => Ok(bytes),
            Err(error) => {
                self.dma_failure = Some(error.clone());
                Err(error)
            }
        }
    }
    fn targets(&mut self) -> crate::record::Result<(usize, i64)> {
        let mut count = 0;
        let mut selected = -1;
        for (core, pins) in self.controllers.iter().enumerate() {
            if pins
                .as_ref()
                .is_some_and(|pins| pins.enabled() & (1 << IRQ) != 0)
            {
                selected = core as i64;
                count += 1;
            }
        }
        Ok((count, selected))
    }
}
impl Device for VectorGpu {
    crate::checkpoint_fields!(state: CommandProcessor, due: BTreeMap<u64,u64>);
    fn fingerprint(&self) -> std::result::Result<serde_json::Value, String> {
        Ok(serde_json::json!({"engine":self.state,"deadlines":self.due}))
    }

    fn name(&self) -> &'static str {
        "vector-gpu"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        if let Err(error) = self.settle(cx) {
            cx.fail(error);
            return 0;
        }
        match self.state.read(address, size.into()) {
            Ok(value) => value.into(),
            Err(error) => {
                cx.unsupported(error.0);
                0
            }
        }
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        if let Err(error) = self.settle(cx) {
            cx.fail(error);
            return;
        }
        let Ok(value) = u32::try_from(value) else {
            cx.unsupported("vector GPU word exceeds 32 bits");
            return;
        };
        if size != 4 {
            cx.unsupported("vector GPU requires aligned word accesses");
            return;
        }
        let cpu = cx.cpu as i64;
        let mut host = Connected {
            cx,
            controllers: &self.controllers,
            dma_failure: None,
        };
        let mut actions = Vec::new();
        let result = self
            .state
            .write_native(cpu, address, value, &mut host, &mut actions);
        let dma_failure = host.dma_failure.take();
        if let Some(error) = dma_failure {
            cx.fail(error);
            return;
        }
        // A rejected hardware command does not acquire a completion or DMA.
        // Preserve any diagnostic records emitted before the refusal.
        if let Err(error) = self.actions(cx, actions) {
            cx.fail(error);
            return;
        }
        if let Err(Unsupported(reason)) = result {
            cx.unsupported(reason);
        }
    }
    fn pollable(&self, address: u64, size: u8) -> bool {
        self.state.read(address, size.into()).is_ok()
    }
    fn next_deadline(&self, _now: u64) -> Option<u64> {
        self.due.values().copied().min()
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        if self
            .next_deadline(cx.now())
            .is_some_and(|at| at <= cx.now())
        {
            if let Err(error) = self.settle(cx) {
                cx.fail(error);
            }
        }
    }
    fn save(&self) -> serde_json::Value {
        // Host presentation samples this on every continuation. Pending DMA
        // pixels belong to the exact typed checkpoint and fingerprint, not
        // the diagnostic JSON returned to a viewer.
        let s = &self.state;
        serde_json::json!({"engine":{
            "regs":s.regs,"path":s.path,"fill":s.fill,"written":s.written,
            "clear_ack_offsets":s.clear_ack_offsets,"command_pending":s.command_pending,
            "busy":s.busy,"jobs_started":s.jobs_started,"jobs_completed":s.jobs_completed,
            "bytes_completed":s.bytes_completed,"irqs_raised":s.irqs_raised,
            "acknowledgments":s.acknowledgments,"last_job":s.last_job,
            "ram_bytes":s.ram_bytes,"max_fill_bytes":s.max_fill_bytes,
            "max_copy_bytes":s.max_copy_bytes,"max_clear_bytes":s.max_clear_bytes,
            "max_command_capture_bytes":s.max_command_capture_bytes,
            "bytes_per_tick":s.bytes_per_tick,"copy_profile":s.copy_profile,
            "clear_profile":s.clear_profile,"pending_completions":s.completions.len(),
            "next_token":s.next_token,"next_job":s.next_job
        },"deadlines":self.due})
    }
}

#[cfg(test)]
mod tests;

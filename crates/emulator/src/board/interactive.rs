//! Concurrent CPU windows with owner-only device and memory barriers.
use super::*;
use fujimod_emulator_cpu::{RunStatus, RunToken};

#[cfg(test)]
mod tests;

pub(super) struct Epoch {
    progress: Vec<u64>,
    advanced: u64,
}
impl Epoch {
    fn new(cores: usize) -> Self {
        Self {
            progress: vec![0; cores],
            advanced: 0,
        }
    }
    pub(super) fn advance(&mut self, core: usize, progress: u64) -> u64 {
        self.progress[core] = progress;
        let furthest = self.progress.iter().copied().max().unwrap_or(0);
        let delta = furthest.saturating_sub(self.advanced);
        self.advanced = furthest;
        delta
    }
}

struct Entry {
    core: usize,
    pc: u64,
    token: RunToken,
    done: bool,
    pending: bool,
    exit: Option<Exit>,
}

impl Board {
    /// Every return joins started runs, including failures. A pending callback
    /// is a parked CPU, not permission to touch devices while a peer executes.
    pub(super) fn interactive_window(
        &mut self,
        cores: &[usize],
        budget: usize,
        quantum: usize,
    ) -> Result<(), String> {
        let mut prepared = Vec::with_capacity(cores.len());
        for &core in cores {
            let cpu = &self.cpus[core];
            cpu.set_loop_window(false)?;
            let pc = scalar(&**cpu, Register::Pc)?;
            let begin = if scalar(&**cpu, Register::Pstate)? & 0x20 != 0 {
                pc | 1
            } else {
                pc
            };
            prepared.push((
                core,
                pc,
                Run {
                    begin,
                    until: None,
                    watchdog_us: 0,
                    instructions: Some(budget),
                },
            ));
        }
        let mut entries = Vec::with_capacity(cores.len());
        let mut error = None;
        {
            let mut state = self.state.borrow_mut();
            state.parallel = Some(Epoch::new(self.cpus.len()));
            state.code_run = None;
            for &core in cores {
                state.accounted[core] = 0;
            }
        }
        for (core, pc, request) in prepared {
            let cpu = &self.cpus[core];
            match cpu.start_run(request) {
                Ok(token) => entries.push(Entry {
                    core,
                    pc,
                    token,
                    done: false,
                    pending: false,
                    exit: None,
                }),
                Err(e) => {
                    error = Some(e.to_string());
                    break;
                }
            }
        }
        let mut barrier = false;
        loop {
            let generation = self.run_wakeup.generation();
            let mut running = false;
            for entry in entries.iter_mut().filter(|e| !e.done) {
                match self.cpus[entry.core].poll_run(entry.token) {
                    Ok(RunStatus::Running) => {
                        entry.pending = false;
                        running = true;
                    }
                    Ok(RunStatus::CallbackPending) => entry.pending = true,
                    Ok(RunStatus::Complete(result)) => {
                        entry.done = true;
                        entry.pending = false;
                        match result {
                            Ok(exit) => entry.exit = Some(exit),
                            Err(e) => {
                                error.get_or_insert_with(|| e.to_string());
                            }
                        }
                    }
                    Err(e) => {
                        entry.done = true;
                        entry.pending = false;
                        error.get_or_insert_with(|| e.to_string());
                    }
                }
            }
            if !barrier && (error.is_some() || entries.iter().any(|e| e.done || e.pending)) {
                barrier = true;
                for entry in entries.iter().filter(|e| !e.done) {
                    if let Err(e) = self.cpus[entry.core].request_pause(entry.token) {
                        error.get_or_insert_with(|| e.to_string());
                    }
                }
            }
            if entries.iter().all(|e| e.done) {
                break;
            }
            if barrier && !running {
                // All assembly execution is stopped. Account the whole cluster
                // before a callback can settle timers or publish DMA.
                let mut state = self.state.borrow_mut();
                for entry in &entries {
                    if let Err(e) = state.sync(&*self.cpus[entry.core], entry.core) {
                        error.get_or_insert(e);
                    }
                }
                if let Some(entry) = entries.iter().find(|e| e.pending) {
                    match schedule::Slice::new(
                        entry.core,
                        state.instructions[entry.core],
                        quantum,
                        quantum,
                    ) {
                        Ok(slice) => state.active_slice = Some(slice),
                        Err(e) => {
                            error.get_or_insert(e);
                        }
                    }
                    drop(state);
                    let result = if error.is_some() {
                        self.cpus[entry.core].cancel_pending_callback(entry.token)
                    } else {
                        self.cpus[entry.core].service_pending_callback(entry.token)
                    };
                    if let Err(e) = result {
                        error.get_or_insert_with(|| e.to_string());
                    }
                    // The response allows this CPU to retire its access and
                    // honor the sticky pause before another callback is served.
                    continue;
                }
            }
            self.run_wakeup.wait_for_change(generation);
        }
        let mut state = self.state.borrow_mut();
        for entry in &entries {
            if let Err(e) = state.sync(&*self.cpus[entry.core], entry.core) {
                error.get_or_insert(e);
            }
        }
        state.active_slice = None;
        state.parallel = None;
        for entry in entries {
            if let Some(exit) = entry.exit {
                if let Err(e) =
                    state.finish_exit(&*self.cpus[entry.core], entry.core, entry.pc, exit)
                {
                    error.get_or_insert(e);
                }
            }
        }
        if let Err(e) = state.apply_memory(&self.cpus) {
            error.get_or_insert(e);
        }
        if let Some(e) = state.error.take() {
            error.get_or_insert(e);
        }
        self.next_core.set((cores[0] + 1) % self.cpus.len());
        self.slice_left.set(0);
        error.map_or(Ok(()), Err)
    }
}

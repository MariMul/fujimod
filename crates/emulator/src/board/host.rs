//! Stopped-board capabilities for host sessions. No CPU/device callback access.
use super::Board;
use fujimod_emulator_devices::Device;

impl Board {
    pub fn device_states(&self, kind: &str) -> Result<serde_json::Value, String> {
        let state = self.state.try_borrow().map_err(|_| "board is executing")?;
        Ok(serde_json::json!(state.bus.device_states(kind)))
    }

    /// CPU/IRQ/scheduler observation without serializing device DMA payloads.
    pub fn observation(&self) -> Result<serde_json::Value, String> {
        let s = self.state.try_borrow().map_err(|_| "board is executing")?;
        Ok(serde_json::json!({
            "execution_mode":s.bus.mode().name(), "counts_as_boot":s.bus.mode()==super::bus::Mode::Strict,
            "timeline":s.timeline.save(), "instructions":s.instructions,
            "released":s.released, "waiting":s.waiting, "waiting_event":s.waiting_event,
            "events":s.events, "next_core":self.next_core.get(), "slice_left":self.slice_left.get(),
            "quantum":self.quantum.get(), "failure":s.error,
            "interrupt_controllers":s.bus.device_states("exr2-interrupt-controller")
        }))
    }

    pub fn inspect_device<T: Device, R>(
        &self,
        id: usize,
        observe: impl FnOnce(&T) -> Result<R, String>,
    ) -> Result<R, String> {
        let state = self.state.try_borrow().map_err(|_| "board is executing")?;
        observe(state.bus.device::<T>(id)?)
    }

    pub fn edit_device<T: Device, R>(
        &mut self,
        id: usize,
        edit: impl FnOnce(&mut T) -> Result<R, String>,
    ) -> Result<R, String> {
        let result = {
            let mut state = self
                .state
                .try_borrow_mut()
                .map_err(|_| "board is executing")?;
            edit(state.bus.device_mut::<T>(id)?)
        };
        // Even a refused edit may have changed an input. Re-read only this
        // device and its declared dependents before execution resumes.
        self.inputs_changed(id)?;
        result
    }
    /// Apply an explicit external device input while processors are stopped.
    /// Its context carries guest time, physical DMA and IRQ wires, never CPUs.
    pub fn device_input<T: Device, R>(
        &mut self,
        id: usize,
        input: impl FnOnce(&mut T, &mut fujimod_emulator_devices::Cx<'_>) -> Result<R, String>,
    ) -> Result<R, String> {
        let result = {
            let mut state = self
                .state
                .try_borrow_mut()
                .map_err(|_| "board is executing")?;
            state.with_host(&self.cpus, |bus, host| {
                let device = bus.device_mut::<T>(id)?;
                let mut cx = fujimod_emulator_devices::Cx::new(host, usize::MAX, 0, id);
                let result = input(device, &mut cx);
                // Return the operation's refusal inside the host result so
                // already emitted IRQ effects still wake their controllers.
                Ok(match cx.take_refusal() {
                    Some(error) => Err(error),
                    None => result,
                })
            })
        }
        .and_then(|result| result);
        let refresh = self.inputs_changed(id);
        result.and_then(|value| refresh.map(|()| value))
    }
}

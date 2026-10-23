//! Persistent NAND is external to the powered controller and its pending work.
use super::Session;
use fujimod_emulator_exr2_storage::nand::NandController;
use fujimod_emulator_devices::parts::nand::Nand;
impl Session {
    pub fn nand_state(&self, name: &str) -> Result<serde_json::Value, String> {
        self.board
            .inspect_device::<NandController, _>(self.node(name)?, |controller| {
                Ok(fujimod_emulator_devices::Device::save(controller))
            })
    }

    pub fn nand_medium(&self, name: &str) -> Result<Nand, String> {
        self.board
            .inspect_device::<NandController, _>(self.node(name)?, |controller| {
                Ok(controller.medium().clone())
            })
    }
    pub fn retain_nand(&mut self, name: &str, medium: Nand) -> Result<(), String> {
        if self.board.instruction_counts().iter().any(|n| *n != 0) {
            return Err("retaining NAND requires an unexecuted board".into());
        }
        self.board
            .edit_device::<NandController, _>(self.node(name)?, |controller| {
                controller.retain(medium)
            })
    }
}

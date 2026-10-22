//! Host FAT access to the very medium owned by the native SD controller.
use super::Session;
use fujimod_emulator_devices::{
    exr2::sdhc::{DataFault, SdHost},
    parts::sd_card::Card,
    storage::fat,
};
use serde_json::{Value, json};

impl Session {
    pub fn card_state(&self, name: &str) -> Result<Value, String> {
        let contacts = self.contacts.get(name).ok_or("no SD host with that name")?;
        self.board.inspect_device::<SdHost, _>(self.node(name)?, |host| Ok(json!({
            "supported": true, "present": contacts.present(), "protected": contacts.protected(), "busy": host.busy(),
            "revision": host.card().revision(), "capacity_bytes": host.card().sectors() as u64 * 512,
            "data_faults": host.data_faults(), "fault_history": host.fault_history(),
        })))
    }
    /// Declare a data CRC failure on the card's data lines for later transfers.
    pub fn card_data_fault(&mut self, name: &str, fault: DataFault) -> Result<(), String> {
        self.board
            .edit_device::<SdHost, _>(self.node(name)?, |host| host.inject_data_fault(fault))
    }
    pub fn card_clear_data_faults(&mut self, name: &str) -> Result<(), String> {
        self.board
            .edit_device::<SdHost, _>(self.node(name)?, |host| host.clear_data_faults())
    }
    pub fn card_tree(&self, name: &str, revision: Option<u64>) -> Result<Value, String> {
        let entries = self
            .board
            .inspect_device::<SdHost, _>(self.node(name)?, |host| host.files(revision)?.tree())?;
        let mut state = self.card_state(name)?;
        state["entries"] = serde_json::to_value(entries).map_err(|e| e.to_string())?;
        Ok(state)
    }
    pub fn card_read(
        &self,
        name: &str,
        path: &str,
        revision: Option<u64>,
        max_bytes: usize,
    ) -> Result<Vec<u8>, String> {
        if !(1..=fat::MAX_BYTES).contains(&max_bytes) {
            return Err("card reads require a bound of 1..64 MiB".into());
        }
        self.board
            .inspect_device::<SdHost, _>(self.node(name)?, |host| {
                host.files(revision)?.read(path, max_bytes)
            })
    }
    fn admit_card_contacts(
        &self,
        name: &str,
        present: bool,
        protected: bool,
    ) -> Result<(), String> {
        self.node(name)?;
        self.contacts.get(name).ok_or("no SD host with that name")?;
        if let Some(pins) = self.resources.card_pins.get(name) {
            pins.pads.admit_requests(&[
                (pins.detect, present == pins.present_high),
                (pins.protect, protected == pins.protected_high),
            ])?;
        }
        Ok(())
    }
    pub fn card_contacts(
        &mut self,
        name: &str,
        present: bool,
        protected: bool,
    ) -> Result<(), String> {
        let contacts = self.contacts.get(name).ok_or("no SD host with that name")?;
        if let Some(pins) = self.resources.card_pins.get(name) {
            pins.pads.request_many(&[
                (pins.detect, present == pins.present_high),
                (pins.protect, protected == pins.protected_high),
            ])?;
            self.board.inputs_changed(pins.gpio)?;
        }
        contacts.set(present, protected);
        self.board.inputs_changed(self.node(name)?)
    }
    pub fn card_format(
        &mut self,
        name: &str,
        options: &fat::Options,
        files: &[(String, Vec<u8>)],
    ) -> Result<(), String> {
        // Preflight activity, presence, geometry and all files before replacing
        // media. Guest formatting still happens exclusively via sector writes.
        let id = self.node(name)?;
        self.board.inspect_device::<SdHost, _>(id, |host| {
            if host.busy() {
                return Err("SD medium is busy".into());
            }
            if host.card().sectors() != options.sectors {
                return Err("formatted card capacity differs".into());
            }
            Ok(())
        })?;
        let card = fat::format(options, files)?;
        self.board
            .edit_device::<SdHost, _>(id, |host| host.replace_card(card).map(|_| ()))
    }
    pub fn card_medium(&self, name: &str) -> Result<Card, String> {
        self.board
            .inspect_device::<SdHost, _>(self.node(name)?, |host| Ok(host.card().clone()))
    }
    pub fn initialize_card_revision(&mut self, name: &str, revision: u64) -> Result<(), String> {
        if self.board.instruction_counts().iter().any(|n| *n != 0) {
            return Err("initial card revision requires an unexecuted board".into());
        }
        self.board
            .edit_device::<SdHost, _>(self.node(name)?, |host| host.initialize_revision(revision))
    }
    pub fn retain_card(
        &mut self,
        name: &str,
        card: Card,
        present: bool,
        protected: bool,
    ) -> Result<(), String> {
        if self.board.instruction_counts().iter().any(|n| *n != 0) {
            return Err("retaining external media requires an unexecuted board".into());
        }
        self.admit_card_contacts(name, present, protected)?;
        self.board
            .edit_device::<SdHost, _>(self.node(name)?, |host| host.retain_card(card))?;
        self.card_contacts(name, present, protected)
    }
    pub fn insert_card(
        &mut self,
        name: &str,
        card: Card,
        present: bool,
        protected: bool,
    ) -> Result<Card, String> {
        self.admit_card_contacts(name, present, protected)?;
        let old = self
            .board
            .edit_device::<SdHost, _>(self.node(name)?, |host| host.replace_card(card))?;
        self.card_contacts(name, present, protected)?;
        Ok(old)
    }
}

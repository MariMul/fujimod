//! Register-level USB core, PHY and optional endpoint engine on one physical
//! controller. Endpoint ownership is explicit and precedes startup controls.
use super::UsbStartup;
use crate::{Action, Cx, Device, Value, parts::usb_endpoint, signals::Level};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub endpoints: Option<usb_endpoint::Options>,
}
pub struct Wiring {
    pub cable: Level,
    pub endpoints: Option<usb_endpoint::Wiring>,
}
pub struct UsbController {
    startup: UsbStartup,
    cable: Level,
    endpoint: Option<usb_endpoint::Endpoint>,
}
impl UsbController {
    pub fn new(options: Options, wiring: Wiring) -> Result<Self, String> {
        let endpoint = match (options.endpoints, wiring.endpoints) {
            (Some(options), Some(wiring)) => Some(usb_endpoint::Endpoint::new(options, wiring)?),
            (None, None) => None,
            _ => return Err("USB endpoint options and wiring must be selected together".into()),
        };
        Ok(Self {
            startup: UsbStartup::default(),
            cable: wiring.cable,
            endpoint,
        })
    }
    pub fn cable_connected(&self) -> bool {
        self.cable.high()
    }
    pub fn endpoint(&self) -> Result<&usb_endpoint::Endpoint, String> {
        self.endpoint
            .as_ref()
            .ok_or_else(|| "USB endpoints are not connected".into())
    }
    pub fn endpoint_mut(&mut self) -> Result<&mut usb_endpoint::Endpoint, String> {
        self.endpoint
            .as_mut()
            .ok_or_else(|| "USB endpoints are not connected".into())
    }
    fn endpoint_owns(&self, address: u64, size: u8) -> bool {
        self.endpoint.is_some() && usb_endpoint::Endpoint::offset(address, size).is_some()
    }
}
impl Device for UsbController {
    fn checkpoint(&self) -> Result<crate::checkpoint::Checkpoint, String> {
        let children = self
            .endpoint
            .as_ref()
            .map(Device::checkpoint)
            .transpose()?
            .into_iter()
            .collect();
        Ok(
            crate::checkpoint::Checkpoint::encode(
                self.name(),
                &(&self.startup, self.cable.high()),
            )?
            .with_children(children),
        )
    }
    fn validate_checkpoint(&self, saved: &crate::checkpoint::Checkpoint) -> Result<(), String> {
        let _: (UsbStartup, bool) = saved.decode(self.name())?;
        if saved.children().len() != usize::from(self.endpoint.is_some()) {
            return Err("USB snapshot endpoint wiring differs".into());
        }
        if let Some(endpoint) = &self.endpoint {
            endpoint.validate_checkpoint(&saved.children()[0])?;
        }
        Ok(())
    }
    fn restore_checkpoint(&mut self, saved: &crate::checkpoint::Checkpoint) -> Result<(), String> {
        self.validate_checkpoint(saved)?;
        let (startup, high) = saved.decode(self.name())?;
        if let Some(endpoint) = &mut self.endpoint {
            endpoint.restore_checkpoint(&saved.children()[0])?;
        }
        self.startup = startup;
        self.cable.drive(high);
        Ok(())
    }

    fn fingerprint(&self) -> Result<serde_json::Value, String> {
        let endpoints = self
            .endpoint
            .as_ref()
            .map(Device::fingerprint)
            .transpose()?;
        Ok(
            serde_json::json!({"startup":self.startup,"cable":self.cable.high(),"endpoints":endpoints}),
        )
    }
    fn name(&self) -> &'static str {
        "usb-controller"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        if self.endpoint_owns(address, size) {
            return self.endpoint.as_mut().unwrap().read(cx, address, size);
        }
        match self.startup.read(address, size.into(), self.cable.high()) {
            Ok(value) => value.into(),
            Err(error) => {
                cx.unsupported(error.0);
                0
            }
        }
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        if self.endpoint_owns(address, size) {
            self.endpoint
                .as_mut()
                .unwrap()
                .write(cx, address, size, value);
            return;
        }
        let mut actions = Vec::new();
        match self.startup.write(
            Value::int(cx.cpu as u64),
            address,
            size.into(),
            value,
            Value::int(size),
            &mut actions,
        ) {
            Ok(()) => {
                for action in actions {
                    match action {
                        Action::Log(record) | Action::Record(record) => cx.record(record),
                        _ => {
                            cx.fail("USB startup produced an unexpected asynchronous effect");
                            return;
                        }
                    }
                }
            }
            Err(error) => cx.unsupported(error.0),
        }
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        if let Some(endpoint) = &mut self.endpoint {
            endpoint.advance(cx);
        }
    }
    fn next_deadline(&self, now: u64) -> Option<u64> {
        self.endpoint.as_ref().and_then(|e| e.next_deadline(now))
    }
    fn pollable(&self, address: u64, size: u8) -> bool {
        if self.endpoint_owns(address, size) {
            return self.endpoint.as_ref().unwrap().pollable(address, size);
        }
        UsbStartup::readable(address, size.into())
    }
    fn save(&self) -> serde_json::Value {
        serde_json::json!({"startup":self.startup,"cable":self.cable.high(),"endpoints":self.endpoint.as_ref().map(Device::save)})
    }
}

#[cfg(test)]
mod tests;

//! Opaque external USB bus transactions on the native controller.
use super::Session;
use fujimod_emulator_devices::{Cx, parts::usb_endpoint::Endpoint};
use fujimod_emulator_exr2_io::usb::UsbController;
impl Session {
    fn usb_input<R>(
        &mut self,
        name: &str,
        input: impl FnOnce(&mut Endpoint, &mut Cx<'_>) -> Result<R, String>,
    ) -> Result<R, String> {
        self.board
            .device_input::<UsbController, _>(self.node(name)?, |controller, cx| {
                if !controller.cable_connected() {
                    return Err("USB bus cable is disconnected".into());
                }
                input(controller.endpoint_mut()?, cx)
            })
    }
    pub fn usb_reset(&mut self, name: &str) -> Result<(), String> {
        self.usb_input(name, |port, cx| port.host_reset(cx))
    }
    pub fn usb_setup(&mut self, name: &str, packet: &[u8]) -> Result<(), String> {
        self.usb_input(name, |port, cx| port.host_setup(cx, packet))
    }
    pub fn usb_out(&mut self, name: &str, endpoint: u8, packet: &[u8]) -> Result<(), String> {
        self.usb_input(name, |port, cx| port.host_out(cx, endpoint, packet))
    }
    pub fn usb_in(&mut self, name: &str, endpoint: u8, size: usize) -> Result<Vec<u8>, String> {
        self.usb_input(name, |port, cx| port.host_in(cx, endpoint, size))
    }
    pub fn usb_in_token(&mut self, name: &str, endpoint: u8) -> Result<(), String> {
        self.usb_input(name, |port, cx| port.host_in_token(cx, endpoint))
    }
    pub fn usb_status(&self, name: &str) -> Result<serde_json::Value, String> {
        self.board.inspect_device::<UsbController,_>(self.node(name)?,|controller| Ok(serde_json::json!({"connected":controller.cable_connected(),"endpoint":controller.endpoint()?.bus_status()})))
    }
}

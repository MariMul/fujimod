//! Camera workflows on an open session. Firmware facts arrive as the selected
//! firmware's camera table; the workflow, not Python, decides each step.
use super::*;
use fujimod_defs::{camera::CameraDefinition, service::FirmwareIdentity};
use fujimod_workflows::service_mode;

fn definition(json: &str, id: String, sha256: &str) -> PyResult<CameraDefinition> {
    let identity =
        FirmwareIdentity::new(id, sha256).map_err(|e| PyValueError::new_err(e.to_string()))?;
    CameraDefinition::from_json(json, &identity).map_err(|e| PyValueError::new_err(e.to_string()))
}

#[pymethods]
impl AsyncSession {
    /// Confirm stock service mode for the selected firmware and arm its reset
    /// guard. Returns (guard_was_set, debug_flag_before, progress_lines).
    /// A failed check raises RuntimeError; USB failures raise UsbError.
    async fn arm_reset_guard(
        &self,
        camera_table: String,
        firmware_id: String,
        firmware_sha256: String,
    ) -> PyResult<(bool, u8, Vec<String>)> {
        let camera = definition(&camera_table, firmware_id, &firmware_sha256)?;
        let limits = service_mode::Limits {
            memory: self.limits.memory,
            configuration: self.limits.configuration,
        };
        let outcome = self
            .with(move |session| {
                Box::pin(async move {
                    let mut lines = Vec::new();
                    let result =
                        service_mode::arm_reset_guard(session, &camera, limits, &mut |line| {
                            lines.push(line.to_string())
                        })
                        .await;
                    match result {
                        Ok(armed) => Ok(Ok((armed, lines))),
                        Err(service_mode::ServiceModeError::Usb(error)) => Err(error),
                        Err(other) => Ok(Err(format!("{other} (after: {})", lines.join("; ")))),
                    }
                })
            })
            .await?;
        let (armed, lines) = outcome.map_err(PyRuntimeError::new_err)?;
        Ok((armed.guard_was_set, armed.debug_flag_before, lines))
    }
}

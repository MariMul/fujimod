//! Firmware-selected camera workflows share the session's ordinary lease.
use crate::{error::property, session::Operation, session::Session, *};
use fujimod_catalog::{camera::CameraDefinition, service::FirmwareIdentity};
use fujimod_workflows::{Limits, debug_mode, service_mode};

pub(crate) enum Action {
    ArmResetGuard,
    Enter,
    Leave,
}
pub(crate) struct Workflow {
    camera: CameraDefinition,
    limits: Limits,
    action: Action,
}
fn workflow_error(message: impl std::fmt::Display) -> JsValue {
    fail(ErrorKind::Workflow, &message.to_string())
}
impl Session {
    fn workflow(
        &self,
        camera_table: &str,
        firmware_id: String,
        firmware_sha256: &str,
        action: Action,
    ) -> Result<js_sys::Promise, JsValue> {
        let identity =
            FirmwareIdentity::new(firmware_id, firmware_sha256).map_err(workflow_error)?;
        let camera =
            CameraDefinition::from_json(camera_table, &identity).map_err(workflow_error)?;
        self.call(Operation::Workflow(Workflow {
            camera,
            limits: Limits {
                memory: self.limits.memory,
                configuration: self.limits.configuration,
            },
            action,
        }))
    }
}
#[wasm_bindgen]
impl Session {
    #[wasm_bindgen(js_name=armResetGuard, unchecked_return_type="Promise<ResetGuardResult>")]
    pub fn arm_reset_guard(
        &self,
        camera_table: &str,
        firmware_id: String,
        firmware_sha256: &str,
    ) -> Result<js_sys::Promise, JsValue> {
        self.workflow(
            camera_table,
            firmware_id,
            firmware_sha256,
            Action::ArmResetGuard,
        )
    }
    #[wasm_bindgen(js_name=enterDebugMode, unchecked_return_type="Promise<DebugModeResult>")]
    pub fn enter_debug_mode(
        &self,
        camera_table: &str,
        firmware_id: String,
        firmware_sha256: &str,
    ) -> Result<js_sys::Promise, JsValue> {
        self.workflow(camera_table, firmware_id, firmware_sha256, Action::Enter)
    }
    #[wasm_bindgen(js_name=leaveDebugMode, unchecked_return_type="Promise<DebugModeResult>")]
    pub fn leave_debug_mode(
        &self,
        camera_table: &str,
        firmware_id: String,
        firmware_sha256: &str,
    ) -> Result<js_sys::Promise, JsValue> {
        self.workflow(camera_table, firmware_id, firmware_sha256, Action::Leave)
    }
}
impl Workflow {
    pub(crate) async fn run(self, session: &mut usb::ff80::Session) -> Result<JsValue, JsValue> {
        let mut lines = Vec::new();
        let mut progress = |line: &str| lines.push(line.to_owned());
        let result = js_sys::Object::new();
        match self.action {
            Action::ArmResetGuard => {
                let armed = service_mode::arm_reset_guard(
                    session,
                    &self.camera,
                    self.limits,
                    &mut progress,
                )
                .await
                .map_err(|source| match source {
                    service_mode::ServiceModeError::Usb(source) => error(source),
                    other => workflow_error(format!("{other} (after: {})", lines.join("; "))),
                })?;
                property(&result, "guardWasSet", armed.guard_was_set);
                property(&result, "debugFlagBefore", armed.debug_flag_before);
            }
            Action::Enter | Action::Leave => {
                let outcome = match self.action {
                    Action::Enter => {
                        debug_mode::enter(session, &self.camera, self.limits, &mut progress).await
                    }
                    Action::Leave => {
                        debug_mode::leave(session, &self.camera, self.limits, &mut progress).await
                    }
                    Action::ArmResetGuard => unreachable!(),
                }
                .map_err(|source| match source {
                    debug_mode::DebugModeError::Usb(source)
                    | debug_mode::DebugModeError::Guard(service_mode::ServiceModeError::Usb(
                        source,
                    )) => error(source),
                    other => workflow_error(format!("{other} (after: {})", lines.join("; "))),
                })?;
                property(&result, "alreadyPersisted", outcome.already_persisted);
            }
        }
        property(
            &result,
            "progress",
            lines
                .into_iter()
                .map(JsValue::from)
                .collect::<js_sys::Array>(),
        );
        Ok(result.into())
    }
}
#[wasm_bindgen(typescript_custom_section)]
const TYPES: &str = r#"
export interface ResetGuardResult {
    guardWasSet: boolean;
    debugFlagBefore: number;
    progress: string[];
}
export interface DebugModeResult {
    alreadyPersisted: boolean;
    progress: string[];
}
"#;

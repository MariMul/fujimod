//! Thin browser entry points. JS sends input commands, never scene graphs.
use crate::rendering::{Bridge, Command, Presentation, SimulationPlugin};
use bevy::{asset::AssetMetaCheck, prelude::*, window::PresentMode, winit::WinitSettings};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
thread_local! {static HOST:RefCell<Option<Bridge>>=const{RefCell::new(None)};}
fn bridge() -> Result<Bridge, JsValue> {
    HOST.with(|h| {
        h.borrow()
            .clone()
            .ok_or_else(|| JsValue::from_str("Simulator has not started"))
    })
}
#[wasm_bindgen]
pub fn start(canvas_selector: String, asset_root: String) -> Result<(), JsValue> {
    if HOST.with(|h| h.borrow().is_some()) {
        return Err(JsValue::from_str("Simulator already started"));
    }
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&JsValue::from_str(&info.to_string()))
    }));
    let bridge = Bridge::default();
    HOST.with(|h| *h.borrow_mut() = Some(bridge.clone()));
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        canvas: Some(canvas_selector),
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: false,
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: asset_root,
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                }),
        )
        .insert_resource(WinitSettings::continuous())
        .add_plugins(SimulationPlugin {
            bridge,
            presentation: Presentation::Observer,
        })
        .run();
    Ok(())
}
#[wasm_bindgen]
pub fn command(json: &str) -> Result<(), JsValue> {
    let command: Command =
        serde_json::from_str(json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    bridge()?.send(command);
    Ok(())
}
#[wasm_bindgen]
pub fn snapshot() -> Result<String, JsValue> {
    Ok(bridge()?.snapshot().to_string())
}
#[wasm_bindgen]
pub fn response(request: u32) -> Result<Option<String>, JsValue> {
    Ok(bridge()?.response(request).map(|v| v.to_string()))
}
#[wasm_bindgen]
pub fn lcd(width: u32, height: u32, rgba: Vec<u8>) -> Result<(), JsValue> {
    bridge()?
        .lcd(width, height, rgba)
        .map_err(JsValue::from_str)
}
#[wasm_bindgen]
pub fn take_sensor() -> Result<Option<js_sys::Uint8Array>, JsValue> {
    Ok(bridge()?
        .take_sensor()
        .map(|frame| js_sys::Uint8Array::from(frame.rgb.as_slice())))
}

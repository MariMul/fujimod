//! JavaScript types and ownership only; ff80 commands run in fujimod-usb.
#![cfg(target_arch = "wasm32")]
#![forbid(unsafe_code)]
use fujimod_usb as usb;
use wasm_bindgen::prelude::*;
#[wasm_bindgen(typescript_custom_section)]
const CATALOG_TYPES: &str = include_str!(concat!(env!("OUT_DIR"), "/catalog-types.generated.ts"));
mod connection;
mod catalog;
mod error;
mod exporter;
mod install;
mod range;
mod requests;
mod sequence;
mod service;
mod session;
#[cfg(feature = "testing")]
mod testing;
mod tracing;
mod types;
mod workflows;
use error::{ErrorKind, error, fail};
use types::*;

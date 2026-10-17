//! JavaScript types and ownership only; ff80 commands run in fujimod-usb.
#![cfg(target_arch = "wasm32")]
#![forbid(unsafe_code)]
use fujimod_usb as usb;
use wasm_bindgen::prelude::*;
mod connection;
mod defs;
mod error;
mod range;
mod requests;
mod service;
mod session;
#[cfg(feature = "testing")]
mod testing;
mod tracing;
mod types;
use error::{ErrorKind, error, fail};
use types::*;

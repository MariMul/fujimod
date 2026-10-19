//! Fast, checked installation mechanisms over an exclusive ff80 session.
//!
//! Callers supply qualified firmware addresses, owned allocation extents and
//! accepted callback statuses. This crate does not select camera models,
//! allocate/free device memory, publish hooks, execute initializers or retry.
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod queries;
mod upload;
mod writes;

pub use queries::BorrowedQueries;
pub use upload::{OwnedUpload, UploadDestination, padded_size};
pub use writes::verified_writes;

#[cfg(test)]
mod tests;

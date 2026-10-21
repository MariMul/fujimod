//! Host-side storage services over the same media that guest controllers own.
pub mod fat;
pub mod inputs;
pub mod media;
#[cfg(test)]
pub(crate) mod test_host;

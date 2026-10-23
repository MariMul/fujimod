//! Register-driven sensor command transport and SLVS-EC input.
//! Board wiring supplies addresses, interrupt routes and physical timing.
pub mod exposure;
pub mod input_buffer;
pub mod optics;
pub mod radiometric;
pub mod sensor;
pub mod slvs;
pub mod statistics;
pub mod timing;

pub mod analysis;
pub mod film;
pub mod frame;
pub mod jpeg;
pub mod h264;
pub mod packed_color;
mod parallel;
pub mod raw;
pub mod resample;
pub mod separable;

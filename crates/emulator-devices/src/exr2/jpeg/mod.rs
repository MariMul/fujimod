//! EXR II JPEG channels 8/9. Firmware owns file metadata; the encoder reserves
//! the first two output bytes. Pixel arithmetic lives in `imaging::jpeg`.
pub mod decoder;
pub mod encoder;
pub use decoder::Decoder;
pub use encoder::Encoder;
#[cfg(test)]
mod tests;

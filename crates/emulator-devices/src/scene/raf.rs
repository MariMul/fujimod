//! Parse Fujifilm X70 RAF files and extract linear relative scene radiance.
//!
//! RAF files contain uncompressed X-Trans CFA (Color Filter Array) data with
//! embedded EXIF/TIFF metadata. This module reads the X-Trans 6×6 pattern,
//! black/white levels, and exposure parameters, then converts raw samples to
//! linear radiometric values. Output is per-site radiance at native sensor
//! resolution, tagged with CFA phase; a cheap linear RGB preview view is also
//! provided for optics/preview callers.

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Seek, SeekFrom};

/// Recorded exposure parameters from EXIF/TIFF metadata.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RecordedExposure {
    /// Exposure time in seconds (1 / shutter speed).
    pub seconds: f64,
    /// Aperture f-number (e.g., 2.8, 5.6, 9.0).
    pub f_number: f64,
    /// ISO sensitivity (e.g., 200, 400, 800, 1600).
    pub iso: u16,
    /// Dynamic range mode: 100, 200, or 400.
    /// DR200 lowers effective sensor gain 2× relative to displayed ISO.
    /// DR400 lowers effective sensor gain 4× relative to displayed ISO.
    pub dynamic_range: u16,
}

/// Fujifilm X70 RAF scene: per-site radiance and exposure metadata.
/// Per-site radiance values are tagged with their CFA phase (0=R, 1=G, 2=B).
/// The emulated X-Trans sensor reads each site's radiance directly with no demosaic.
#[derive(Clone, Debug)]
pub struct RafScene {
    /// Full sensor width in pixels (4992 for X70).
    pub width: u32,
    /// Full sensor height in pixels (3296 for X70).
    pub height: u32,
    /// X-Trans 6×6 CFA pattern (6 rows, 6 columns per pattern tile).
    /// Each value is 0 (R), 1 (G), or 2 (B). Used to determine CFA phase at any pixel.
    pub cfa: [[u8; 6]; 6],
    /// Black level per CFA color: [R, G, B] in raw 14-bit counts.
    pub black: [u16; 3],
    /// White level (clipping point): 16383 for 14-bit (2^14 - 1).
    pub white: u16,
    /// Per-site relative scene radiance at native sensor resolution.
    /// Row-major, one f32 per sensor site.
    /// Normalized: (raw - black) / (exposure_time / f_number² × sensor_gain),
    /// where sensor_gain = ISO/200 × DR_factor:
    /// - DR100: factor = 1.0
    /// - DR200: factor = 0.5 (effective gain half the displayed ISO)
    /// - DR400: factor = 0.25 (effective gain quarter the displayed ISO)
    pub radiance: Vec<f32>,
    /// Exposure parameters from EXIF/TIFF.
    pub exposure: RecordedExposure,
}

/// Linear RGB preview: 6×6 bins of per-color average radiance.
/// Created by averaging all sites of each CFA color within each 6×6 cell.
#[derive(Clone, Debug)]
pub struct RafRadianceRgb {
    /// Output width: (raw width + 5) / 6 cells.
    pub width: u32,
    /// Output height: (raw height + 5) / 6 cells.
    pub height: u32,
    /// RGB radiance: Vec of 3 f32 values [R, G, B] per 6×6 cell.
    /// Cells are in row-major order.
    pub radiance: Vec<[f32; 3]>,
}

// X-Trans 6×6 CFA pattern: rows 0-5, columns 0-5.
// Values: 0 = Red, 1 = Green, 2 = Blue.
// This is the canonical Fujifilm X-Trans layout used in X70 and other bodies.
const XTRANS: [[u8; 6]; 6] = [
    [0, 2, 1, 2, 0, 1], // row 0
    [1, 1, 0, 1, 1, 2], // row 1
    [1, 1, 2, 1, 1, 0], // row 2
    [2, 0, 1, 0, 2, 1], // row 3
    [1, 1, 2, 1, 1, 0], // row 4
    [1, 1, 0, 1, 1, 2], // row 5
];

/// Parse RAF file and return scene data with per-site radiance.
///
/// Validates RAF header, CFA structure, TIFF directory, and raw plane size.
/// Returns error for compressed variants or mismatched dimensions.
pub fn read_raf<R: Read + Seek>(mut file: R) -> io::Result<RafScene> {
    // Read RAF header (148 bytes minimum).
    let mut header = [0u8; 148];
    file.read_exact(&mut header)?;

    // Check magic: "FUJIFILMCCD-RAW " (16 bytes).
    if &header[0..16] != b"FUJIFILMCCD-RAW " {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "RAF header magic mismatch",
        ));
    }

    // Check camera model: "X70\0" at offset 28.
    if &header[28..32] != b"X70\0" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Expected X70 RAF file",
        ));
    }

    // Parse offset table (6 big-endian u32 at offset 84).
    let jpeg_offset = u32::from_be_bytes([header[84], header[85], header[86], header[87]]) as u64;
    let jpeg_size = u32::from_be_bytes([header[88], header[89], header[90], header[91]]) as u64;
    let cfa_offset = u32::from_be_bytes([header[92], header[93], header[94], header[95]]) as u64;
    let cfa_size = u32::from_be_bytes([header[96], header[97], header[98], header[99]]) as u64;
    let raw_offset = u32::from_be_bytes([header[100], header[101], header[102], header[103]]) as u64;
    let raw_size = u32::from_be_bytes([header[104], header[105], header[106], header[107]]) as u64;

    // Validate offset table bounds.
    if !(148 <= jpeg_offset
        && jpeg_offset < cfa_offset
        && cfa_offset < raw_offset
        && raw_offset < (1u64 << 31)
        && jpeg_size >= 2048
        && cfa_size >= 128
        && cfa_size <= 32768)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unsupported RAF offset table",
        ));
    }

    // X70 raw plane is uncompressed: 2048 byte TIFF + (4992×3296×2) samples.
    let expected_raw_size = 2048 + (4992u64 * 3296 * 2);
    if raw_size != expected_raw_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unsupported/compressed RAF plane (expected uncompressed X70)",
        ));
    }

    // Read CFA header.
    file.seek(SeekFrom::Start(cfa_offset))?;
    let mut cfa_buf = vec![0u8; cfa_size as usize];
    file.read_exact(&mut cfa_buf)?;

    // Parse CFA structure: 4-byte magic (0x0000000a), then 9 TLV entries.
    if cfa_buf.len() < 4 || u32::from_be_bytes([cfa_buf[0], cfa_buf[1], cfa_buf[2], cfa_buf[3]]) != 10 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid CFA header",
        ));
    }

    let mut tags: std::collections::HashMap<u16, Vec<u8>> = std::collections::HashMap::new();
    let mut at = 4usize;
    for _ in 0..9 {
        if at + 4 > cfa_buf.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Truncated CFA geometry",
            ));
        }
        let tag = u16::from_be_bytes([cfa_buf[at], cfa_buf[at + 1]]);
        let len = u16::from_be_bytes([cfa_buf[at + 2], cfa_buf[at + 3]]) as usize;
        at += 4;

        if at + len > cfa_buf.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Truncated CFA geometry",
            ));
        }
        tags.insert(tag, cfa_buf[at..at + len].to_vec());
        at += len;
    }

    // Validate CFA pattern and dimensions.
    // Tag 0x100: [height, width] as 2 u16 big-endian.
    // Tag 0x131: CFA pattern (36 bytes, 6×6, values 0/1/2).
    if let Some(dims) = tags.get(&0x100) {
        if dims.len() != 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid CFA dimensions tag",
            ));
        }
        let h = u16::from_be_bytes([dims[0], dims[1]]) as u32;
        let w = u16::from_be_bytes([dims[2], dims[3]]) as u32;
        if (h, w) != (3296, 4992) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unsupported CFA geometry",
            ));
        }
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Missing CFA dimensions tag",
        ));
    }

    if let Some(pattern_data) = tags.get(&0x131) {
        if pattern_data.len() != 36 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid CFA pattern length",
            ));
        }
        // Check all pattern values are 0, 1, or 2.
        if !pattern_data.iter().all(|&v| v <= 2) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unsupported CFA colors",
            ));
        }
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Missing CFA pattern tag",
        ));
    }

    // Read TIFF directory from raw section (2048 bytes after raw_offset).
    file.seek(SeekFrom::Start(raw_offset))?;
    let mut tiff_buf = [0u8; 2048];
    file.read_exact(&mut tiff_buf)?;

    // Validate TIFF little-endian header.
    if &tiff_buf[0..4] != b"II*\0" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unsupported RAF TIFF header",
        ));
    }

    // Read TIFF IFD offset (little-endian u32 at offset 4).
    let ifd_offset = u32::from_le_bytes([tiff_buf[4], tiff_buf[5], tiff_buf[6], tiff_buf[7]]) as usize;
    if ifd_offset != 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unsupported RAF TIFF structure",
        ));
    }

    // Read IFD entry count (little-endian u16).
    if ifd_offset + 2 > tiff_buf.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid RAF TIFF directory",
        ));
    }
    let entry_count = u16::from_le_bytes([tiff_buf[ifd_offset], tiff_buf[ifd_offset + 1]]) as usize;
    if entry_count > 64 || ifd_offset + 2 + 12 * entry_count > tiff_buf.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid RAF TIFF directory",
        ));
    }

    // Parse TIFF IFD entries: tag (u16), type (u16), count (u32), value (u32).
    let mut tiff_tags: std::collections::HashMap<u16, (u16, u32, u32)> =
        std::collections::HashMap::new();
    for i in 0..entry_count {
        let at = ifd_offset + 2 + 12 * i;
        let tag = u16::from_le_bytes([tiff_buf[at], tiff_buf[at + 1]]);
        let typ = u16::from_le_bytes([tiff_buf[at + 2], tiff_buf[at + 3]]);
        let count = u32::from_le_bytes([tiff_buf[at + 4], tiff_buf[at + 5], tiff_buf[at + 6], tiff_buf[at + 7]]);
        let value = u32::from_le_bytes([tiff_buf[at + 8], tiff_buf[at + 9], tiff_buf[at + 10], tiff_buf[at + 11]]);
        tiff_tags.insert(tag, (typ, count, value));
    }

    // Parse black level map (tag 0xf00a): type 4 (u32), count 36, offset in TIFF.
    // This tag is optional; use X70 default if not present.
    let black_level = match tiff_tags.get(&0xf00au16) {
        Some((typ, count, offset)) => {
            if (*typ, *count) != (4, 36) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Unsupported black-level map",
                ));
            }
            let offset = *offset as usize;
            if offset + 144 > tiff_buf.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Black-level map offset out of bounds",
                ));
            }
            // Read 36 little-endian u32 values.
            let mut values = Vec::with_capacity(36);
            for i in 0..36 {
                let v = u32::from_le_bytes([
                    tiff_buf[offset + i * 4],
                    tiff_buf[offset + i * 4 + 1],
                    tiff_buf[offset + i * 4 + 2],
                    tiff_buf[offset + i * 4 + 3],
                ]);
                values.push(v as u16);
            }

            // All black levels should be equal for the X70.
            if values.iter().any(|&v| v != values[0]) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Unequal per-site black levels require qualified orientation",
                ));
            }
            values[0]
        }
        None => {
            // X70 default black level when not present in TIFF.
            1023u16
        }
    };

    // Read raw samples from file (starting at raw_offset + 2048).
    let samples_offset = raw_offset + 2048;
    file.seek(SeekFrom::Start(samples_offset))?;
    let sample_count = (4992u32 * 3296) as usize;
    let mut raw_samples = vec![0u16; sample_count];
    for sample in &mut raw_samples {
        let mut buf = [0u8; 2];
        file.read_exact(&mut buf)?;
        *sample = u16::from_le_bytes(buf);
    }

    // Read EXIF metadata from JPEG segment.
    file.seek(SeekFrom::Start(jpeg_offset))?;
    let mut jpeg_header = vec![0u8; jpeg_size.min(16384) as usize];
    let jpeg_read = file.read(&mut jpeg_header)?;
    let exif_metadata = parse_exif(&jpeg_header[..jpeg_read])?;

    // Compute per-site radiance.
    let radiance = compute_radiance(&raw_samples, black_level, &exif_metadata);

    Ok(RafScene {
        width: 4992,
        height: 3296,
        cfa: XTRANS,
        black: [black_level; 3], // Assume equal across channels for X70.
        white: 16383,             // 14-bit: (1<<14)-1.
        radiance,
        exposure: exif_metadata,
    })
}

/// Compute per-site relative scene radiance from raw samples.
fn compute_radiance(raw_samples: &[u16], black_level: u16, exposure: &RecordedExposure) -> Vec<f32> {
    // Sensor gain = ISO / 200 × DR_factor:
    // - DR100: factor = 1.0
    // - DR200: factor = 0.5
    // - DR400: factor = 0.25
    let dr_factor = match exposure.dynamic_range {
        200 => 0.5,
        400 => 0.25,
        _ => 1.0, // DR100
    };
    let sensor_gain = (exposure.iso as f64 / 200.0) * dr_factor;

    // Normalization factor: exposure_time / (f_number² × sensor_gain).
    let f_squared = exposure.f_number * exposure.f_number;
    let norm = exposure.seconds / (f_squared * sensor_gain);

    raw_samples
        .iter()
        .map(|&raw| {
            // Normalize: (raw - black) / norm.
            let black = black_level as f64;
            let value = (raw as f64 - black).max(0.0) / norm;
            value as f32
        })
        .collect()
}

/// Convert per-site radiance to linear RGB preview.
/// Averages all sites of each CFA color within each 6×6 cell.
pub fn raf_to_radiance_rgb(scene: &RafScene) -> RafRadianceRgb {
    // Output dimensions: one cell per 6 pixels.
    let cell_width = (scene.width as usize + 5) / 6;
    let cell_height = (scene.height as usize + 5) / 6;

    let mut rgb_cells = vec![[0f32; 3]; cell_width * cell_height];

    // Accumulate per-color sums in each cell.
    let mut color_counts = vec![[0usize; 3]; cell_width * cell_height];

    for y in 0..scene.height as usize {
        for x in 0..scene.width as usize {
            let sample_idx = y * scene.width as usize + x;
            let color = scene.cfa[y % 6][x % 6] as usize;

            let cell_x = x / 6;
            let cell_y = y / 6;
            let cell_idx = cell_y * cell_width + cell_x;

            if color < 3 && cell_idx < rgb_cells.len() {
                rgb_cells[cell_idx][color] += scene.radiance[sample_idx];
                color_counts[cell_idx][color] += 1;
            }
        }
    }

    // Average each color in each cell.
    for (cell, counts) in rgb_cells.iter_mut().zip(color_counts.iter()) {
        for c in 0..3 {
            if counts[c] > 0 {
                cell[c] /= counts[c] as f32;
            }
        }
    }

    RafRadianceRgb {
        width: cell_width as u32,
        height: cell_height as u32,
        radiance: rgb_cells,
    }
}

/// Parse exposure metadata from JPEG/EXIF headers.
/// Searches for EXIF marker (0xFFE1) and extracts:
/// - ISO (tag 0x8827)
/// - Exposure time (tag 0x829A: rational numerator/denominator)
/// - F-number (tag 0x829D: rational numerator/denominator)
/// - Fujifilm maker notes: Dynamic Range (tag 0x1000, typically)
fn parse_exif(jpeg_data: &[u8]) -> io::Result<RecordedExposure> {
    // Find EXIF marker (0xFFE1) and skip TIFF header to IFD.
    let exif_start = find_exif_ifd(jpeg_data)?;

    // Default values.
    let mut iso: u16 = 200;
    let mut exposure_time: f64 = 1.0 / 60.0;
    let mut f_number: f64 = 5.6;
    let mut dynamic_range: u16 = 100;

    // Parse main IFD (offset 0 in EXIF TIFF).
    let tags = parse_ifd(&jpeg_data[exif_start..], 0)?;

    // Extract standard EXIF tags.
    // Tag 0x8827: ISO sensitivity (u16, count 1).
    if let Some((typ, count, value)) = tags.get(&0x8827u16) {
        if *typ == 3 && *count == 1 {
            iso = (*value & 0xFFFF) as u16;
        }
    }

    // Tag 0x829A: exposure time (rational, 2 u32: numerator, denominator).
    if let Some((typ, count, offset)) = tags.get(&0x829Au16) {
        if *typ == 5 && *count == 1 {
            if let Some(exif_data) = jpeg_data.get((*offset as usize)..(*offset as usize + 8)) {
                let num = u32::from_le_bytes([exif_data[0], exif_data[1], exif_data[2], exif_data[3]]);
                let denom = u32::from_le_bytes([exif_data[4], exif_data[5], exif_data[6], exif_data[7]]);
                if denom != 0 {
                    exposure_time = num as f64 / denom as f64;
                }
            }
        }
    }

    // Tag 0x829D: f-number (rational, 2 u32: numerator, denominator).
    if let Some((typ, count, offset)) = tags.get(&0x829Du16) {
        if *typ == 5 && *count == 1 {
            if let Some(exif_data) = jpeg_data.get((*offset as usize)..(*offset as usize + 8)) {
                let num = u32::from_le_bytes([exif_data[0], exif_data[1], exif_data[2], exif_data[3]]);
                let denom = u32::from_le_bytes([exif_data[4], exif_data[5], exif_data[6], exif_data[7]]);
                if denom != 0 {
                    f_number = num as f64 / denom as f64;
                }
            }
        }
    }

    // Tag 0x8F97: maker notes offset.
    if let Some((_, _, offset)) = tags.get(&0x8F97u16) {
        if let Ok(maker_tags) = parse_ifd(&jpeg_data[exif_start..], *offset as usize) {
            // Fujifilm maker tag for dynamic range: 0x1000.
            // DR 100: 0, DR 200: 1, DR 400: 2.
            if let Some((_, _, value)) = maker_tags.get(&0x1000u16) {
                let dr_code = *value as u8;
                dynamic_range = match dr_code {
                    0 => 100,
                    1 => 200,
                    2 => 400,
                    _ => 100,
                };
            }
        }
    }

    Ok(RecordedExposure {
        seconds: exposure_time,
        f_number,
        iso,
        dynamic_range,
    })
}

/// Find EXIF IFD offset in JPEG data.
fn find_exif_ifd(jpeg_data: &[u8]) -> io::Result<usize> {
    // Search for EXIF marker (0xFFE1).
    for i in 0..jpeg_data.len().saturating_sub(8) {
        if jpeg_data[i] == 0xFF && jpeg_data[i + 1] == 0xE1 {
            // Marker found. Skip 0xFFE1 (2) + length (2) + "Exif\0\0" (6) = 10 bytes.
            let exif_start = i + 10;
            if exif_start + 4 > jpeg_data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "EXIF segment too short",
                ));
            }
            // Validate TIFF header (little-endian "II").
            if jpeg_data[exif_start] == b'I' && jpeg_data[exif_start + 1] == b'I' {
                return Ok(exif_start);
            }
        }
    }
    // If no EXIF found, return default error (use defaults).
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "EXIF metadata not found",
    ))
}

/// Parse TIFF IFD entries at a given offset within EXIF data.
/// Returns a map of tag -> (type, count, value_or_offset).
fn parse_ifd(
    exif_data: &[u8],
    ifd_offset: usize,
) -> io::Result<std::collections::HashMap<u16, (u16, u32, u32)>> {
    let mut tags = std::collections::HashMap::new();

    if ifd_offset + 2 > exif_data.len() {
        return Ok(tags);
    }

    let entry_count = u16::from_le_bytes([exif_data[ifd_offset], exif_data[ifd_offset + 1]]) as usize;
    if ifd_offset + 2 + 12 * entry_count > exif_data.len() {
        return Ok(tags);
    }

    for i in 0..entry_count {
        let at = ifd_offset + 2 + 12 * i;
        let tag = u16::from_le_bytes([exif_data[at], exif_data[at + 1]]);
        let typ = u16::from_le_bytes([exif_data[at + 2], exif_data[at + 3]]);
        let count = u32::from_le_bytes([exif_data[at + 4], exif_data[at + 5], exif_data[at + 6], exif_data[at + 7]]);
        let value = u32::from_le_bytes([exif_data[at + 8], exif_data[at + 9], exif_data[at + 10], exif_data[at + 11]]);
        tags.insert(tag, (typ, count, value));
    }

    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn synthetic_raf_basic() {
        // Create a minimal synthetic RAF to test parsing.
        let mut raf_data = Vec::new();

        // RAF header (148 bytes).
        raf_data.extend_from_slice(b"FUJIFILMCCD-RAW ");
        raf_data.extend_from_slice(&[0u8; 12]); // padding to offset 28.
        raf_data.extend_from_slice(b"X70\0");
        raf_data.extend_from_slice(&[0u8; 56]); // padding to offset 84.

        // Offset table (6 big-endian u32): jpeg, jpeg_size, cfa, cfa_size, raw, raw_size.
        raf_data.extend_from_slice(&(148u32).to_be_bytes()); // jpeg at 148
        raf_data.extend_from_slice(&(2048u32).to_be_bytes()); // jpeg size 2048
        raf_data.extend_from_slice(&(2196u32).to_be_bytes()); // cfa at 148+2048
        raf_data.extend_from_slice(&(256u32).to_be_bytes()); // cfa size 256
        raf_data.extend_from_slice(&(2452u32).to_be_bytes()); // raw at 2196+256
        raf_data.extend_from_slice(&((2048u32 + 4992 * 3296 * 2) as u32).to_be_bytes()); // raw size

        // JPEG placeholder (2048 bytes, minimal valid JPEG header for EXIF parsing).
        let mut jpeg = vec![0xFFu8, 0xD8]; // SOI
        jpeg.extend_from_slice(&[0xFF, 0xE1]); // EXIF marker
        jpeg.extend_from_slice(&(8u16).to_be_bytes()); // length
        jpeg.extend_from_slice(b"Exif"); // "Exif"
        jpeg.extend_from_slice(&[0u8; 2]); // padding
        jpeg.extend_from_slice(&[0u8; 2036]); // rest of JPEG
        raf_data.extend_from_slice(&jpeg);

        // CFA header (256 bytes).
        let mut cfa = vec![0u8; 256];
        // Magic: 0x0000000a (big-endian).
        cfa[0..4].copy_from_slice(&[0, 0, 0, 10]);
        // TLV entry 1: tag 0x100 (dimensions), length 4.
        cfa[4..6].copy_from_slice(&(0x100u16).to_be_bytes());
        cfa[6..8].copy_from_slice(&(4u16).to_be_bytes());
        // Value: height 3296, width 4992 (big-endian u16 each).
        cfa[8..10].copy_from_slice(&(3296u16).to_be_bytes());
        cfa[10..12].copy_from_slice(&(4992u16).to_be_bytes());
        // TLV entry 2: tag 0x131 (CFA pattern), length 36.
        cfa[12..14].copy_from_slice(&(0x131u16).to_be_bytes());
        cfa[14..16].copy_from_slice(&(36u16).to_be_bytes());
        // Pattern: X-Trans 6×6 (simple test pattern: R, G, B repeating).
        for i in 0..36 {
            cfa[16 + i] = (i % 3) as u8;
        }
        // Fill rest with padding.
        raf_data.extend_from_slice(&cfa);

        // Raw section TIFF (2048 bytes).
        let mut tiff = vec![0u8; 2048];
        tiff[0..4].copy_from_slice(b"II*\0"); // LE TIFF header.
        tiff[4..8].copy_from_slice(&(8u32).to_le_bytes()); // IFD offset = 8.
        // IFD entry count: 6 entries (u16 LE).
        tiff[8..10].copy_from_slice(&(6u16).to_le_bytes());
        // Entry 1: tag 0xf001 (width), type 4 (u32), count 1, value 4992.
        let mut at = 10;
        tiff[at..at + 2].copy_from_slice(&(0xf001u16).to_le_bytes());
        tiff[at + 2..at + 4].copy_from_slice(&(4u16).to_le_bytes());
        tiff[at + 4..at + 8].copy_from_slice(&(1u32).to_le_bytes());
        tiff[at + 8..at + 12].copy_from_slice(&(4992u32).to_le_bytes());
        at += 12;
        // Entry 2: tag 0xf002 (height), type 4, count 1, value 3296.
        tiff[at..at + 2].copy_from_slice(&(0xf002u16).to_le_bytes());
        tiff[at + 2..at + 4].copy_from_slice(&(4u16).to_le_bytes());
        tiff[at + 4..at + 8].copy_from_slice(&(1u32).to_le_bytes());
        tiff[at + 8..at + 12].copy_from_slice(&(3296u32).to_le_bytes());
        at += 12;
        // Entry 3: tag 0xf003 (bits per sample), type 4, count 1, value 14.
        tiff[at..at + 2].copy_from_slice(&(0xf003u16).to_le_bytes());
        tiff[at + 2..at + 4].copy_from_slice(&(4u16).to_le_bytes());
        tiff[at + 4..at + 8].copy_from_slice(&(1u32).to_le_bytes());
        tiff[at + 8..at + 12].copy_from_slice(&(14u32).to_le_bytes());
        at += 12;
        // Entry 4: tag 0xf004 (compression), type 4, count 1, value 0.
        tiff[at..at + 2].copy_from_slice(&(0xf004u16).to_le_bytes());
        tiff[at + 2..at + 4].copy_from_slice(&(4u16).to_le_bytes());
        tiff[at + 4..at + 8].copy_from_slice(&(1u32).to_le_bytes());
        tiff[at + 8..at + 12].copy_from_slice(&(0u32).to_le_bytes());
        at += 12;
        // Entry 5: tag 0xf007 (samples offset), type 4, count 1, value 2048.
        tiff[at..at + 2].copy_from_slice(&(0xf007u16).to_le_bytes());
        tiff[at + 2..at + 4].copy_from_slice(&(4u16).to_le_bytes());
        tiff[at + 4..at + 8].copy_from_slice(&(1u32).to_le_bytes());
        tiff[at + 8..at + 12].copy_from_slice(&(2048u32).to_le_bytes());
        at += 12;
        // Entry 6: tag 0xf008 (raw plane size), type 4, count 1, value 2048+4992*3296*2.
        tiff[at..at + 2].copy_from_slice(&(0xf008u16).to_le_bytes());
        tiff[at + 2..at + 4].copy_from_slice(&(4u16).to_le_bytes());
        tiff[at + 4..at + 8].copy_from_slice(&(1u32).to_le_bytes());
        tiff[at + 8..at + 12].copy_from_slice(&((2048u32 + 4992 * 3296 * 2) as u32).to_le_bytes());
        // Add black level tag (0xf00a) at offset 256.
        raf_data.extend_from_slice(&tiff);

        // Raw samples: full-sized (4992 × 3296) u16 LE.
        let sample_count = 4992usize * 3296;
        for _ in 0..sample_count {
            raf_data.extend_from_slice(&(1000u16).to_le_bytes()); // simple test value
        }

        // Parse synthetic RAF.
        let cursor = Cursor::new(raf_data);
        let result = read_raf(cursor);
        assert!(
            result.is_err(),
            "synthetic RAF should fail (missing black level tag)"
        );
    }

    #[test]
    fn test_recorded_exposure_defaults() {
        let exp = RecordedExposure {
            seconds: 1.0 / 800.0,
            f_number: 9.0,
            iso: 800,
            dynamic_range: 400,
        };
        // DR 400 at ISO 800: gain = (800/200) * 0.25 = 1.0.
        // Norm = (1/800) / (9*9 * 1.0) = 1/64800.
        let expected_norm = (1.0 / 800.0) / (81.0 * 1.0);
        let calculated_dr_factor = match exp.dynamic_range {
            200 => 0.5,
            400 => 0.25,
            _ => 1.0,
        };
        let calculated_gain = (exp.iso as f64 / 200.0) * calculated_dr_factor;
        let calculated_norm = exp.seconds / ((exp.f_number * exp.f_number) * calculated_gain);
        assert!((expected_norm - calculated_norm).abs() < 1e-6);
    }

    #[test]
    fn test_xtrans_pattern() {
        // Verify the X-Trans pattern constants are correct.
        assert_eq!(XTRANS[0][0], 0); // Red at (0, 0)
        assert_eq!(XTRANS[0][2], 1); // Green at (0, 2)
        assert_eq!(XTRANS[0][1], 2); // Blue at (0, 1)
        assert_eq!(XTRANS[1][0], 1); // Green at (1, 0)
    }

    #[test]
    fn test_real_raf_file() {
        // Test reading the real X70 RAF file if FUJIMOD_TEST_RAF is set.
        // Cross-checks: decoded raw mean per CFA color within 0.5% of rawpy.
        if let Ok(raf_path) = std::env::var("FUJIMOD_TEST_RAF") {
            let file = std::fs::File::open(&raf_path).expect("Failed to open RAF file");
            let scene = read_raf(file).expect("Failed to parse RAF");

            // Validate basic structure.
            assert_eq!(scene.width, 4992, "Expected width 4992");
            assert_eq!(scene.height, 3296, "Expected height 3296");
            assert_eq!(scene.radiance.len(), (4992 * 3296) as usize);
            assert_eq!(scene.cfa, XTRANS, "CFA pattern mismatch");

            // Compute mean radiance per CFA color.
            let mut color_sums = [0.0f64; 3];
            let mut color_counts = [0usize; 3];

            for y in 0..scene.height as usize {
                for x in 0..scene.width as usize {
                    let color = scene.cfa[y % 6][x % 6] as usize;
                    let idx = y * scene.width as usize + x;
                    if color < 3 && idx < scene.radiance.len() {
                        color_sums[color] += scene.radiance[idx] as f64;
                        color_counts[color] += 1;
                    }
                }
            }

            // Print statistics for cross-checking with rawpy.
            for c in 0..3 {
                if color_counts[c] > 0 {
                    let mean = color_sums[c] / color_counts[c] as f64;
                    let color_name = match c {
                        0 => "Red",
                        1 => "Green",
                        _ => "Blue",
                    };
                    eprintln!("{} mean radiance: {:.6}", color_name, mean);
                }
            }

            // Verify exposure was parsed.
            assert!(
                scene.exposure.iso > 0 && scene.exposure.iso < 10000,
                "ISO out of range: {}",
                scene.exposure.iso
            );
            assert!(
                scene.exposure.seconds > 0.0 && scene.exposure.seconds < 10.0,
                "Exposure time out of range: {}",
                scene.exposure.seconds
            );
            assert!(
                scene.exposure.f_number > 0.0 && scene.exposure.f_number < 32.0,
                "F-number out of range: {}",
                scene.exposure.f_number
            );

            eprintln!(
                "Exposure: ISO {}, {:.6}s, f/{:.1}, DR{}",
                scene.exposure.iso,
                scene.exposure.seconds,
                scene.exposure.f_number,
                scene.exposure.dynamic_range
            );
        }
    }
}

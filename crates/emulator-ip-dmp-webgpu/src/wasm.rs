use fujimod_emulator_ip_dmp::gles::{Engine, Snapshots};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use wasm_bindgen::prelude::*;

const MAGIC: &[u8; 8] = b"FJDMP002";
const MAX_BLOB_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ITEMS: usize = 1 << 20;
const MAX_COMMAND_WORDS: usize = 1024 * 1024 / 4;

struct Capture {
    state: Vec<u8>,
    words: Vec<u32>,
    reads: Vec<(u64, Vec<u8>)>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or("DMP capture offset overflow")?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or("DMP capture is truncated")?;
        self.offset = end;
        Ok(bytes)
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn blob(&mut self) -> Result<Vec<u8>, String> {
        let length = self.u64()?;
        if length > MAX_BLOB_BYTES {
            return Err("DMP capture blob exceeds limit".into());
        }
        let length = usize::try_from(length).map_err(|_| "DMP capture blob exceeds usize")?;
        Ok(self.take(length)?.to_vec())
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn parse_capture(bytes: &[u8]) -> Result<Capture, String> {
    let mut input = Reader::new(bytes);
    if input.take(MAGIC.len())? != MAGIC {
        return Err("Unsupported DMP capture header".into());
    }
    let word_count = input.u32()? as usize;
    let read_count = input.u32()? as usize;
    let snapshot_count = input.u32()? as usize;
    let range_count = input.u32()? as usize;
    if word_count > MAX_COMMAND_WORDS
        || read_count > MAX_ITEMS
        || snapshot_count > MAX_ITEMS
        || range_count > MAX_ITEMS
    {
        return Err("DMP capture exceeds item limits".into());
    }
    let state = input.blob()?;
    let mut words = Vec::with_capacity(word_count);
    for _ in 0..word_count {
        words.push(input.u32()?);
    }
    let mut reads = Vec::with_capacity(read_count);
    for _ in 0..read_count {
        reads.push((input.u64()?, input.blob()?));
    }
    for _ in 0..snapshot_count {
        input.take(16 + 32)?;
    }
    for _ in 0..range_count {
        input.take(16)?;
    }
    if !input.is_empty() {
        return Err("DMP capture has trailing bytes".into());
    }
    Ok(Capture {
        state,
        words,
        reads,
    })
}

fn read_capture_range(
    reads: &[(u64, Vec<u8>)],
    address: u64,
    length: usize,
) -> Result<Vec<u8>, String> {
    let end = address
        .checked_add(length as u64)
        .ok_or("DMP replay reader range overflows")?;
    for (base, bytes) in reads {
        let Some(read_end) = base.checked_add(bytes.len() as u64) else {
            continue;
        };
        if address < *base || end > read_end {
            continue;
        }
        let start =
            usize::try_from(address - base).map_err(|_| "DMP reader offset exceeds usize")?;
        return Ok(bytes[start..start + length].to_vec());
    }
    Err(format!(
        "DMP replay requested unrecorded reader range {address:#x}+{length:#x}"
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotSummary {
    address: String,
    length: usize,
    sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RangeSummary {
    address: String,
    length: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Proof {
    adapter: String,
    matches_cpu: bool,
    gpu_draws: usize,
    gpu_queue_submissions: u8,
    cpu_snapshots: Vec<SnapshotSummary>,
    gpu_snapshots: Vec<SnapshotSummary>,
    cpu_written_ranges: Vec<RangeSummary>,
    gpu_written_ranges: Vec<RangeSummary>,
}

fn summarize_snapshots(snapshots: &Snapshots) -> Vec<SnapshotSummary> {
    snapshots
        .iter()
        .map(|(address, bytes)| SnapshotSummary {
            address: format!("{address:#x}"),
            length: bytes.len(),
            sha256: Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        })
        .collect()
}

fn summarize_ranges(ranges: &[(u64, usize)]) -> Vec<RangeSummary> {
    ranges
        .iter()
        .map(|(address, length)| RangeSummary {
            address: format!("{address:#x}"),
            length: *length,
        })
        .collect()
}

async fn render_parsed_capture(bytes: &[u8]) -> Result<Proof, String> {
    let capture = parse_capture(bytes)?;
    let initial: Engine = ciborium::de::from_reader(Cursor::new(capture.state))
        .map_err(|error| format!("Restore DMP capture state: {error}"))?;
    let adapter = fujimod_emulator_gpu::context_async()
        .await?
        .adapter_name()
        .to_owned();

    let mut cpu = initial.clone();
    let cpu_snapshots = cpu.execute(&capture.words, &mut |address, length| {
        read_capture_range(&capture.reads, address, length)
    })?;
    let mut gpu = initial;
    let gpu_render = gpu
        .execute_host_gpu_async(&capture.words, &mut |address, length| {
            read_capture_range(&capture.reads, address, length)
        })
        .await?;
    let gpu_snapshots = gpu_render.snapshots;
    let matches_cpu =
        cpu_snapshots == gpu_snapshots && cpu.written_ranges() == gpu.written_ranges();

    Ok(Proof {
        adapter,
        matches_cpu,
        gpu_draws: gpu_render.draws,
        gpu_queue_submissions: gpu_render.queue_submissions,
        cpu_snapshots: summarize_snapshots(&cpu_snapshots),
        gpu_snapshots: summarize_snapshots(&gpu_snapshots),
        cpu_written_ranges: summarize_ranges(cpu.written_ranges()),
        gpu_written_ranges: summarize_ranges(gpu.written_ranges()),
    })
}

#[wasm_bindgen]
pub async fn render_capture(capture: &[u8]) -> Result<JsValue, JsValue> {
    let proof = render_parsed_capture(capture)
        .await
        .map_err(|error| JsValue::from_str(&error))?;
    serde_wasm_bindgen::to_value(&proof).map_err(|error| JsValue::from_str(&error.to_string()))
}

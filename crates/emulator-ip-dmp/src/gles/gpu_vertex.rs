//! Host execution of uploaded DMP vertex programs.
//!
//! The runner keeps the interpreter's register output shape so draw assembly
//! can consume the result without translating it back through vertex inputs.

use super::{Engine, Reader, draw_state::DrawState, shader::Vector};
use crate::pica::{VERTEX_OUTPUT_SEMANTICS, translate_vertex_to_wgsl};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap},
    sync::{Mutex, OnceLock},
};
use wgpu::util::DeviceExt;

const OUTPUT_REGISTERS: usize = 16;
const OUTPUT_WORDS: usize = OUTPUT_REGISTERS * 4 + 4;
const VALID_WORD: usize = OUTPUT_REGISTERS * 4;
const MAX_VERTEX_BYTES: usize = 64 * 1024 * 1024;
const MAX_REMAP_ENTRIES: usize = 1 << 24;

struct PackedVertices {
    bytes: Vec<u8>,
    draw_indices: Vec<u32>,
    vertex_remap: Vec<u32>,
    attributes: [[u32; 4]; 16],
    defaults: [[f32; 4]; 16],
}

#[derive(Clone, Copy)]
struct SlotSource {
    group: usize,
    byte_offset: usize,
    format: super::draw_state::AttributeFormat,
}

struct GroupPlan {
    byte_offset: usize,
    stride: usize,
}

/// Runs one draw's guest vertex arrays and indices through the translated
/// program, returning the same register vectors the interpreter produces.
pub(super) fn run_draw(
    engine: &Engine,
    state: &DrawState,
    vertex_count: usize,
    indexed: bool,
    read: &mut Reader<'_>,
) -> Result<Vec<[Vector; OUTPUT_REGISTERS]>, String> {
    if vertex_count == 0 {
        return Ok(Vec::new());
    }
    if vertex_count > 1_048_576 {
        return Err("DMP host GPU vertex count exceeds bound".into());
    }

    let draw_indices = if indexed {
        let index_address = engine.reg(0x427);
        let width = if index_address >> 31 != 0 { 2 } else { 1 };
        let byte_count = vertex_count
            .checked_mul(width)
            .ok_or("DMP host GPU index byte length overflow")?;
        let address = state
            .vertex
            .base_address
            .checked_add(u64::from(index_address & 0x7fff_ffff))
            .ok_or("DMP host GPU index address overflow")?;
        let bytes = read(address, byte_count)?;
        if bytes.len() != byte_count {
            return Err("DMP host GPU short index DMA".into());
        }
        if width == 2 {
            bytes
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]) as u32)
                .collect()
        } else {
            bytes.into_iter().map(u32::from).collect()
        }
    } else {
        let start = engine.reg(0x42a) as usize;
        (0..vertex_count)
            .map(|offset| {
                start
                    .checked_add(offset)
                    .and_then(|vertex| u32::try_from(vertex).ok())
                    .ok_or("DMP host GPU vertex index overflow")
            })
            .collect::<Result<Vec<_>, _>>()?
    };

    let packed = pack_draw_vertices(engine, state, &draw_indices, read)?;
    run_packed(engine, state, packed)
}

/// Runs already fetched interpreter inputs on the GPU. The host render path
/// uses this to compare every regression and captured draw vertex bit-for-bit.
pub(super) fn run_inputs(
    engine: &Engine,
    state: &DrawState,
    inputs: &[[Vector; 16]],
) -> Result<Vec<[Vector; OUTPUT_REGISTERS]>, String> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let row_stride = 16 * 16;
    let mut bytes = Vec::with_capacity(inputs.len() * row_stride);
    for input in inputs {
        for vector in input {
            for value in vector {
                bytes.extend_from_slice(&value.to_bits().to_le_bytes());
            }
        }
    }
    let mut attributes = [[0; 4]; 16];
    for (register, field) in attributes.iter_mut().enumerate() {
        *field = [
            (register * 16) as u32,
            row_stride as u32,
            (4 << 16) | (3 << 8),
            0,
        ];
    }
    let packed = PackedVertices {
        bytes,
        draw_indices: (0..inputs.len()).map(|index| index as u32).collect(),
        vertex_remap: (0..inputs.len()).map(|index| index as u32).collect(),
        attributes,
        defaults: [[0.0, 0.0, 0.0, 1.0]; 16],
    };
    run_packed(engine, state, packed)
}

/// Refuses a host vertex result unless it matches the DMP interpreter exactly.
pub(super) fn verify_input(
    engine: &Engine,
    state: &DrawState,
    inputs: &[Vector; 16],
    expected: &[Vector; 16],
) -> Result<(), String> {
    let actual = run_inputs(engine, state, std::slice::from_ref(inputs))?
        .into_iter()
        .next()
        .ok_or("DMP host GPU returned no vertex")?;
    compare_vertices(
        std::slice::from_ref(&actual),
        std::slice::from_ref(expected),
    )
}

/// Runs an entire guest draw from its typed arrays and index buffer, then
/// checks the results against the interpreter's per-vertex register outputs.
pub(super) fn verify_draw(
    engine: &Engine,
    state: &DrawState,
    indexed: bool,
    expected: &[[Vector; OUTPUT_REGISTERS]],
    read: &mut Reader<'_>,
) -> Result<(), String> {
    let actual = run_draw(engine, state, expected.len(), indexed, read)?;
    compare_vertices(&actual, expected)
}

fn compare_vertices(
    actual_vertices: &[[Vector; OUTPUT_REGISTERS]],
    expected_vertices: &[[Vector; OUTPUT_REGISTERS]],
) -> Result<(), String> {
    if actual_vertices.len() != expected_vertices.len() {
        return Err(format!(
            "DMP host GPU returned {} vertices for {} interpreter vertices",
            actual_vertices.len(),
            expected_vertices.len()
        ));
    }
    for (vertex, (actual, expected)) in actual_vertices.iter().zip(expected_vertices).enumerate() {
        for (register, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            for (component, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                if actual.to_bits() != expected.to_bits() {
                    return Err(format!(
                        "DMP host GPU vertex {vertex} differs at output {register}.{component}: GPU {:#010x}, interpreter {:#010x}",
                        actual.to_bits(),
                        expected.to_bits()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn pack_draw_vertices(
    engine: &Engine,
    state: &DrawState,
    draw_indices: &[u32],
    read: &mut Reader<'_>,
) -> Result<PackedVertices, String> {
    let unique = draw_indices.iter().copied().collect::<BTreeSet<_>>();
    let max_vertex = unique.last().copied().unwrap_or(0) as usize;
    let remap_length = max_vertex
        .checked_add(1)
        .ok_or("DMP host GPU vertex remap overflow")?;
    if remap_length > MAX_REMAP_ENTRIES {
        return Err("DMP host GPU vertex remap exceeds bound".into());
    }
    let mut vertex_remap = vec![0; remap_length];
    for (row, vertex) in unique.iter().copied().enumerate() {
        vertex_remap[vertex as usize] =
            u32::try_from(row).map_err(|_| "DMP host GPU vertex row overflow")?;
    }

    let mut groups = Vec::new();
    let mut slot_sources = [None; 8];
    for (group_index, group) in state.vertex.groups.iter().enumerate() {
        let count = group.attribute_count as usize;
        if count == 0 {
            groups.push(None);
            continue;
        }
        if count > 8 {
            return Err("DMP host GPU fetch group exceeds slot count".into());
        }
        if group.stride == 0 {
            return Err("DMP host GPU zero vertex fetch stride".into());
        }
        let mut at = 0usize;
        for item in 0..count {
            let slot = ((group.order >> (item * 4)) & 15) as usize;
            if slot >= 12 {
                at = at
                    .checked_add(3)
                    .ok_or("DMP host GPU attribute offset overflow")?
                    & !3;
                at = at
                    .checked_add((slot - 11) * 4)
                    .ok_or("DMP host GPU attribute offset overflow")?;
                continue;
            }
            if slot >= 8 {
                return Err("DMP host GPU unsupported attribute format bank".into());
            }
            let format = state.vertex.formats[slot];
            let width = format.bytes_per_component();
            at = at
                .checked_add(width - 1)
                .ok_or("DMP host GPU attribute offset overflow")?
                & !(width - 1);
            let source = SlotSource {
                group: group_index,
                byte_offset: at,
                format,
            };
            at = at
                .checked_add(format.components as usize * width)
                .ok_or("DMP host GPU attribute extent overflow")?;
            if at > group.stride {
                return Err("DMP host GPU vertex fetch exceeds stride".into());
            }
            slot_sources[slot] = Some(source);
        }
        groups.push(Some(GroupPlan {
            byte_offset: 0,
            stride: group.stride,
        }));
    }

    let unique = unique.into_iter().collect::<Vec<_>>();
    let mut bytes = Vec::new();
    for (group_index, plan) in groups.iter_mut().enumerate() {
        let Some(plan) = plan else {
            continue;
        };
        bytes.resize((bytes.len() + 3) & !3, 0);
        plan.byte_offset = bytes.len();
        let group_bytes = unique
            .len()
            .checked_mul(plan.stride)
            .ok_or("DMP host GPU vertex group byte length overflow")?;
        let end = bytes
            .len()
            .checked_add(group_bytes)
            .ok_or("DMP host GPU vertex buffer length overflow")?;
        if end > MAX_VERTEX_BYTES {
            return Err("DMP host GPU vertex buffer exceeds bound".into());
        }
        bytes.resize(end, 0);
        for (row, vertex) in unique.iter().copied().enumerate() {
            let address = state
                .vertex
                .base_address
                .checked_add(state.vertex.groups[group_index].byte_offset)
                .and_then(|base| {
                    (vertex as u64)
                        .checked_mul(plan.stride as u64)
                        .and_then(|offset| base.checked_add(offset))
                })
                .ok_or("DMP host GPU vertex address overflow")?;
            let row_start = plan.byte_offset + row * plan.stride;
            let row_end = row_start + plan.stride;
            let fetched = read(address, plan.stride)?;
            if fetched.len() != plan.stride {
                return Err("DMP host GPU short vertex DMA".into());
            }
            bytes[row_start..row_end].copy_from_slice(&fetched);
        }
    }
    if bytes.is_empty() {
        bytes.resize(4, 0);
    } else {
        bytes.resize((bytes.len() + 3) & !3, 0);
    }

    let mut mapped_slots = [None; 16];
    for (slot, input) in state.vertex.input_map.iter().copied().enumerate() {
        mapped_slots[input as usize] = Some(slot);
    }
    let mut attributes = [[0; 4]; 16];
    let mut defaults = [[0.0, 0.0, 0.0, 1.0]; 16];
    for register in 0..16 {
        let Some(slot) = mapped_slots[register] else {
            continue;
        };
        if let Some(source) = slot_sources[slot] {
            let plan = groups[source.group]
                .as_ref()
                .ok_or("DMP host GPU attribute group missing")?;
            let base = plan
                .byte_offset
                .checked_add(source.byte_offset)
                .ok_or("DMP host GPU attribute base overflow")?;
            attributes[register] = [
                u32::try_from(base).map_err(|_| "DMP host GPU attribute base exceeds u32")?,
                u32::try_from(plan.stride).map_err(|_| "DMP host GPU stride exceeds u32")?,
                ((source.format.components as u32) << 16)
                    | (attribute_type(source.format.kind) << 8),
                0,
            ];
        } else {
            defaults[register] = engine.attributes[slot];
        }
    }

    Ok(PackedVertices {
        bytes,
        draw_indices: draw_indices.to_vec(),
        vertex_remap,
        attributes,
        defaults,
    })
}

fn attribute_type(kind: super::draw_state::AttributeKind) -> u32 {
    match kind {
        super::draw_state::AttributeKind::Signed8 => 0,
        super::draw_state::AttributeKind::Unsigned8 => 1,
        super::draw_state::AttributeKind::Signed16 => 2,
        super::draw_state::AttributeKind::Float32 => 3,
    }
}

fn run_packed(
    engine: &Engine,
    state: &DrawState,
    packed: PackedVertices,
) -> Result<Vec<[Vector; OUTPUT_REGISTERS]>, String> {
    if packed.draw_indices.is_empty() {
        return Ok(Vec::new());
    }
    let semantics = vertex_semantics(state);
    let context = crate::gpu::context()?;
    if context.adapter_device_type() == wgpu::DeviceType::Cpu {
        return Err("DMP host GPU adapter: a hardware adapter is required".into());
    }
    let _submission = context.acquire()?;
    let mut gpu = GPU
        .get_or_init(|| Mutex::new(Gpu::new()))
        .lock()
        .map_err(|_| "DMP host GPU vertex lock poisoned")?;
    gpu.execute(context, engine, state, &semantics, packed)
}

fn vertex_semantics(state: &DrawState) -> [u32; VERTEX_OUTPUT_SEMANTICS] {
    let mut semantics = [u32::MAX; VERTEX_OUTPUT_SEMANTICS];
    for (register, vector) in state.vertex.output_map.0.iter().enumerate() {
        for (component, semantic) in vector.iter().copied().enumerate() {
            let semantic = semantic as usize;
            if semantic < semantics.len() {
                semantics[semantic] = (register * 4 + component) as u32;
            }
        }
    }
    semantics
}

struct Gpu {
    pipelines: HashMap<[u8; 32], wgpu::ComputePipeline>,
}

impl Gpu {
    fn new() -> Self {
        Self {
            pipelines: HashMap::new(),
        }
    }

    fn execute(
        &mut self,
        context: &crate::gpu::GpuContext,
        engine: &Engine,
        state: &DrawState,
        semantics: &[u32; VERTEX_OUTPUT_SEMANTICS],
        packed: PackedVertices,
    ) -> Result<Vec<[Vector; OUTPUT_REGISTERS]>, String> {
        let device = context.device();
        let queue = context.queue();
        let key = pipeline_key(engine, semantics);
        if !self.pipelines.contains_key(&key) {
            let mut source = translate_vertex_to_wgsl(
                &engine.code,
                &engine.operands,
                engine.reg(0x48a) & 0xffff,
                semantics,
            )?;
            source.push_str(COMPUTE_ENTRY);
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("DMP translated vertex program"),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("DMP translated vertex program"),
                layout: None,
                module: &shader,
                entry_point: Some("capture_vertex"),
                compilation_options: Default::default(),
                cache: None,
            });
            self.pipelines.insert(key, pipeline);
        }

        let shading_bytes = shading_bytes(engine, state, packed.attributes, packed.defaults);
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("DMP vertex uniforms"),
            contents: &shading_bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let input_buffer = storage_buffer(device, "DMP vertex attributes", &packed.bytes);
        let draw_index_bytes = words_to_bytes(&packed.draw_indices);
        let draw_index_buffer =
            storage_buffer(device, "DMP vertex draw indices", &draw_index_bytes);
        let remap_bytes = words_to_bytes(&packed.vertex_remap);
        let remap_buffer = storage_buffer(device, "DMP vertex index remap", &remap_bytes);
        let output_size = u64::try_from(packed.draw_indices.len())
            .ok()
            .and_then(|count| count.checked_mul((OUTPUT_WORDS * 4) as u64))
            .ok_or("DMP host GPU vertex output size overflow")?;
        if output_size > u64::from(device.limits().max_storage_buffer_binding_size) {
            return Err("DMP host GPU vertex output exceeds storage binding limit".into());
        }
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DMP vertex register outputs"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DMP vertex register readback"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pipeline = self
            .pipelines
            .get(&key)
            .ok_or("DMP host GPU vertex pipeline missing")?;
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DMP vertex program buffers"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: input_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: draw_index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: remap_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DMP vertex program pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            pass.dispatch_workgroups((packed.draw_indices.len() as u32).div_ceil(64), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, output_size);
        queue.submit([encoder.finish()]);

        let (sender, receiver) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| format!("DMP host GPU vertex polling: {error}"))?;
        receiver
            .recv()
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        let bytes = readback
            .slice(..)
            .get_mapped_range()
            .map_err(|error| error.to_string())?;
        let words = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|word| u32::from_le_bytes(*word))
            .collect::<Vec<_>>();
        drop(bytes);
        readback.unmap();

        let mut vertices = Vec::with_capacity(packed.draw_indices.len());
        for (vertex, output) in words.chunks_exact(OUTPUT_WORDS).enumerate() {
            if output[VALID_WORD] == 0 {
                return Err(format!(
                    "DMP host GPU refused vertex {vertex} for the translated program"
                ));
            }
            let registers = std::array::from_fn(|register| {
                let at = register * 4;
                std::array::from_fn(|component| f32::from_bits(output[at + component]))
            });
            vertices.push(registers);
        }
        Ok(vertices)
    }
}

fn storage_buffer(device: &wgpu::Device, label: &str, bytes: &[u8]) -> wgpu::Buffer {
    let mut padded = bytes.to_vec();
    padded.resize((padded.len() + 3) & !3, 0);
    if padded.is_empty() {
        padded.resize(4, 0);
    }
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: &padded,
        usage: wgpu::BufferUsages::STORAGE,
    })
}

fn words_to_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn shading_bytes(
    engine: &Engine,
    state: &DrawState,
    attributes: [[u32; 4]; 16],
    defaults: [[f32; 4]; 16],
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(2256);
    for slot in 0..96 {
        let uniform = engine.uniforms.get(slot).copied().unwrap_or([0; 4]);
        for word in uniform {
            push_u32(&mut bytes, word);
        }
    }
    for index in 0..4 {
        let packed = engine.reg(0x481 + index as u16);
        for value in [
            (packed & 255) as i32,
            (packed >> 8 & 255) as i32,
            ((packed >> 16 & 255) as u8 as i8) as i32,
            (packed >> 24 & 255) as i32,
        ] {
            push_u32(&mut bytes, value as u32);
        }
    }
    push_u32(&mut bytes, engine.reg(0x480));
    push_u32(&mut bytes, engine.reg(0x48a) & 0xffff);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    for semantic in vertex_semantics(state) {
        push_u32(&mut bytes, semantic);
    }
    push_u32(&mut bytes, super::float24(engine.reg(0x24d)).to_bits());
    push_u32(&mut bytes, super::float24(engine.reg(0x24e)).to_bits());
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    let viewport = [
        state.viewport.x_origin as f32,
        state.viewport.y_origin as f32,
        super::float24(engine.reg(0x241)) * 2.0,
        super::float24(engine.reg(0x243)) * 2.0,
    ];
    for value in viewport {
        push_u32(&mut bytes, value.to_bits());
    }
    for field in attributes {
        for value in field {
            push_u32(&mut bytes, value);
        }
    }
    for vector in defaults {
        for value in vector {
            push_u32(&mut bytes, value.to_bits());
        }
    }
    debug_assert_eq!(bytes.len(), 2256);
    bytes
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn pipeline_key(engine: &Engine, semantics: &[u32; VERTEX_OUTPUT_SEMANTICS]) -> [u8; 32] {
    let mut hash = Sha256::new();
    for words in [&engine.code, &engine.operands] {
        hash.update((words.len() as u64).to_le_bytes());
        for word in words {
            hash.update(word.to_le_bytes());
        }
    }
    hash.update((engine.reg(0x48a) & 0xffff).to_le_bytes());
    for semantic in semantics {
        hash.update(semantic.to_le_bytes());
    }
    hash.finalize().into()
}

const COMPUTE_ENTRY: &str = r#"
@group(0) @binding(8) var<storage, read> draw_indices: array<u32>;
@group(0) @binding(9) var<storage, read> vertex_remap: array<u32>;
@group(0) @binding(10) var<storage, read_write> captures: array<u32>;

@compute @workgroup_size(64)
fn capture_vertex(@builtin(global_invocation_id) global_id: vec3<u32>) {
    if (global_id.x >= arrayLength(&draw_indices)) {
        return;
    }
    gl_VertexIndex_1 = vertex_remap[draw_indices[global_id.x]];
    main_1();
    let base = global_id.x * 68u;
    for (var i = 0u; i < 16u; i = i + 1u) {
        let value = bitcast<vec4<u32>>(outputs[i]);
        captures[base + i * 4u] = value.x;
        captures[base + i * 4u + 1u] = value.y;
        captures[base + i * 4u + 2u] = value.z;
        captures[base + i * 4u + 3u] = value.w;
    }
    captures[base + 64u] = out_valid;
}
"#;

static GPU: OnceLock<Mutex<Gpu>> = OnceLock::new();

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn mov(destination: u32, source: u32) -> u32 {
        (0x13 << 26) | (destination << 21) | (source << 12)
    }

    fn test_engine() -> Engine {
        let mut engine = Engine::default();
        engine.code = vec![mov(0, 0), mov(1, 1), mov(2, 2), 0x22 << 26];
        engine.operands = vec![0x8d86_c36f];
        engine.regs.insert(0x48a, 0);
        engine.regs.insert(0x400, 0x200);
        engine.regs.insert(0x401, 9 | (3 << 4));
        engine.regs.insert(0x403, 0);
        engine.regs.insert(0x404, 0x1c0);
        engine.regs.insert(0x405, (3 << 28) | (12 << 16));
        engine.regs.insert(0x427, 0x8000_0020);
        engine.regs.insert(0x428, 3);
        engine.regs.insert(0x48b, 0x210);
        engine.attributes[2] = [8.0, 9.0, 10.0, 1.0];
        engine
    }

    fn vertex_memory() -> BTreeMap<u64, Vec<u8>> {
        let mut memory = BTreeMap::new();
        memory.insert(0x1020, vec![2, 0, 0, 0, 1, 0]);
        for (index, color, scalar) in [
            (0, [1, 2, 3], 1.25_f32),
            (1, [4, 5, 6], -2.5_f32),
            (2, [7, 8, 9], 3.75_f32),
        ] {
            let mut bytes = vec![color[0], color[1], color[2], 0, 0, 0, 0, 0];
            bytes.extend_from_slice(&scalar.to_le_bytes());
            memory.insert(0x1000 + index * 12, bytes);
        }
        memory
    }

    #[test]
    fn indexed_typed_fetch_matches_interpreter_registers() {
        let engine = test_engine();
        let state = engine.draw_state();
        let memory = vertex_memory();
        let mut read = |address, length| {
            memory
                .get(&address)
                .filter(|bytes| bytes.len() == length)
                .cloned()
                .ok_or_else(|| format!("missing test guest range {address:#x}+{length:#x}"))
        };
        let actual = match run_draw(&engine, &state, 3, true, &mut read) {
            Ok(actual) => actual,
            Err(error) if error.starts_with("DMP host GPU adapter:") => {
                eprintln!("skipping DMP vertex conformance without an adapter: {error}");
                return;
            }
            Err(error) => panic!("GPU vertex runner failed: {error}"),
        };
        let expected = [2, 0, 1].map(|vertex| {
            let mut fetch = |address, length| {
                memory
                    .get(&address)
                    .filter(|bytes| bytes.len() == length)
                    .cloned()
                    .ok_or_else(|| format!("missing test guest range {address:#x}+{length:#x}"))
            };
            let inputs = engine.fetch(vertex, &state, &mut fetch).unwrap();
            super::super::shader::execute(&engine, &inputs).unwrap()
        });
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(
                actual.map(|register| register.map(f32::to_bits)),
                expected.map(|register| register.map(f32::to_bits))
            );
        }
    }
}

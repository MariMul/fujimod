//! Immutable per-list upload arenas. Draws bind aligned slices; identical
//! texture snapshots and vertex uniforms share bytes without another upload.
use super::{PreparedDraw, encode_words};
use std::{collections::HashMap, ops::Range, sync::Arc};
use wgpu::util::DeviceExt;

#[derive(Default)]
struct Arena<'a> {
    bytes: Vec<u8>,
    shared: HashMap<&'a [u8], Range<u64>>,
    textures: HashMap<usize, Range<u64>>,
}
impl<'a> Arena<'a> {
    fn append(&mut self, bytes: &[u8], alignment: u32) -> Range<u64> {
        let start = self.bytes.len().next_multiple_of(alignment as usize);
        self.bytes.resize(start, 0);
        self.bytes.extend_from_slice(bytes);
        start as u64..self.bytes.len() as u64
    }
    fn shared(&mut self, bytes: &'a [u8], alignment: u32) -> Range<u64> {
        if let Some(range) = self.shared.get(bytes) {
            return range.clone();
        }
        let range = self.append(bytes, alignment);
        self.shared.insert(bytes, range.clone());
        range
    }
    fn texture(&mut self, bytes: &Arc<Vec<u8>>, alignment: u32) -> Range<u64> {
        // Prepared draws retain every interned upload for this list. Identity
        // is stable until all arenas are uploaded, so repeated atlases need
        // neither another copy nor a full-content hash for every draw.
        let identity = Arc::as_ptr(bytes) as usize;
        if let Some(range) = self.textures.get(&identity) {
            return range.clone();
        }
        let range = self.append(bytes, alignment);
        self.textures.insert(identity, range.clone());
        range
    }
    fn upload(
        &self,
        device: &wgpu::Device,
        label: &str,
        usage: wgpu::BufferUsages,
    ) -> Result<wgpu::Buffer, String> {
        if self.bytes.len() as u64 > device.limits().max_buffer_size {
            return Err(format!(
                "DMP {label} upload arena exceeds host buffer bound"
            ));
        }
        Ok(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: &self.bytes,
                usage,
            }),
        )
    }
}

struct Ranges {
    parameters: Range<u64>,
    guest: Range<u64>,
    uniforms: Range<u64>,
    inputs: Range<u64>,
    indices: Range<u64>,
}

pub(super) struct DrawBuffers {
    parameters: wgpu::Buffer,
    guest: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    inputs: wgpu::Buffer,
    indices: wgpu::Buffer,
    ranges: Vec<Ranges>,
}
impl DrawBuffers {
    pub(super) fn new(device: &wgpu::Device, draws: &[PreparedDraw]) -> Result<Self, String> {
        let limits = device.limits();
        let storage = limits.min_storage_buffer_offset_alignment;
        let uniform = limits.min_uniform_buffer_offset_alignment;
        let mut parameters = Arena::default();
        let mut guest = Arena::default();
        let mut uniforms = Arena::default();
        let mut inputs = Arena::default();
        let mut indices = Arena::default();
        let mut ranges = Vec::with_capacity(draws.len());
        for draw in draws {
            ranges.push(Ranges {
                parameters: parameters.append(&encode_words(&draw.parameters), storage),
                guest: guest.texture(&draw.guest_bytes, storage),
                uniforms: uniforms.shared(&draw.vertex.shading_bytes, uniform),
                inputs: inputs.append(&draw.vertex.input_bytes, storage),
                indices: indices.append(&encode_words(&draw.vertex.indices), 4),
            });
        }
        Ok(Self {
            parameters: parameters.upload(
                device,
                "draw parameters",
                wgpu::BufferUsages::STORAGE,
            )?,
            guest: guest.upload(device, "draw textures", wgpu::BufferUsages::STORAGE)?,
            uniforms: uniforms.upload(device, "vertex uniforms", wgpu::BufferUsages::UNIFORM)?,
            inputs: inputs.upload(device, "vertex attributes", wgpu::BufferUsages::STORAGE)?,
            indices: indices.upload(device, "vertex indices", wgpu::BufferUsages::INDEX)?,
            ranges,
        })
    }
    pub(super) fn parameters(&self, index: usize) -> wgpu::BindingResource<'_> {
        binding(&self.parameters, &self.ranges[index].parameters)
    }
    pub(super) fn guest(&self, index: usize) -> wgpu::BindingResource<'_> {
        binding(&self.guest, &self.ranges[index].guest)
    }
    pub(super) fn uniforms(&self, index: usize) -> wgpu::BindingResource<'_> {
        binding(&self.uniforms, &self.ranges[index].uniforms)
    }
    pub(super) fn inputs(&self, index: usize) -> wgpu::BindingResource<'_> {
        binding(&self.inputs, &self.ranges[index].inputs)
    }
    pub(super) fn indices(&self, index: usize) -> wgpu::BufferSlice<'_> {
        self.indices.slice(self.ranges[index].indices.clone())
    }
}
fn binding<'a>(buffer: &'a wgpu::Buffer, range: &Range<u64>) -> wgpu::BindingResource<'a> {
    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
        buffer,
        offset: range.start,
        size: std::num::NonZeroU64::new(range.end - range.start),
    })
}

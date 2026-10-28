//! GPU-resident DMP surfaces with exact Morton-ordered guest-memory transfers.
//!
//! Surface texels retain the packed guest pixel bits in an `r32uint` texture.
//! The format stays in the cache key so later draw pipelines can decode the
//! same color and depth formats without losing guest bytes during upload or
//! readback. A second texture exposes the resolved per-byte write mask; its
//! atomic accumulator lets multiple draws contribute disjoint byte writes.

use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, OnceLock},
    task::{Context, Poll, Wake},
};

use wgpu::util::DeviceExt;

const TILE_SIDE: u32 = 8;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SurfaceFormat {
    Rgba8,
    Rgb565,
    Rgb5A1,
    Rgba4,
    D16,
    D24,
    D24S8,
}

impl SurfaceFormat {
    /// DMP color target register values: 0 RGBA8, 1 RGB565, 2 RGB5A1, 3 RGBA4.
    pub fn from_color_code(code: u32) -> Result<Self, String> {
        match code {
            0 => Ok(Self::Rgba8),
            1 => Ok(Self::Rgb565),
            2 => Ok(Self::Rgb5A1),
            3 => Ok(Self::Rgba4),
            value => Err(format!("DMP unsupported color surface format {value}")),
        }
    }

    /// DMP depth target register values: 0 D16, 2 D24, 3 D24S8.
    pub fn from_depth_code(code: u32) -> Result<Self, String> {
        match code {
            0 => Ok(Self::D16),
            2 => Ok(Self::D24),
            3 => Ok(Self::D24S8),
            value => Err(format!("DMP unsupported depth surface format {value}")),
        }
    }

    pub const fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::Rgba8 | Self::D24S8 => 4,
            Self::D24 => 3,
            Self::Rgb565 | Self::Rgb5A1 | Self::Rgba4 | Self::D16 => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SurfaceExtent {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SurfaceKey {
    pub guest_address: u64,
    pub format: SurfaceFormat,
    pub extent: SurfaceExtent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelWriteMask {
    pub x: u32,
    pub y: u32,
    /// Bit `n` marks guest byte lane `n` within this pixel as written.
    pub byte_mask: u32,
}

pub struct SurfaceReadback {
    /// Packed guest bytes in Morton order, suitable for writing at the key's address.
    pub bytes: Vec<u8>,
    /// Sorted, contiguous byte ranges written since the last upload or clear.
    pub written_ranges: Vec<(u64, usize)>,
}

pub struct GpuSurface {
    key: SurfaceKey,
    texture: wgpu::Texture,
    write_mask_texture: wgpu::Texture,
    write_mask_accumulator: wgpu::Buffer,
    pixel_count: usize,
    byte_len: usize,
}

impl GpuSurface {
    pub fn key(&self) -> SurfaceKey {
        self.key
    }

    /// Packed guest pixel values, one `r32uint` texel per linear image pixel.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// Resolved per-pixel byte masks. Bit `n` marks byte lane `n` as written.
    /// Call [`SurfaceCache::readback`] to resolve pending atomic writes first.
    pub fn write_mask_texture(&self) -> &wgpu::Texture {
        &self.write_mask_texture
    }

    /// Atomic byte-mask accumulator indexed by `y * width + x`.
    ///
    /// Draw shaders can bind this as `array<atomic<u32>>` and use
    /// `atomicOr(&mask[y * width + x], byte_mask)` to preserve writes across
    /// overlapping draws. The cache resolves it into `write_mask_texture`
    /// before readback.
    pub fn write_mask_accumulator(&self) -> &wgpu::Buffer {
        &self.write_mask_accumulator
    }

    pub fn byte_len(&self) -> usize {
        self.byte_len
    }
}

pub struct SurfaceCache {
    gpu: Arc<Gpu>,
    surfaces: HashMap<SurfaceKey, GpuSurface>,
}

impl SurfaceCache {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            gpu: shared_gpu()?,
            surfaces: HashMap::new(),
        })
    }

    /// Return a resident surface when present, uploading `guest_bytes` only on
    /// the first use of this address, format, and extent tuple.
    pub fn get_or_upload(
        &mut self,
        key: SurfaceKey,
        guest_bytes: &[u8],
    ) -> Result<&GpuSurface, String> {
        if !self.surfaces.contains_key(&key) {
            self.upload(key, guest_bytes)?;
        }
        Ok(self.surfaces.get(&key).expect("surface inserted above"))
    }

    /// Upload guest Morton bytes into the resident texture, replacing an
    /// existing cache entry when its guest pages have changed.
    pub fn upload(&mut self, key: SurfaceKey, guest_bytes: &[u8]) -> Result<(), String> {
        let (pixel_count, byte_len) = validate_key(key, &self.gpu)?;
        if guest_bytes.len() != byte_len {
            return Err(format!(
                "DMP surface upload has {} bytes, expected {byte_len}",
                guest_bytes.len()
            ));
        }
        if !self.surfaces.contains_key(&key) {
            let surface = create_surface(key, pixel_count, byte_len, &self.gpu);
            self.surfaces.insert(key, surface);
        }
        let surface = self.surfaces.get(&key).expect("surface inserted above");
        self.gpu.upload(surface, guest_bytes)
    }

    pub fn get(&self, key: SurfaceKey) -> Option<&GpuSurface> {
        self.surfaces.get(&key)
    }

    pub fn len(&self) -> usize {
        self.surfaces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }

    pub fn evict(&mut self, key: SurfaceKey) -> bool {
        self.surfaces.remove(&key).is_some()
    }

    /// Atomically add byte writes for any pixels. Duplicate coordinates are
    /// ORed, so disjoint channel or depth/stencil writes remain exact.
    pub fn record_write_masks(
        &self,
        key: SurfaceKey,
        writes: &[PixelWriteMask],
    ) -> Result<(), String> {
        if writes.is_empty() {
            return Ok(());
        }
        let surface = self
            .surfaces
            .get(&key)
            .ok_or("DMP surface is not resident")?;
        let bytes_per_pixel = key.format.bytes_per_pixel();
        let valid_mask = (1 << bytes_per_pixel) - 1;
        let record_bytes = writes
            .len()
            .checked_mul(12)
            .ok_or("DMP write-mask record size overflow")?;
        let limits = self.gpu.device.limits();
        if writes.len() > u32::MAX as usize
            || record_bytes > limits.max_storage_buffer_binding_size as usize
            || (writes.len() as u32).div_ceil(64) > limits.max_compute_workgroups_per_dimension
        {
            return Err("DMP write-mask batch exceeds host GPU limits".into());
        }
        let mut records = Vec::with_capacity(record_bytes);
        for write in writes {
            if write.x >= key.extent.width || write.y >= key.extent.height {
                return Err("DMP write-mask coordinate is outside the surface".into());
            }
            if write.byte_mask & !valid_mask != 0 {
                return Err("DMP write mask exceeds the surface pixel format".into());
            }
            records.extend_from_slice(&write.x.to_le_bytes());
            records.extend_from_slice(&write.y.to_le_bytes());
            records.extend_from_slice(&write.byte_mask.to_le_bytes());
        }
        self.gpu.record_write_masks(surface, &records, writes.len())
    }

    /// Clear the byte-write tracker without changing the resident pixels.
    pub fn clear_write_masks(&self, key: SurfaceKey) -> Result<(), String> {
        let surface = self
            .surfaces
            .get(&key)
            .ok_or("DMP surface is not resident")?;
        self.gpu.clear_write_masks(surface)
    }

    /// Convert resident pixels back to Morton guest bytes and exact byte ranges.
    pub fn readback(&self, key: SurfaceKey) -> Result<SurfaceReadback, String> {
        let surface = self
            .surfaces
            .get(&key)
            .ok_or("DMP surface is not resident")?;
        self.gpu.readback(surface)
    }
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    upload_pipeline: wgpu::ComputePipeline,
    clear_mask_pipeline: wgpu::ComputePipeline,
    record_mask_pipeline: wgpu::ComputePipeline,
    resolve_mask_pipeline: wgpu::ComputePipeline,
    readback_pipeline: wgpu::ComputePipeline,
}

impl Gpu {
    fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .map_err(|error| format!("DMP host GPU adapter: {error}"))?;
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|error| format!("DMP host GPU device: {error}"))?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DMP resident surfaces"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_surfaces.wgsl").into()),
        });
        let pipeline = |label, entry_point| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: None,
                module: &shader,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let upload_pipeline = pipeline("DMP Morton upload", "upload_morton");
        let clear_mask_pipeline = pipeline("DMP write-mask clear", "clear_write_mask");
        let record_mask_pipeline = pipeline("DMP write-mask record", "record_write_masks");
        let resolve_mask_pipeline = pipeline("DMP write-mask resolve", "resolve_write_mask");
        let readback_pipeline = pipeline("DMP Morton readback", "readback_morton");
        Ok(Self {
            device,
            queue,
            upload_pipeline,
            clear_mask_pipeline,
            record_mask_pipeline,
            resolve_mask_pipeline,
            readback_pipeline,
        })
    }

    fn upload(&self, surface: &GpuSurface, guest_bytes: &[u8]) -> Result<(), String> {
        let guest = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("DMP Morton guest bytes"),
                contents: guest_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let params = self.params(surface, 0);
        let upload_bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DMP surface upload bindings"),
            layout: &self.upload_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: guest.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &surface.texture.create_view(&Default::default()),
                    ),
                },
            ],
        });
        let clear_bindings = self.clear_mask_bindings(surface, &params);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        dispatch_2d(
            &mut encoder,
            &self.upload_pipeline,
            &upload_bindings,
            surface.key.extent,
            "DMP Morton upload",
        );
        dispatch_2d(
            &mut encoder,
            &self.clear_mask_pipeline,
            &clear_bindings,
            surface.key.extent,
            "DMP write-mask clear",
        );
        self.queue.submit([encoder.finish()]);
        Ok(())
    }

    fn record_write_masks(
        &self,
        surface: &GpuSurface,
        records: &[u8],
        count: usize,
    ) -> Result<(), String> {
        let record_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("DMP pixel byte-write masks"),
                contents: records,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let params = self.params(surface, count as u32);
        let bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DMP write-mask record bindings"),
            layout: &self.record_mask_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: record_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: surface.write_mask_accumulator.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.record_mask_pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            pass.dispatch_workgroups((count as u32).div_ceil(64), 1, 1);
        }
        self.queue.submit([encoder.finish()]);
        Ok(())
    }

    fn clear_write_masks(&self, surface: &GpuSurface) -> Result<(), String> {
        let params = self.params(surface, 0);
        let bindings = self.clear_mask_bindings(surface, &params);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        dispatch_2d(
            &mut encoder,
            &self.clear_mask_pipeline,
            &bindings,
            surface.key.extent,
            "DMP write-mask clear",
        );
        self.queue.submit([encoder.finish()]);
        Ok(())
    }

    fn readback(&self, surface: &GpuSurface) -> Result<SurfaceReadback, String> {
        let result_size = surface
            .pixel_count
            .checked_mul(8)
            .ok_or("DMP surface readback size overflow")? as u64;
        let result = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DMP Morton surface result"),
            size: result_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let download = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DMP Morton readback staging"),
            size: result_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params = self.params(surface, 0);
        let resolve_bindings = self.resolve_mask_bindings(surface, &params);
        let readback_bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DMP surface readback bindings"),
            layout: &self.readback_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(
                        &surface.texture.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(
                        &surface.write_mask_texture.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: result.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        dispatch_2d(
            &mut encoder,
            &self.resolve_mask_pipeline,
            &resolve_bindings,
            surface.key.extent,
            "DMP write-mask resolve",
        );
        dispatch_2d(
            &mut encoder,
            &self.readback_pipeline,
            &readback_bindings,
            surface.key.extent,
            "DMP Morton readback",
        );
        encoder.copy_buffer_to_buffer(&result, 0, &download, 0, result_size);
        self.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        download
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |status| {
                let _ = sender.send(status);
            });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| format!("DMP host GPU polling: {error}"))?;
        receiver
            .recv()
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        let mapped = download
            .slice(..)
            .get_mapped_range()
            .map_err(|error| error.to_string())?;
        let mut bytes = Vec::with_capacity(surface.byte_len);
        let mut byte_flags = Vec::with_capacity(surface.byte_len);
        let bytes_per_pixel = surface.key.format.bytes_per_pixel() as usize;
        for record in mapped.chunks_exact(8) {
            let pixel = u32::from_le_bytes(record[..4].try_into().unwrap());
            let mask = u32::from_le_bytes(record[4..].try_into().unwrap());
            for lane in 0..bytes_per_pixel {
                bytes.push((pixel >> (lane * 8)) as u8);
                byte_flags.push(mask & (1 << lane) != 0);
            }
        }
        drop(mapped);
        download.unmap();
        let written_ranges = byte_ranges(surface.key.guest_address, &byte_flags);
        Ok(SurfaceReadback {
            bytes,
            written_ranges,
        })
    }

    fn params(&self, surface: &GpuSurface, write_count: u32) -> wgpu::Buffer {
        let words = [
            surface.key.extent.width,
            surface.key.extent.height,
            surface.key.extent.width,
            surface.key.extent.height,
            surface.key.extent.width / TILE_SIDE,
            surface.key.format.bytes_per_pixel(),
            write_count,
            0,
        ];
        let contents: Vec<u8> = words.into_iter().flat_map(u32::to_le_bytes).collect();
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("DMP surface parameters"),
                contents: &contents,
                usage: wgpu::BufferUsages::UNIFORM,
            })
    }

    fn clear_mask_bindings(&self, surface: &GpuSurface, params: &wgpu::Buffer) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DMP write-mask clear bindings"),
            layout: &self.clear_mask_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: surface.write_mask_accumulator.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(
                        &surface.write_mask_texture.create_view(&Default::default()),
                    ),
                },
            ],
        })
    }

    fn resolve_mask_bindings(
        &self,
        surface: &GpuSurface,
        params: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DMP write-mask resolve bindings"),
            layout: &self.resolve_mask_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: surface.write_mask_accumulator.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(
                        &surface.write_mask_texture.create_view(&Default::default()),
                    ),
                },
            ],
        })
    }
}

fn create_surface(key: SurfaceKey, pixel_count: usize, byte_len: usize, gpu: &Gpu) -> GpuSurface {
    let texture_size = wgpu::Extent3d {
        width: key.extent.width,
        height: key.extent.height,
        depth_or_array_layers: 1,
    };
    let create_texture = |label| {
        gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    };
    let accumulator = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("DMP atomic byte-write masks"),
        size: (pixel_count * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    GpuSurface {
        key,
        texture: create_texture("DMP resident packed pixels"),
        write_mask_texture: create_texture("DMP resolved byte-write masks"),
        write_mask_accumulator: accumulator,
        pixel_count,
        byte_len,
    }
}

fn validate_key(key: SurfaceKey, gpu: &Gpu) -> Result<(usize, usize), String> {
    let extent = key.extent;
    if extent.width == 0 || extent.height == 0 {
        return Err("DMP surface extent must be nonzero".into());
    }
    if extent.width % TILE_SIDE != 0 || extent.height % TILE_SIDE != 0 {
        return Err("DMP Morton surface extent must be a multiple of 8 pixels".into());
    }
    if extent.width > gpu.device.limits().max_texture_dimension_2d
        || extent.height > gpu.device.limits().max_texture_dimension_2d
    {
        return Err("DMP surface exceeds the host GPU texture limit".into());
    }
    let pixel_count = (extent.width as usize)
        .checked_mul(extent.height as usize)
        .ok_or("DMP surface pixel count overflow")?;
    let byte_len = pixel_count
        .checked_mul(key.format.bytes_per_pixel() as usize)
        .ok_or("DMP surface byte count overflow")?;
    key.guest_address
        .checked_add(byte_len as u64)
        .ok_or("DMP surface guest address overflow")?;
    let storage_limit = gpu.device.limits().max_storage_buffer_binding_size as u64;
    if byte_len as u64 > storage_limit
        || (pixel_count as u64) * 8 > storage_limit
        || (pixel_count as u64) * 4 > storage_limit
    {
        return Err("DMP surface exceeds the host GPU storage-buffer limit".into());
    }
    Ok((pixel_count, byte_len))
}

fn dispatch_2d(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::ComputePipeline,
    bindings: &wgpu::BindGroup,
    extent: SurfaceExtent,
    label: &'static str,
) {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some(label),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bindings, &[]);
    pass.dispatch_workgroups(extent.width.div_ceil(8), extent.height.div_ceil(8), 1);
}

fn byte_ranges(address: u64, written: &[bool]) -> Vec<(u64, usize)> {
    let mut ranges = Vec::new();
    let mut offset = 0;
    while offset < written.len() {
        if !written[offset] {
            offset += 1;
            continue;
        }
        let start = offset;
        while offset < written.len() && written[offset] {
            offset += 1;
        }
        ranges.push((address + start as u64, offset - start));
    }
    ranges
}

fn shared_gpu() -> Result<Arc<Gpu>, String> {
    static GPU: OnceLock<Result<Arc<Gpu>, String>> = OnceLock::new();
    GPU.get_or_init(|| Gpu::new().map(Arc::new)).clone()
}

struct ThreadWake(std::thread::Thread);

impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Arc::new(ThreadWake(std::thread::current())).into();
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXTENT: SurfaceExtent = SurfaceExtent {
        width: 16,
        height: 8,
    };

    #[test]
    fn guest_upload_and_readback_are_exact_for_every_dmp_format() {
        let mut cache = SurfaceCache::new().expect("host GPU adapter");
        let formats = [
            SurfaceFormat::Rgba8,
            SurfaceFormat::Rgb565,
            SurfaceFormat::Rgb5A1,
            SurfaceFormat::Rgba4,
            SurfaceFormat::D16,
            SurfaceFormat::D24,
            SurfaceFormat::D24S8,
        ];
        for (index, format) in formats.into_iter().enumerate() {
            let key = SurfaceKey {
                guest_address: 0x10000 + index as u64 * 0x1000,
                format,
                extent: EXTENT,
            };
            let byte_len =
                EXTENT.width as usize * EXTENT.height as usize * format.bytes_per_pixel() as usize;
            let guest_bytes: Vec<u8> = (0..byte_len)
                .map(|offset| (offset.wrapping_mul(37).wrapping_add(index * 11)) as u8)
                .collect();
            cache.get_or_upload(key, &guest_bytes).unwrap();
            let readback = cache.readback(key).unwrap();
            assert_eq!(readback.bytes, guest_bytes, "{format:?}");
            assert!(readback.written_ranges.is_empty(), "{format:?}");
        }
    }

    #[test]
    fn write_mask_reports_exact_morton_guest_byte_ranges() {
        let mut cache = SurfaceCache::new().expect("host GPU adapter");
        let format = SurfaceFormat::D24S8;
        let key = SurfaceKey {
            guest_address: 0x24000,
            format,
            extent: EXTENT,
        };
        let guest_bytes = vec![0xA5; EXTENT.width as usize * EXTENT.height as usize * 4];
        cache.get_or_upload(key, &guest_bytes).unwrap();
        cache
            .record_write_masks(
                key,
                &[
                    PixelWriteMask {
                        x: 0,
                        y: 0,
                        byte_mask: 0b1111,
                    },
                    PixelWriteMask {
                        x: 1,
                        y: 0,
                        byte_mask: 0b0101,
                    },
                    PixelWriteMask {
                        x: 0,
                        y: 1,
                        byte_mask: 0b1111,
                    },
                ],
            )
            .unwrap();
        let readback = cache.readback(key).unwrap();
        assert_eq!(readback.bytes, guest_bytes);
        assert_eq!(
            readback.written_ranges,
            vec![
                (key.guest_address, 5),
                (key.guest_address + 6, 1),
                (key.guest_address + 8, 4)
            ]
        );
    }
}

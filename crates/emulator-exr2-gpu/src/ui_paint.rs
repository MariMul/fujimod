//! GPU paint and compositing for EXR2 vector UI batches.

use crate::{Inputs, RenderedBatch, batch};
use fujimod_emulator_devices::record::{Result, Unsupported};
use fujimod_emulator_gpu::{
    CoverageEdge, CoverageRequest, EdgeCoordinates, FillRule, SampleGrid, SurfaceExtent,
    SurfaceFormat, SurfaceKey, SurfaceLayout,
};
use fujimod_emulator_graphics::render;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    future::Future,
    task::{Context, Poll, Wake},
};
use wgpu::util::DeviceExt;

struct PaintKernel {
    pipeline: wgpu::ComputePipeline,
}

fn fail<T>(message: impl Into<String>) -> Result<T> {
    Err(Unsupported(message.into()))
}

fn kernel() -> Result<&'static PaintKernel> {
    static KERNEL: OnceLock<std::result::Result<PaintKernel, String>> = OnceLock::new();
    match KERNEL.get_or_init(|| {
        let context = fujimod_emulator_gpu::context()?;
        if !context.supports_read_write_r32uint_storage() {
            return Err("host GPU lacks native R32Uint read/write storage support".into());
        }
        let device = context.device();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("EXR2 vector GPU paint"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui_paint.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("EXR2 vector GPU paint"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Ok(PaintKernel { pipeline })
    }) {
        Ok(kernel) => Ok(kernel),
        Err(error) => fail(format!("Graphics UI GPU paint {error}")),
    }
}

fn image_key(texture: &batch::Texture) -> (u64, u32, u32) {
    (texture.source, texture.width, texture.height)
}

fn push_f32(words: &mut Vec<u32>, value: f64) -> Result<()> {
    let narrowed = value as f32;
    if !narrowed.is_finite() {
        return fail("Graphics UI paint coefficient exceeds the GPU float range");
    }
    words.push(narrowed.to_bits());
    Ok(())
}

fn paint_parameters(
    draw: &batch::Draw,
    plan: &render::CoveragePlan,
    raster_offset: u32,
) -> Result<Vec<u32>> {
    let (target_width, target_height, order) = (draw.target.1, draw.target.2, draw.target.3);
    let mode = if let Some(gradient) = &draw.gradient {
        if gradient.radial.is_some() { 3 } else { 2 }
    } else if draw.texture.is_some() {
        1
    } else {
        0
    };
    let texture_dimensions = draw
        .texture
        .as_ref()
        .map(|texture| [texture.width, texture.height])
        .unwrap_or([0; 2]);
    let valid_dimensions = draw
        .texture
        .as_ref()
        .map(|texture| [texture.valid_width, texture.valid_height])
        .unwrap_or([0; 2]);
    let mut words = Vec::with_capacity(40);
    words.extend([
        plan.x0 as u32,
        plan.y0 as u32,
        plan.width as u32,
        plan.height as u32,
        target_width,
        target_height,
        raster_offset,
        plan.samples * plan.samples,
        order,
        u32::from(draw.blend != 0),
        mode,
        texture_dimensions[0],
        texture_dimensions[1],
        valid_dimensions[0] as u32,
        valid_dimensions[1] as u32,
    ]);
    if let Some(texture) = &draw.texture {
        for value in texture.matrix {
            push_f32(&mut words, value)?;
        }
    } else {
        words.extend([0; 6]);
    }
    if let Some(gradient) = &draw.gradient {
        let (a, c, e, f) = gradient.coefficients;
        for value in [a, c, e, f] {
            push_f32(&mut words, value)?;
        }
        words.extend([gradient.spread, u32::from(gradient.premultiplied)]);
    } else {
        words.extend([0; 6]);
    }
    if let Some(radial) = draw
        .gradient
        .as_ref()
        .and_then(|gradient| gradient.radial.as_ref())
    {
        push_f32(&mut words, radial.quadratics[0])?;
        push_f32(&mut words, radial.quadratics[1])?;
        words.push(
            u32::from(radial.zero_x)
                | (u32::from(radial.zero_y) << 1)
                | (u32::from(radial.negative_denominator) << 2),
        );
    } else {
        words.extend([0; 3]);
    }
    words.push(u32::from(draw.color_transform.is_some()));
    if let Some(transform) = &draw.color_transform {
        for value in transform.scale.into_iter().chain(transform.bias) {
            push_f32(&mut words, value)?;
        }
    } else {
        words.extend([0; 8]);
    }
    words.push(u32::from_le_bytes(draw.color));
    if words.len() != 40 {
        return fail("Graphics UI GPU paint parameters have an invalid extent");
    }
    Ok(words)
}

fn paint_data(draw: &batch::Draw, image: Option<&[u8]>) -> Result<Vec<u32>> {
    if let Some(gradient) = &draw.gradient {
        if gradient.colors.len() != 128 {
            return fail("Graphics UI gradient paint does not contain 128 RGBA entries");
        }
        return Ok(gradient
            .colors
            .iter()
            .map(|rgba| {
                u32::from(rgba[0])
                    | (u32::from(rgba[1]) << 8)
                    | (u32::from(rgba[2]) << 16)
                    | (u32::from(rgba[3]) << 24)
            })
            .collect());
    }
    if let Some(texture) = &draw.texture {
        let bytes =
            image.ok_or_else(|| Unsupported("Graphics UI texture snapshot is missing".into()))?;
        let expected = texture.width as usize * texture.height as usize * 4;
        if bytes.len() != expected {
            return fail("Graphics UI texture paint snapshot is incomplete");
        }
        let words = bytes
            .chunks_exact(4)
            .map(|pixel| u32::from_le_bytes(pixel.try_into().unwrap()))
            .collect();
        return Ok(words);
    }
    Ok(vec![0])
}

/// Paint a complete EXR2 vector batch, keeping coverage and pixels on the GPU.
pub(super) async fn render_async(
    draws: &[batch::Draw],
    cpu: i64,
    host: &mut dyn Inputs,
) -> Result<RenderedBatch> {
    let context = fujimod_emulator_gpu::context()
        .map_err(|error| Unsupported(format!("Graphics UI GPU paint {error}")))?;
    let mut targets = Vec::<(SurfaceKey, Option<Vec<u8>>, Option<Vec<u64>>)>::new();
    let mut target_slots = HashMap::<u64, usize>::new();
    let mut texture_bytes = HashMap::<(u64, u32, u32), Vec<u8>>::new();
    for draw in draws {
        let (address, width, height, _) = draw.target;
        if let std::collections::hash_map::Entry::Vacant(entry) = target_slots.entry(address) {
            let length = (width as u64)
                .checked_mul(height as u64)
                .and_then(|count| count.checked_mul(4))
                .ok_or_else(|| Unsupported("Graphics UI target size overflow".into()))?;
            let key = SurfaceKey {
                guest_address: address,
                format: SurfaceFormat::Rgba8,
                extent: SurfaceExtent { width, height },
                layout: SurfaceLayout::Morton8x8,
            };
            let versions = host
                .page_versions(cpu, address, length as usize)?;
            let current = versions
                .as_deref()
                .map(|versions| context.target_is_current(key, versions))
                .transpose()
                .map_err(|error| Unsupported(format!("Graphics UI GPU target versions {error}")))?
                .unwrap_or(false);
            let bytes = if current {
                None
            } else {
                let bytes = host.read(cpu, address, length).map_err(Unsupported)?;
                if bytes.len() as u64 != length {
                    return fail("Graphics UI destination snapshot is incomplete");
                }
                Some(bytes)
            };
            entry.insert(targets.len());
            targets.push((key, bytes, versions));
        }
        if let Some(texture) = &draw.texture {
            let key = image_key(texture);
            if let std::collections::hash_map::Entry::Vacant(entry) = texture_bytes.entry(key) {
                let length = (texture.width as u64)
                    .checked_mul(texture.height as u64)
                    .and_then(|count| count.checked_mul(4))
                    .ok_or_else(|| Unsupported("Graphics UI texture size overflow".into()))?;
                let bytes = host
                    .read(cpu, texture.source, length)
                    .map_err(Unsupported)?;
                if bytes.len() as u64 != length {
                    return fail("Graphics UI texture snapshot is incomplete");
                }
                entry.insert(bytes);
            }
        }
    }

    let _submission = context
        .acquire()
        .map_err(|error| Unsupported(format!("Graphics UI GPU paint {error}")))?;
    for (key, bytes, versions) in &targets {
        context
            .get_or_create_target_versioned(*key, versions.as_deref(), bytes.as_deref())
            .map_err(|error| Unsupported(format!("Graphics UI GPU paint {error}")))?;
    }

    let plans = render::coverage_plans(draws)?;
    let mut readback_regions = HashMap::new();
    for (draw, plan) in draws.iter().zip(&plans) {
        if plan.is_empty() {
            continue;
        }
        let (left, top) = (plan.x0 as u32, plan.y0 as u32);
        let (right, bottom) = (
            (plan.x0 + plan.width as i64) as u32,
            (plan.y0 + plan.height as i64) as u32,
        );
        readback_regions
            .entry(draw.target.0)
            .and_modify(|region: &mut fujimod_emulator_gpu::SurfaceRegion| {
                let x1 = region.x + region.width;
                let y1 = region.y + region.height;
                let x0 = left.min(region.x);
                let y0 = top.min(region.y);
                *region = fujimod_emulator_gpu::SurfaceRegion {
                    x: x0,
                    y: y0,
                    width: right.max(x1) - x0,
                    height: bottom.max(y1) - y0,
                };
            })
            .or_insert(fujimod_emulator_gpu::SurfaceRegion {
                x: left,
                y: top,
                width: right - left,
                height: bottom - top,
            });
    }
    let target_snapshot_bytes = targets
        .iter()
        .filter_map(|(_, bytes, _)| bytes.as_ref())
        .map(|bytes| bytes.len() as u64)
        .sum::<u64>();
    let edge_sets: Vec<Vec<CoverageEdge>> = plans
        .iter()
        .map(|plan| {
            plan.edges
                .iter()
                .map(|&(start, end)| {
                    let coordinates = [start.0, start.1, end.0, end.1];
                    if coordinates
                        .iter()
                        .any(|coordinate| coordinate.abs() > 16_777_216.0)
                    {
                        return fail("Graphics UI fill edge exceeds the GPU coordinate range");
                    }
                    Ok(CoverageEdge::from_f64([start.0, start.1], [end.0, end.1]))
                })
                .collect::<Result<_>>()
        })
        .collect::<Result<_>>()?;
    let requests: Vec<_> = plans
        .iter()
        .zip(&edge_sets)
        .map(|(plan, edges)| {
            let sample_grid = match plan.samples {
                1 => SampleGrid::OneByOne,
                2 => SampleGrid::TwoByTwo,
                4 => SampleGrid::FourByFour,
                _ => return fail("Graphics UI uses an unsupported coverage sample grid"),
            };
            Ok(CoverageRequest {
                bounds: plan.bounds(),
                edges,
                edge_coordinates: EdgeCoordinates::FloatPixel,
                fill_rule: if plan.nonzero {
                    FillRule::NonZero
                } else {
                    FillRule::EvenOdd
                },
                sample_grid,
                stroke_radius: None,
            })
        })
        .collect::<Result<_>>()?;

    let device = context.device();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("EXR2 vector GPU coverage and paint"),
    });
    for (key, _, _) in &targets {
        context
            .encode_clear_write_masks(&mut encoder, *key)
            .map_err(|error| Unsupported(format!("Graphics UI GPU write masks {error}")))?;
    }
    let coverage = super::coverage_kernel()?
        .encode_batch(context, &mut encoder, &requests)
        .map_err(|error| Unsupported(format!("Graphics UI GPU coverage {error}")))?;
    let surfaces: Vec<_> = targets
        .iter()
        .map(|(key, _, _)| {
            context
                .get(*key)
                .map_err(|error| Unsupported(format!("Graphics UI GPU paint {error}")))?
                .ok_or_else(|| Unsupported("Graphics UI GPU target is not resident".into()))
        })
        .collect::<Result<_>>()?;
    let touched = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("EXR2 vector GPU touched-pixel count"),
        contents: &0u32.to_le_bytes(),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    });
    let touched_readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("EXR2 vector GPU touched-pixel readback"),
        size: 4,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    if coverage.total_count() != 0 {
        let paint = kernel()?;
        let coverage_buffer = coverage
            .buffer()
            .ok_or_else(|| Unsupported("Graphics UI GPU coverage buffer is missing".into()))?;
        for ((draw, plan), raster) in draws.iter().zip(&plans).zip(coverage.rasters()) {
            if plan.is_empty() || raster.length == 0 {
                continue;
            }
            let target_index = *target_slots
                .get(&draw.target.0)
                .ok_or_else(|| Unsupported("Graphics UI target lookup failed".into()))?;
            let surface = &surfaces[target_index];
            let image = draw
                .texture
                .as_ref()
                .and_then(|texture| texture_bytes.get(&image_key(texture)))
                .map(Vec::as_slice);
            let params = paint_parameters(draw, plan, raster.offset)?;
            let params_bytes = words_as_bytes(&params);
            let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("EXR2 vector GPU paint parameters"),
                contents: &params_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
            let clip_values: Vec<u32> = plan
                .clip_mask()
                .iter()
                .map(|&inside| u32::from(inside))
                .collect();
            let clip_bytes = words_as_bytes(&clip_values);
            let clip_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("EXR2 vector GPU clip mask"),
                contents: &clip_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
            let data = paint_data(draw, image)?;
            let data_bytes = words_as_bytes(&data);
            let data_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("EXR2 vector GPU paint data"),
                contents: &data_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
            let source_view = surface.texture().create_view(&Default::default());
            let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("EXR2 vector GPU paint bindings"),
                layout: &paint.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: coverage_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: clip_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&source_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: data_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: touched.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: surface.write_mask_accumulator().as_entire_binding(),
                    },
                ],
            });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("EXR2 vector GPU paint"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&paint.pipeline);
                pass.set_bind_group(0, &bindings, &[]);
                pass.dispatch_workgroups(
                    (plan.width as u32).div_ceil(8),
                    (plan.height as u32).div_ceil(8),
                    1,
                );
            }
        }
    }
    encoder.copy_buffer_to_buffer(&touched, 0, &touched_readback, 0, 4);
    let readback_requests: Vec<_> = targets
        .iter()
        .filter_map(|(key, _, _)| readback_regions.get(&key.guest_address).map(|region| (*key, *region)))
        .collect();
    let pending = context
        .encode_region_readbacks(&mut encoder, &readback_requests)
        .map_err(|error| Unsupported(format!("Graphics UI GPU readback {error}")))?;
    context.submit(encoder.finish());

    let readbacks = context
        .finish_region_readbacks_async(pending)
        .await
        .map_err(|error| Unsupported(format!("Graphics UI GPU readback {error}")))?;
    let region_readback_bytes = readbacks
        .iter()
        .map(|(_, readback)| readback.records.len() as u64)
        .sum::<u64>();
    let touched_bytes = context
        .map_read_buffer_async(&touched_readback)
        .await
        .map_err(|error| Unsupported(format!("Graphics UI GPU touched-pixel readback: {error}")))?;
    let touched_pixels = u32::from_le_bytes(touched_bytes[..4].try_into().unwrap()) as u64;

    let mut output = Vec::new();
    for (key, readback) in readbacks {
        output.extend(
            output_runs(key, &readback.records, readback.region)?
                .into_iter()
                .map(|(address, bytes)| (address, Arc::new(bytes))),
        );
    }
    let output_bytes = output.iter().map(|(_, bytes)| bytes.len() as u64).sum();
    debug_assert!(output_bytes <= touched_pixels * 4);
    if std::env::var_os("FUJIMOD_VECTOR_GPU_STATS").is_some() {
        eprintln!(
            "vector_gpu_transfer target_snapshot_bytes={target_snapshot_bytes} region_readback_bytes={region_readback_bytes} guest_writeback_bytes={output_bytes}"
        );
    }
    Ok((output, output_bytes))
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn render(
    draws: &[batch::Draw],
    cpu: i64,
    host: &mut dyn Inputs,
) -> Result<RenderedBatch> {
    block_on(render_async(draws, cpu, host))
}

#[cfg(not(target_arch = "wasm32"))]
struct ThreadWake(std::thread::Thread);

#[cfg(not(target_arch = "wasm32"))]
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

#[cfg(not(target_arch = "wasm32"))]
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

fn output_runs(
    key: SurfaceKey,
    records: &[u8],
    region: fujimod_emulator_gpu::SurfaceRegion,
) -> Result<Vec<(u64, Vec<u8>)>> {
    let bpp = key.format.bytes_per_pixel() as usize;
    let expected = (region.width as usize)
        .checked_mul(region.height as usize)
        .and_then(|pixels| pixels.checked_mul(8))
        .ok_or_else(|| Unsupported("Graphics UI GPU region size overflow".into()))?;
    if records.len() != expected || bpp != 4 {
        return fail("Graphics UI GPU readback region is incomplete");
    }
    let mut runs = Vec::<(u64, Vec<u8>)>::new();
    if region.width == 0 || region.height == 0 {
        return Ok(runs);
    }
    match key.layout {
        SurfaceLayout::Linear => {
            for y in region.y..region.y + region.height {
                for x in region.x..region.x + region.width {
                    append_pixel_write(&mut runs, key, records, region, x, y);
                }
            }
        }
        SurfaceLayout::Morton8x8 => {
            let x_end = region.x + region.width;
            let y_end = region.y + region.height;
            for tile_y in region.y / 8..y_end.div_ceil(8) {
                for tile_x in region.x / 8..x_end.div_ceil(8) {
                    for local in 0..64 {
                        let (local_x, local_y) = morton_coordinates(local);
                        let x = tile_x * 8 + local_x;
                        let y = tile_y * 8 + local_y;
                        if x >= region.x && x < x_end && y >= region.y && y < y_end {
                            append_pixel_write(&mut runs, key, records, region, x, y);
                        }
                    }
                }
            }
        }
    }
    Ok(runs)
}

fn append_pixel_write(
    runs: &mut Vec<(u64, Vec<u8>)>,
    key: SurfaceKey,
    records: &[u8],
    region: fujimod_emulator_gpu::SurfaceRegion,
    x: u32,
    y: u32,
) {
    let record_index = ((y - region.y) * region.width + (x - region.x)) as usize;
    let record = &records[record_index * 8..record_index * 8 + 8];
    let pixel = match key.layout {
        SurfaceLayout::Linear => y as usize * key.extent.width as usize + x as usize,
        SurfaceLayout::Morton8x8 => {
            let local = (x & 1)
                | ((y & 1) << 1)
                | ((x & 2) << 1)
                | ((y & 2) << 2)
                | ((x & 4) << 2)
                | ((y & 4) << 3);
            ((y as usize >> 3) * (key.extent.width as usize >> 3) + (x as usize >> 3)) * 64
                + local as usize
        }
    };
    let address = key.guest_address + (pixel * key.format.bytes_per_pixel() as usize) as u64;
    let value = u32::from_le_bytes(record[..4].try_into().unwrap());
    let mask = u32::from_le_bytes(record[4..].try_into().unwrap());
    for lane in 0..key.format.bytes_per_pixel() as usize {
        if mask & (1 << lane) == 0 {
            continue;
        }
        let write_address = address + lane as u64;
        let byte = (value >> (lane * 8)) as u8;
        if let Some((start, data)) = runs.last_mut()
            && *start + data.len() as u64 == write_address
        {
            data.push(byte);
        } else {
            runs.push((write_address, vec![byte]));
        }
    }
}

fn morton_coordinates(index: u32) -> (u32, u32) {
    let mut x = 0;
    let mut y = 0;
    for bit in 0..3 {
        x |= ((index >> (2 * bit)) & 1) << bit;
        y |= ((index >> (2 * bit + 1)) & 1) << bit;
    }
    (x, y)
}

fn words_as_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Inputs;
    use fujimod_emulator_graphics::batch::{ColorTransform, Gradient, Texture};
    use std::rc::Rc;

    struct TestInputs {
        regions: HashMap<u64, Vec<u8>>,
    }

    impl Inputs for TestInputs {
        fn has_reader(&self) -> bool {
            true
        }

        fn has_irq(&self) -> bool {
            true
        }

        fn routed(&self) -> bool {
            true
        }

        fn read(
            &mut self,
            _cpu: i64,
            address: u64,
            length: u64,
        ) -> std::result::Result<Vec<u8>, String> {
            self.regions
                .get(&address)
                .filter(|bytes| bytes.len() as u64 == length)
                .cloned()
                .ok_or_else(|| format!("unexpected test read at {address:#x} for {length} bytes"))
        }

        fn targets(&mut self) -> Result<(usize, i64)> {
            Ok((0, 0))
        }
    }

    fn path(bounds: [f64; 4]) -> Vec<fujimod_emulator_graphics::stream::Geometry> {
        let [left, top, right, bottom] = bounds;
        vec![
            (0x28, 0, vec![left, top]),
            (0x29, 0, vec![right, top]),
            (0x29, 0, vec![right, bottom]),
            (0x29, 0, vec![left, bottom]),
            (0x29, 0, vec![left, top]),
        ]
    }

    fn draw(
        address: u64,
        order: u32,
        bounds: [f64; 4],
        samples: u32,
        blend: u32,
        color: [u8; 4],
    ) -> batch::Draw {
        batch::Draw {
            target: (address, 8, 8, order),
            sequence: 0,
            matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            geometry: path(bounds),
            samples,
            clips: vec![(0, 0, 8, 8)],
            nonzero: false,
            blend,
            color,
            stroke: false,
            texture: None,
            gradient: None,
            color_transform: None,
            radius: 0.0,
            cap: 0,
            join: 0,
            device_radius: false,
        }
    }

    fn cpu_coverage(draw: &batch::Draw) -> Result<Rc<render::Coverage>> {
        render::coverage(draw).map(Rc::new)
    }

    fn compare_with_cpu(order: u32) {
        let context = match fujimod_emulator_gpu::context() {
            Ok(context) => context,
            Err(error) if error.starts_with("host GPU adapter:") => {
                eprintln!("skipping EXR2 GPU paint test without an adapter: {error}");
                return;
            }
            Err(error) => panic!("EXR2 GPU paint context failed: {error}"),
        };
        assert!(
            context.supports_read_write_r32uint_storage(),
            "the EXR2 paint path requires in-place R32Uint storage support"
        );

        let target_address = 0x175000 + u64::from(order) * 0x1000;
        let texture_address = target_address + 0x1000;
        let target: Vec<u8> = (0..8 * 8 * 4)
            .map(|index| (index * 37 + 11) as u8)
            .collect();
        let initial_target = target.clone();
        let texture_bytes: Vec<u8> = (0..8 * 8 * 4).map(|index| (index * 19 + 7) as u8).collect();
        let mut draws = vec![
            draw(
                target_address,
                order,
                [0.0, 0.0, 8.0, 8.0],
                1,
                0,
                [19, 79, 137, 211],
            ),
            draw(
                target_address,
                order,
                [1.25, 0.7, 6.6, 6.1],
                4,
                1,
                [223, 57, 91, 149],
            ),
        ];
        let mut gradient_draw = draw(
            target_address,
            order,
            [0.4, 1.2, 7.6, 7.1],
            2,
            1,
            [0, 0, 0, 0],
        );
        gradient_draw.gradient = Some(Gradient {
            coefficients: (0.047, 0.11, 0.029, 0.17),
            radial: None,
            colors: (0..128)
                .map(|index| {
                    [
                        (index * 17 % 256) as u8,
                        (index * 41 % 256) as u8,
                        (255 - index * 13 % 256) as u8,
                        (64 + index * 7 % 192) as u8,
                    ]
                })
                .collect(),
            spread: 1,
            premultiplied: false,
        });
        gradient_draw.color_transform = Some(ColorTransform {
            scale: [0.875, 1.125, 0.75, 1.0],
            bias: [0.03125, 0.0, 0.015625, -0.015625],
        });
        let mut texture_draw = draw(
            target_address,
            order,
            [2.1, 0.3, 7.4, 5.8],
            4,
            0,
            [0, 0, 0, 0],
        );
        texture_draw.texture = Some(Texture {
            source: texture_address,
            width: 8,
            height: 8,
            valid_width: 7,
            valid_height: 6,
            matrix: [1.0, 0.0, 0.25, 0.0, 1.0, -0.5],
        });
        draws.extend([gradient_draw, texture_draw]);

        let mut host = TestInputs {
            regions: HashMap::from([
                (target_address, target.clone()),
                (texture_address, texture_bytes.clone()),
            ]),
        };
        let actual = render(&draws, 0, &mut host).expect("GPU paint should render");

        let targets = HashMap::from([(target_address, target)]);
        let textures = HashMap::from([((texture_address, 8, 8), texture_bytes)]);
        let mut coverage = cpu_coverage;
        let (expected, _) =
            super::render::render(&draws, &targets, &textures, &mut coverage)
                .expect("CPU paint reference should render");
        let output_bytes: u64 = actual.0.iter().map(|(_, bytes)| bytes.len() as u64).sum();
        assert_eq!(actual.1, output_bytes);
        let mut actual_surface = initial_target;
        for (address, bytes) in &actual.0 {
            let start = (*address - target_address) as usize;
            actual_surface[start..start + bytes.len()].copy_from_slice(bytes);
        }
        if actual_surface.as_slice() != expected[0].1.as_slice() {
            let differences: Vec<_> = actual_surface
                .chunks_exact(4)
                .zip(expected[0].1.chunks_exact(4))
                .enumerate()
                .filter(|(_, (actual, expected))| actual != expected)
                .take(8)
                .map(|(index, (actual, expected))| (index, actual.to_vec(), expected.to_vec()))
                .collect();
            panic!("paint output differed for order {order}: {differences:?}");
        }
    }

    #[test]
    fn rgba8_src_src_over_texture_and_linear_ramp_match_cpu_reference() {
        compare_with_cpu(0);
        compare_with_cpu(1);
    }
}

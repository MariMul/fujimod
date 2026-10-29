//! Shared coverage rasterization over floating-point edge coordinates.

use super::GpuContext;
use std::mem::size_of;
use wgpu::util::DeviceExt;

const MAX_EDGE_GRID_COORDINATE: f64 = 16_383.0;
// This fixed-point rasterizer keeps signed edge cross products within i32.
const MAX_BOUNDS: u32 = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillRule {
    EvenOdd,
    NonZero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleGrid {
    OneByOne,
    TwoByTwo,
    FourByFour,
}

impl SampleGrid {
    pub const fn side(self) -> u32 {
        match self {
            Self::OneByOne => 1,
            Self::TwoByTwo => 2,
            Self::FourByFour => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverageEdge {
    pub start: [f32; 2],
    pub end: [f32; 2],
}

impl CoverageEdge {
    pub const fn new(start: [f32; 2], end: [f32; 2]) -> Self {
        Self { start, end }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CoverageRequest<'a> {
    /// Pixel bounds as `[left, right, top, bottom)`, with right and bottom exclusive.
    pub bounds: [u32; 4],
    /// Line segments in pixel coordinates. The rasterizer classifies them on an eighth-pixel grid.
    pub edges: &'a [CoverageEdge],
    pub fill_rule: FillRule,
    pub sample_grid: SampleGrid,
    /// Optional stroke radius in pixels, also classified on the eighth-pixel grid.
    pub stroke_radius: Option<f32>,
}

pub struct CoverageKernel {
    pipeline: wgpu::ComputePipeline,
}

impl CoverageKernel {
    pub fn new(context: &GpuContext) -> Self {
        let device = context.device();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GPU coverage"),
            source: wgpu::ShaderSource::Wgsl(include_str!("coverage.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("GPU coverage"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Self { pipeline }
    }

    /// Rasterize a batch of edge sets to row-major per-pixel sample counts.
    pub fn coverage_batch(
        &self,
        context: &GpuContext,
        requests: &[CoverageRequest<'_>],
    ) -> Result<Vec<Vec<u32>>, String> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        let mut jobs = Vec::<u32>::with_capacity(requests.len() * 12);
        let mut edges = Vec::<[f32; 4]>::new();
        let mut output_lengths = Vec::with_capacity(requests.len());
        let mut output_count = 0usize;
        let mut max_width = 0usize;
        let mut max_height = 0usize;

        for request in requests {
            let [x0, x1, y0, y1] = request.bounds;
            if x0 > x1 || y0 > y1 || x1 > MAX_BOUNDS || y1 > MAX_BOUNDS {
                return Err("coverage input has invalid bounds".into());
            }
            for edge in request.edges {
                for coordinate in edge.start.into_iter().chain(edge.end) {
                    validate_grid_coordinate(coordinate)?;
                }
            }
            let stroke_radius = request
                .stroke_radius
                .map(|radius| quantized_grid_coordinate(radius))
                .transpose()?;

            let width = (x1 - x0) as usize;
            let height = (y1 - y0) as usize;
            let output_len = width
                .checked_mul(height)
                .ok_or("coverage output extent overflow")?;
            let edge_offset: u32 = edges
                .len()
                .try_into()
                .map_err(|_| "coverage edge offset overflow")?;
            let edge_count: u32 = request
                .edges
                .len()
                .try_into()
                .map_err(|_| "coverage edge count overflow")?;
            let coverage_offset: u32 = output_count
                .try_into()
                .map_err(|_| "coverage output offset overflow")?;
            let mut flags = u32::from(stroke_radius.is_some());
            flags |= u32::from(request.fill_rule == FillRule::NonZero) << 1;
            jobs.extend([
                x0,
                y0,
                width as u32,
                height as u32,
                request.sample_grid.side(),
                edge_offset,
                edge_count,
                flags,
                stroke_radius.unwrap_or(0) as u32,
                coverage_offset,
                0,
                0,
            ]);
            edges.extend(
                request
                    .edges
                    .iter()
                    .map(|edge| [edge.start[0], edge.start[1], edge.end[0], edge.end[1]]),
            );
            output_count = output_count
                .checked_add(output_len)
                .ok_or("coverage batch output overflow")?;
            output_lengths.push(output_len);
            max_width = max_width.max(width);
            max_height = max_height.max(height);
        }

        if output_count == 0 {
            return Ok(output_lengths.iter().map(|_| Vec::new()).collect());
        }
        if edges.is_empty() {
            edges.push([0.0; 4]);
        }

        let edge_bytes = edges
            .len()
            .checked_mul(size_of::<[f32; 4]>())
            .ok_or("coverage edge buffer size overflow")?;
        let job_bytes = jobs
            .len()
            .checked_mul(size_of::<u32>())
            .ok_or("coverage job buffer size overflow")?;
        let output_bytes = output_count
            .checked_mul(size_of::<u32>())
            .ok_or("coverage output buffer size overflow")?;
        let limits = context.device().limits();
        if edge_bytes > limits.max_storage_buffer_binding_size as usize
            || job_bytes > limits.max_storage_buffer_binding_size as usize
            || output_bytes > limits.max_storage_buffer_binding_size as usize
            || output_bytes as u64 > limits.max_buffer_size
            || requests.len() > limits.max_compute_workgroups_per_dimension as usize
            || max_width.div_ceil(8) > limits.max_compute_workgroups_per_dimension as usize
            || max_height.div_ceil(8) > limits.max_compute_workgroups_per_dimension as usize
        {
            return Err("coverage batch exceeds host GPU limits".into());
        }

        let edge_bytes_data: Vec<u8> = edges
            .iter()
            .flat_map(|edge| edge.iter().flat_map(|value| value.to_le_bytes()))
            .collect();
        let job_bytes_data: Vec<u8> = jobs.into_iter().flat_map(u32::to_le_bytes).collect();
        let device = context.device();
        let queue = context.queue();
        let jobs_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("coverage jobs"),
            contents: &job_bytes_data,
            usage: wgpu::BufferUsages::STORAGE,
        });
        let edges_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("coverage edges"),
            contents: &edge_bytes_data,
            usage: wgpu::BufferUsages::STORAGE,
        });
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("coverage output"),
            size: output_bytes as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let download = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("coverage readback"),
            size: output_bytes as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("coverage bindings"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: jobs_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: edges_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            pass.dispatch_workgroups(
                max_width.div_ceil(8) as u32,
                max_height.div_ceil(8) as u32,
                requests.len() as u32,
            );
        }
        encoder.copy_buffer_to_buffer(&output, 0, &download, 0, output_bytes as u64);
        queue.submit([encoder.finish()]);

        let (sender, receiver) = std::sync::mpsc::channel();
        download
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| format!("GPU coverage poll: {error}"))?;
        receiver
            .recv()
            .map_err(|error| format!("GPU coverage readback: {error}"))?
            .map_err(|error| format!("GPU coverage map: {error}"))?;
        let bytes = download
            .slice(..)
            .get_mapped_range()
            .map_err(|error| format!("GPU coverage mapped range: {error}"))?;
        let values: Vec<u32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| u32::from_le_bytes(*chunk))
            .collect();
        drop(bytes);
        download.unmap();

        let mut offset = 0usize;
        let result = output_lengths
            .into_iter()
            .map(|length| {
                let end = offset + length;
                let values = values[offset..end].to_vec();
                offset = end;
                values
            })
            .collect();
        Ok(result)
    }
}

fn validate_grid_coordinate(value: f32) -> Result<(), String> {
    let _ = quantized_grid_coordinate(value)?;
    Ok(())
}

fn quantized_grid_coordinate(value: f32) -> Result<i32, String> {
    if !value.is_finite() {
        return Err("coverage edge coordinate is not finite".into());
    }
    let coordinate = (f64::from(value) * 8.0).round();
    if coordinate.abs() > MAX_EDGE_GRID_COORDINATE {
        return Err("coverage edge coordinate exceeds the fixed-point range".into());
    }
    Ok(coordinate as i32)
}

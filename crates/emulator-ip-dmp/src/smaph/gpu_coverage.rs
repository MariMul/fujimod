//! GPU coverage for a complete SMAPH path bounding box.
//!
//! The shader evaluates the DMP's regular subsample grid directly. It does not
//! use host multisampling positions.

use super::scanline;
use std::sync::OnceLock;
use wgpu::util::DeviceExt;

pub type Point = [f64; 2];

struct Gpu {
    name: String,
    pipeline: wgpu::ComputePipeline,
}

impl Gpu {
    fn new(context: &crate::gpu::GpuContext) -> Self {
        let device = context.device();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SMAPH path coverage"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_coverage.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SMAPH path coverage"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Self {
            name: context.adapter_name().to_owned(),
            pipeline,
        }
    }

    fn coverage_batch(
        &self,
        context: &crate::gpu::GpuContext,
        inputs: &[scanline::CoverageInput],
    ) -> Result<Vec<Vec<u32>>, String> {
        let device = context.device();
        let queue = context.queue();
        if inputs.is_empty() {
            return Ok(Vec::new());
        }

        let mut jobs = Vec::<u32>::with_capacity(inputs.len() * 12);
        let mut edges = Vec::<[i32; 4]>::new();
        let mut output_lengths = Vec::with_capacity(inputs.len());
        let mut output_count = 0usize;
        let mut max_width = 0usize;
        let mut max_height = 0usize;
        for input in inputs {
            let [x0, x1, y0, y1] = input.bounds;
            if x0 > x1 || y0 > y1 || x1 > 2048 || y1 > 2048 {
                return Err("SMAPH coverage batch has invalid bounds".into());
            }
            if !matches!(input.samples, 0 | 1 | 2 | 4) {
                return Err("SMAPH coverage sample count must be 1, 2, or 4".into());
            }
            let width = x1 - x0;
            let height = y1 - y0;
            let output_len = input.output_len()?;
            let edge_offset: u32 = edges
                .len()
                .try_into()
                .map_err(|_| "SMAPH coverage edge offset")?;
            let edge_count: u32 = input
                .edges
                .len()
                .try_into()
                .map_err(|_| "SMAPH coverage edge count")?;
            let coverage_offset: u32 = output_count
                .try_into()
                .map_err(|_| "SMAPH coverage output offset")?;
            let mut flags = u32::from(input.stroke_radius.is_some());
            flags |= u32::from(input.nonzero) << 1;
            jobs.extend([
                x0 as u32,
                y0 as u32,
                width as u32,
                height as u32,
                input.samples,
                edge_offset,
                edge_count,
                flags,
                input.stroke_radius.unwrap_or(0) as u32,
                coverage_offset,
                0,
                0,
            ]);
            edges.extend_from_slice(&input.edges);
            output_count = output_count
                .checked_add(output_len)
                .ok_or("SMAPH coverage batch output overflow")?;
            output_lengths.push(output_len);
            max_width = max_width.max(width);
            max_height = max_height.max(height);
        }
        if output_count == 0 {
            return Ok(output_lengths.iter().map(|_| Vec::new()).collect());
        }
        if edges.is_empty() {
            edges.push([0; 4]);
        }

        let edge_bytes = edges
            .len()
            .checked_mul(16)
            .ok_or("SMAPH coverage edge buffer size overflow")?;
        let job_bytes = jobs
            .len()
            .checked_mul(4)
            .ok_or("SMAPH coverage job buffer size overflow")?;
        let output_bytes = output_count
            .checked_mul(std::mem::size_of::<u32>())
            .ok_or("SMAPH coverage output buffer size overflow")?;
        let limits = device.limits();
        if edge_bytes > limits.max_storage_buffer_binding_size as usize
            || job_bytes > limits.max_storage_buffer_binding_size as usize
            || output_bytes > limits.max_storage_buffer_binding_size as usize
            || output_bytes as u64 > limits.max_buffer_size
            || inputs.len() > limits.max_compute_workgroups_per_dimension as usize
            || max_width.div_ceil(8) > limits.max_compute_workgroups_per_dimension as usize
            || max_height.div_ceil(8) > limits.max_compute_workgroups_per_dimension as usize
        {
            return Err("SMAPH coverage batch exceeds host GPU limits".into());
        }

        let mut edge_bytes_data = Vec::with_capacity(edge_bytes);
        for edge in &edges {
            for value in edge {
                edge_bytes_data.extend_from_slice(&value.to_le_bytes());
            }
        }
        let job_bytes_data: Vec<u8> = jobs.into_iter().flat_map(u32::to_le_bytes).collect();
        let jobs_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SMAPH coverage jobs"),
            contents: &job_bytes_data,
            usage: wgpu::BufferUsages::STORAGE,
        });
        let edges_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SMAPH coverage edges"),
            contents: &edge_bytes_data,
            usage: wgpu::BufferUsages::STORAGE,
        });
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SMAPH coverage output"),
            size: output_bytes as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let download = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SMAPH coverage readback"),
            size: output_bytes as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SMAPH coverage bindings"),
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
                inputs.len() as u32,
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
            .map_err(|error| format!("SMAPH coverage GPU poll: {error}"))?;
        receiver
            .recv()
            .map_err(|error| format!("SMAPH coverage readback: {error}"))?
            .map_err(|error| format!("SMAPH coverage map: {error}"))?;
        let bytes = download
            .slice(..)
            .get_mapped_range()
            .map_err(|error| format!("SMAPH coverage mapped range: {error}"))?;
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

static GPU: OnceLock<Result<Option<Gpu>, String>> = OnceLock::new();

fn shared_gpu() -> Result<Option<&'static Gpu>, String> {
    let context = match crate::gpu::context() {
        Ok(context) => context,
        Err(error) if error.starts_with("DMP host GPU adapter:") => return Ok(None),
        Err(error) => return Err(error),
    };
    match GPU.get_or_init(|| Ok(Some(Gpu::new(context)))) {
        Ok(Some(gpu)) => Ok(Some(gpu)),
        Ok(None) => Ok(None),
        Err(error) => Err(error.clone()),
    }
}

/// Compute coverage for a complete pixel bounding box in one GPU dispatch.
///
/// Results are row-major. Fill edges close each contour with at least three
/// points; stroke edges follow the CPU implementation's open contour windows.
/// Coordinates and stroke radius are rounded to eighth-pixel units, which
/// includes the DMP's current 1x, 2x, and 4x sample modes.
pub fn coverage(
    contours: &[Vec<Point>],
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
    samples: u32,
    stroke: Option<f64>,
    nonzero: bool,
) -> Result<Vec<u32>, String> {
    let input = scanline::CoverageInput::new(contours, [x0, x1, y0, y1], samples, stroke, nonzero);
    coverage_batch(std::slice::from_ref(&input))
        .map(|mut outputs| outputs.pop().unwrap_or_default())
}

/// Rasterize every path in a stream with one dispatch and one edge buffer.
pub(super) fn coverage_batch(inputs: &[scanline::CoverageInput]) -> Result<Vec<Vec<u32>>, String> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let mut no_gpu_work = true;
    for input in inputs {
        if input.samples != 0 && input.output_len()? != 0 {
            no_gpu_work = false;
            break;
        }
    }
    if no_gpu_work {
        return inputs
            .iter()
            .map(|input| Ok(vec![0; input.output_len()?]))
            .collect();
    }
    let gpu = shared_gpu()?.ok_or_else(|| "SMAPH coverage has no wgpu adapter".to_owned())?;
    gpu.coverage_batch(crate::gpu::context()?, inputs)
}

#[cfg(test)]
mod tests {
    use super::{coverage, scanline, shared_gpu};

    type Point = [f64; 2];

    fn require_gpu() -> bool {
        match shared_gpu() {
            Ok(Some(gpu)) => {
                println!("SMAPH GPU coverage adapter: {}", gpu.name);
                true
            }
            Ok(None) => {
                println!("SKIP SMAPH GPU coverage: no wgpu adapter is available");
                false
            }
            Err(error) => panic!("SMAPH GPU adapter initialization failed: {error}"),
        }
    }

    fn compare(contours: &[Vec<Point>]) {
        let x0 = 0;
        let x1 = 16;
        let y0 = 0;
        let y1 = 16;
        for samples in [1, 2, 4] {
            for stroke in [None, Some(0.0), Some(0.5), Some(2.0)] {
                for nonzero in [false, true] {
                    let actual = coverage(contours, x0, x1, y0, y1, samples, stroke, nonzero)
                        .unwrap_or_else(|error| panic!("GPU coverage failed: {error}"));
                    let expected: Vec<u32> = (y0..y1)
                        .flat_map(|y| {
                            scanline::coverage(contours, x0, x1, y, samples, stroke, nonzero)
                        })
                        .collect();
                    assert_eq!(
                        actual, expected,
                        "samples={samples} stroke={stroke:?} nonzero={nonzero} contours={contours:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn authored_smaph_rectangle_matches_cpu_coverage() {
        if !require_gpu() {
            return;
        }
        compare(&[vec![[0.0, 0.0], [16.0, 0.0], [16.0, 16.0], [0.0, 16.0]]]);
    }

    #[test]
    fn scanline_contours_match_cpu_coverage() {
        if !require_gpu() {
            return;
        }
        compare(&[
            vec![[1.5, 1.5], [11.5, 1.5], [11.5, 9.5], [1.5, 9.5]],
            vec![[3.5, 3.5], [8.5, 3.5], [8.5, 7.5], [3.5, 7.5]],
        ]);
        compare(&[
            vec![[2.0, 2.0], [10.0, 10.0], [2.0, 10.0], [10.0, 2.0]],
            vec![[5.0, 5.0], [5.0, 5.0]],
            vec![[0.0, 4.0], [13.0, 4.0]],
        ]);
        compare(&[
            vec![[6.125, 0.125], [6.125, 11.125]],
            vec![[0.125, 6.125], [13.125, 6.125]],
            vec![[6.0, 6.0]],
        ]);
    }

    #[test]
    fn randomized_contours_match_cpu_coverage() {
        if !require_gpu() {
            return;
        }
        let mut seed = 123456789u32;
        for _ in 0..24 {
            let mut contours = Vec::new();
            for _ in 0..3 {
                let mut points = Vec::new();
                for _ in 0..8 {
                    let mut point = [0.0; 2];
                    for coordinate in &mut point {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        *coordinate = (seed % 128) as f64 / 8.0 - 2.0;
                    }
                    points.push(point);
                }
                contours.push(points);
            }
            compare(&contours);
        }
    }
}

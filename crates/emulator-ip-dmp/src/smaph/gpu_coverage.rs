//! GPU coverage for a complete SMAPH path bounding box.
//!
//! The shader evaluates the DMP's regular subsample grid directly. It does not
//! use host multisampling positions.

use super::scanline;
use std::{
    future::Future,
    sync::{Arc, OnceLock},
    task::{Context, Poll, Wake},
};
use wgpu::util::DeviceExt;

pub type Point = [f64; 2];

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

struct Gpu {
    name: String,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
}

impl Gpu {
    fn new() -> Result<Option<Self>, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            match block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())) {
                Ok(adapter) => adapter,
                Err(wgpu::RequestAdapterError::NotFound { .. }) => return Ok(None),
                Err(error) => return Err(format!("SMAPH coverage adapter: {error}")),
            };
        let info = adapter.get_info();
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|error| format!("SMAPH coverage device: {error}"))?;
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
        Ok(Some(Self {
            name: format!("{:?}: {} ({:?})", info.backend, info.name, info.device_type),
            device,
            queue,
            pipeline,
        }))
    }

    fn coverage(
        &self,
        contours: &[Vec<Point>],
        x0: usize,
        x1: usize,
        y0: usize,
        y1: usize,
        samples: u32,
        stroke: Option<f64>,
        nonzero: bool,
    ) -> Result<Vec<u32>, String> {
        let width = x1.saturating_sub(x0);
        let height = y1.saturating_sub(y0);
        let count = width
            .checked_mul(height)
            .ok_or("SMAPH coverage extent overflow")?;
        if count == 0 || samples == 0 {
            return Ok(vec![0; count]);
        }
        if !matches!(samples, 1 | 2 | 4) {
            return Err("SMAPH coverage sample count must be 1, 2, or 4".into());
        }
        if x1 > 2048 || y1 > 2048 {
            return Err("SMAPH coverage bounds exceed the DMP coordinate grid".into());
        }

        let x0: u32 = x0.try_into().map_err(|_| "SMAPH coverage x origin")?;
        let y0: u32 = y0.try_into().map_err(|_| "SMAPH coverage y origin")?;
        let width_u32: u32 = width.try_into().map_err(|_| "SMAPH coverage width")?;
        let height_u32: u32 = height.try_into().map_err(|_| "SMAPH coverage height")?;
        let mut edges = Vec::<[i32; 4]>::new();
        if stroke.is_some() {
            for points in contours {
                for pair in points.windows(2) {
                    edges.push([
                        scanline::grid_coordinate(pair[0][0]),
                        scanline::grid_coordinate(pair[0][1]),
                        scanline::grid_coordinate(pair[1][0]),
                        scanline::grid_coordinate(pair[1][1]),
                    ]);
                }
            }
        } else {
            for points in contours.iter().filter(|points| points.len() >= 3) {
                for (a, b) in points
                    .iter()
                    .zip(points.iter().cycle().skip(1))
                    .take(points.len())
                {
                    edges.push([
                        scanline::grid_coordinate(a[0]),
                        scanline::grid_coordinate(a[1]),
                        scanline::grid_coordinate(b[0]),
                        scanline::grid_coordinate(b[1]),
                    ]);
                }
            }
        }
        let radius = scanline::grid_coordinate(stroke.unwrap_or(0.0));
        let edge_count: u32 = edges
            .len()
            .try_into()
            .map_err(|_| "SMAPH coverage edge count")?;

        let mut params = [0u32; 9];
        params[0] = x0;
        params[1] = y0;
        params[2] = width_u32;
        params[3] = height_u32;
        params[4] = samples;
        params[5] = edge_count;
        params[6] = u32::from(stroke.is_some());
        params[7] = u32::from(nonzero);
        params[8] = radius as u32;

        let mut edge_bytes = Vec::with_capacity(edges.len() * 16);
        for edge in &edges {
            for value in edge {
                edge_bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        if edge_bytes.is_empty() {
            edge_bytes.resize(16, 0);
        }
        let param_bytes: Vec<u8> = params.into_iter().flat_map(u32::to_le_bytes).collect();
        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SMAPH coverage parameters"),
                contents: &param_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let edges_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SMAPH coverage edges"),
                contents: &edge_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let byte_count = count
            .checked_mul(std::mem::size_of::<u32>())
            .ok_or("SMAPH coverage buffer size overflow")?;
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SMAPH coverage output"),
            size: byte_count as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let download = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SMAPH coverage readback"),
            size: byte_count as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SMAPH coverage bindings"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
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
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            pass.dispatch_workgroups(width_u32.div_ceil(8), height_u32.div_ceil(8), 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &download, 0, byte_count as u64);
        self.queue.submit([encoder.finish()]);

        let (sender, receiver) = std::sync::mpsc::channel();
        download
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.device
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
        let result = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| u32::from_le_bytes(*chunk))
            .collect();
        drop(bytes);
        download.unmap();
        Ok(result)
    }
}

static GPU: OnceLock<Result<Option<Gpu>, String>> = OnceLock::new();

fn shared_gpu() -> Result<Option<&'static Gpu>, String> {
    match GPU.get_or_init(Gpu::new) {
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
    shared_gpu()?
        .ok_or_else(|| "SMAPH coverage has no wgpu adapter".to_owned())?
        .coverage(contours, x0, x1, y0, y1, samples, stroke, nonzero)
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

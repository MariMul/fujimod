//! Host compute scan conversion. Integer edge equations match the CPU path;
//! host handles and adapter selection never enter emulated checkpoints.
use std::{
    future::Future,
    sync::{Arc, Mutex, OnceLock},
    task::{Context, Poll, Wake},
};
use wgpu::util::DeviceExt;
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
    fn new() -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
        let info = adapter.get_info();
        if info.device_type == wgpu::DeviceType::Cpu {
            return None;
        }
        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DMP integer coverage"),
            source: wgpu::ShaderSource::Wgsl(include_str!("coverage.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DMP coverage"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Some(Self {
            name: format!("{:?}: {}", info.backend, info.name),
            device,
            queue,
            pipeline,
        })
    }
    fn coverage(&self, p: [i32; 16]) -> Result<Vec<u32>, String> {
        let count = (p[8] * p[9]) as usize;
        let data: Vec<_> = p.into_iter().flat_map(i32::to_le_bytes).collect();
        let parameters = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &data,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (count * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let download = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (count * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: parameters.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            pass.dispatch_workgroups((count as u32).div_ceil(64), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &download, 0, (count * 4) as u64);
        self.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        download
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("DMP host GPU polling: {e}"))?;
        receiver
            .recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let bytes = download
            .slice(..)
            .get_mapped_range()
            .map_err(|e| e.to_string())?;
        let result = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect();
        drop(bytes);
        download.unmap();
        Ok(result)
    }
}
static GPU: OnceLock<Option<Mutex<Gpu>>> = OnceLock::new();
pub(super) fn coverage(
    q: [[i64; 2]; 3],
    bounds: [usize; 4],
    inclusive: [bool; 3],
) -> Result<Option<Vec<u32>>, String> {
    let [x0, y0, x1, y1] = bounds;
    if x1 <= x0 || y1 <= y0 || (x1 - x0) * (y1 - y0) < 256 {
        return Ok(None);
    }
    // Prove every edge product fits signed 32 bits before selecting WGSL.
    if q.iter().flatten().any(|v| !(0..=16383).contains(v)) || x1 * 16 > 16383 || y1 * 16 > 16383 {
        return Ok(None);
    }
    let Some(gpu) = GPU.get_or_init(|| Gpu::new().map(Mutex::new)) else {
        return Ok(None);
    };
    let p = [
        q[0][0] as i32,
        q[0][1] as i32,
        q[1][0] as i32,
        q[1][1] as i32,
        q[2][0] as i32,
        q[2][1] as i32,
        x0 as i32,
        y0 as i32,
        (x1 - x0) as i32,
        (y1 - y0) as i32,
        inclusive[0] as i32,
        inclusive[1] as i32,
        inclusive[2] as i32,
        0,
        0,
        0,
    ];
    gpu.lock()
        .map_err(|_| "DMP host GPU lock poisoned")?
        .coverage(p)
        .map(Some)
}

pub(super) fn backend_name() -> Option<String> {
    GPU.get()?.as_ref()?.lock().ok().map(|gpu| gpu.name.clone())
}

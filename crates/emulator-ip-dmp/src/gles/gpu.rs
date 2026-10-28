//! Host compute scan conversion. Integer edge equations match the CPU path;
//! host handles and adapter selection never enter emulated checkpoints.
use std::sync::{Mutex, OnceLock};
use wgpu::util::DeviceExt;
struct Gpu {
    pipeline: wgpu::ComputePipeline,
}
impl Gpu {
    fn new(context: &crate::gpu::GpuContext) -> Option<Self> {
        if context.adapter_device_type() == wgpu::DeviceType::Cpu {
            return None;
        }
        let device = context.device();
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
        Some(Self { pipeline })
    }
    fn coverage(&self, context: &crate::gpu::GpuContext, p: [i32; 16]) -> Result<Vec<u32>, String> {
        let device = context.device();
        let queue = context.queue();
        let count = (p[8] * p[9]) as usize;
        let data: Vec<_> = p.into_iter().flat_map(i32::to_le_bytes).collect();
        let parameters = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: &data,
            usage: wgpu::BufferUsages::STORAGE,
        });
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (count * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let download = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (count * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
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
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            pass.dispatch_workgroups((count as u32).div_ceil(64), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &download, 0, (count * 4) as u64);
        queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        download
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        device
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
    let context = match crate::gpu::context() {
        Ok(context) => context,
        Err(error) if error.starts_with("DMP host GPU adapter:") => return Ok(None),
        Err(error) => return Err(error),
    };
    let Some(gpu) = GPU.get_or_init(|| Gpu::new(context).map(Mutex::new)) else {
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
        .coverage(context, p)
        .map(Some)
}

pub(super) fn backend_name() -> Option<String> {
    GPU.get()?.as_ref()?;
    crate::gpu::context()
        .ok()
        .map(|context| context.adapter_name().to_owned())
}

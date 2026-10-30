//! Bounded background partial evaluation. Every queued configuration has an
//! equivalent generic pipeline already available to the execution owner.
use super::{DrawState, FragmentProgram, cached_pipeline, gpu_vertex};
use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock, mpsc},
};

struct Task {
    device: wgpu::Device,
    state: DrawState,
    vertex: gpu_vertex::PreparedDraw,
    program: FragmentProgram,
}
struct Compiler {
    requested: HashSet<([u8; 32], FragmentProgram)>,
    sender: mpsc::Sender<Task>,
}
static COMPILER: OnceLock<Mutex<Compiler>> = OnceLock::new();
const MAX_SPECIALIZATIONS: usize = 64;

pub(super) fn request(
    device: &wgpu::Device,
    state: &DrawState,
    vertex: &gpu_vertex::PreparedDraw,
    program: FragmentProgram,
) {
    let compiler = COMPILER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<Task>();
        std::thread::Builder::new()
            .name("DMP shader compiler".into())
            .spawn(move || {
                while let Ok(task) = receiver.recv() {
                    if let Err(error) =
                        cached_pipeline(&task.device, &task.state, &task.vertex, &task.program)
                    {
                        eprintln!("DMP shader specialization: {error}");
                    }
                }
            })
            .expect("start DMP shader compiler");
        Mutex::new(Compiler {
            requested: HashSet::new(),
            sender,
        })
    });
    let Ok(mut compiler) = compiler.lock() else {
        return;
    };
    let key = (vertex.pipeline_key, program.clone());
    if compiler.requested.len() >= MAX_SPECIALIZATIONS || !compiler.requested.insert(key) {
        return;
    }
    let _ = compiler.sender.send(Task {
        device: device.clone(),
        state: state.clone(),
        vertex: vertex.clone(),
        program,
    });
}

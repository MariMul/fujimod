//! Host computation over owned, immutable device inputs. Workers never receive
//! a bus, guest RAM, a device, or a clock. Only the owner publishes their output
//! at its existing guest deadline. Inspection may wait for a result but cannot
//! publish it or advance guest time.
use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex, OnceLock, mpsc},
    thread::{self, JoinHandle},
};

type Task = Box<dyn FnOnce() + Send>;
struct Pool {
    sender: Option<mpsc::Sender<Task>>,
    threads: Vec<JoinHandle<()>>,
}
impl Pool {
    fn new(count: usize) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel::<Task>();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut pool = Self {
            sender: Some(sender),
            threads: Vec::new(),
        };
        for index in 0..count {
            let receiver = receiver.clone();
            pool.threads.push(
                thread::Builder::new()
                    .name(format!("device-{index}"))
                    .stack_size(4 * 1024 * 1024)
                    .spawn(move || {
                        loop {
                            let task = receiver.lock().unwrap().recv();
                            match task {
                                Ok(task) => task(),
                                Err(_) => break,
                            }
                        }
                    })
                    .map_err(|e| format!("device worker startup: {e}"))?,
            );
        }
        Ok(pool)
    }
}
impl Drop for Pool {
    fn drop(&mut self) {
        self.sender.take();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}
thread_local! { static POOL: RefCell<Option<Pool>> = const { RefCell::new(None) }; }

/// Host execution policy, local to the owning CPU thread. Zero executes inline.
/// Reconfiguration drains existing work without publishing any device effects.
pub fn configure(count: usize) -> Result<(), String> {
    if count > 4 {
        return Err("device worker count must be 0..=4".into());
    }
    POOL.with(|slot| {
        slot.borrow_mut().take();
    });
    let pool = if count == 0 {
        None
    } else {
        Some(Pool::new(count)?)
    };
    POOL.with(|slot| {
        *slot.borrow_mut() = pool;
    });
    Ok(())
}

pub fn worker_count() -> usize {
    POOL.with(|pool| pool.borrow().as_ref().map_or(0, |p| p.threads.len()))
}

/// A pure computation with a single owned result. A worker does not retain the
/// result after sending it, so consuming a job never clones its image storage.
pub struct Job<T> {
    value: OnceLock<T>,
    receiver: Option<Mutex<mpsc::Receiver<Result<T, String>>>>,
}
impl<T: Send + Sync + 'static> Job<T> {
    pub fn submit(compute: impl FnOnce() -> T + Send + 'static) -> Self {
        let sender = POOL.with(|pool| {
            pool.borrow()
                .as_ref()
                .map(|p| p.sender.as_ref().unwrap().clone())
        });
        let Some(sender) = sender else {
            return Self {
                value: OnceLock::from(compute()),
                receiver: None,
            };
        };
        let (complete, receiver) = mpsc::channel();
        sender
            .send(Box::new(move || {
                let value = std::panic::catch_unwind(std::panic::AssertUnwindSafe(compute))
                    .map_err(|_| "device computation panicked".to_owned());
                // Cancellation/rollback can discard the receiver. The calculation
                // owns its inputs and its discarded output cannot reach guest RAM.
                let _ = complete.send(value);
            }))
            .unwrap_or_else(|_| panic!("device worker queue disconnected"));
        Self {
            value: OnceLock::new(),
            receiver: Some(Mutex::new(receiver)),
        }
    }
    pub fn get(&self) -> &T {
        self.value.get_or_init(|| {
            self.receiver
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .recv()
                .expect("device worker disconnected")
                .unwrap_or_else(|e| panic!("{e}"))
        })
    }
    pub fn into_value(self) -> T {
        self.get();
        self.value.into_inner().unwrap()
    }
}

/// A staging buffer whose bytes may be computed by a worker. The serialized
/// value is exactly the same byte string/array as eager storage; neither jobs
/// nor host completion order are modeled state. Dropping/canceling a buffer
/// discards its result, including work from a timeline later rolled back.
#[derive(Default)]
pub struct Buffer {
    bytes: Vec<u8>,
    job: Option<Job<Vec<u8>>>,
}
impl Buffer {
    pub fn defer(&mut self, compute: impl FnOnce(Vec<u8>) -> Vec<u8> + Send + 'static) {
        self.job = None;
        let bytes = std::mem::take(&mut self.bytes);
        self.job = Some(Job::submit(move || compute(bytes)));
    }
    /// Partition only pure row arithmetic. Every row retains its original
    /// image coordinate; integer reduction order within a pixel is unchanged.
    pub fn defer_rows(
        &mut self,
        row_bytes: usize,
        rows: usize,
        compute: impl Fn(std::ops::Range<usize>, &mut [u8]) + Send + Sync + 'static,
    ) {
        let workers = worker_count();
        if workers == 0 || rows * row_bytes < 65536 {
            self.defer(move |mut out| {
                out.resize(rows * row_bytes, 0);
                compute(0..rows, &mut out);
                out
            });
            return;
        }
        let compute = Arc::new(compute);
        let mut jobs = Vec::new();
        for start in (0..rows).step_by(rows.div_ceil(workers)) {
            let end = (start + rows.div_ceil(workers)).min(rows);
            let compute = compute.clone();
            jobs.push(Job::submit(move || {
                let mut out = vec![0; (end - start) * row_bytes];
                compute(start..end, &mut out);
                out
            }));
        }
        self.defer(move |mut out| {
            out.clear();
            out.reserve(rows * row_bytes);
            for job in jobs {
                out.extend(job.into_value());
            }
            out
        });
    }
    pub fn clear(&mut self) {
        self.job = None;
        self.bytes.clear();
    }
}
impl From<Vec<u8>> for Buffer {
    fn from(bytes: Vec<u8>) -> Self {
        Self { bytes, job: None }
    }
}
impl Deref for Buffer {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        self.job.as_ref().map_or(&self.bytes, Job::get)
    }
}
impl DerefMut for Buffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        if let Some(job) = self.job.take() {
            self.bytes = job.into_value();
        }
        &mut self.bytes
    }
}
impl serde::Serialize for Buffer {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(self.as_slice(), serializer)
    }
}
impl<'de> serde::Deserialize<'de> for Buffer {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(<Vec<u8> as serde::Deserialize>::deserialize(deserializer)?.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_inputs_finish_out_of_order_without_changing_bytes_or_checkpoints() {
        configure(2).unwrap();
        let (release, held) = mpsc::channel();
        let (started, running) = mpsc::channel();
        let mut first = Buffer::default();
        first.defer(move |_| {
            started.send(()).unwrap();
            held.recv().unwrap();
            vec![1, 2, 3]
        });
        running.recv().unwrap();
        let mut second = Buffer::default();
        second.defer(|_| vec![4, 5, 6]);
        assert_eq!(second.as_slice(), &[4, 5, 6]);
        release.send(()).unwrap();
        let eager = crate::checkpoint::Checkpoint::encode("buffer", &vec![1u8, 2, 3]).unwrap();
        let worker = crate::checkpoint::Checkpoint::encode("buffer", &first).unwrap();
        let (mut eager_bytes, mut worker_bytes) = (Vec::new(), Vec::new());
        ciborium::into_writer(&eager, &mut eager_bytes).unwrap();
        ciborium::into_writer(&worker, &mut worker_bytes).unwrap();
        assert_eq!(eager_bytes, worker_bytes);
        assert_eq!(serde_json::to_vec(&first).unwrap(), b"[1,2,3]");
        configure(0).unwrap();
    }
    #[test]
    fn row_partitions_preserve_global_coordinates_and_run_off_the_owner() {
        configure(4).unwrap();
        let owner = thread::current().id();
        let mut buffer = Buffer::default();
        buffer.defer_rows(1024, 257, move |rows, out| {
            assert_ne!(thread::current().id(), owner);
            for (local, value) in out.iter_mut().enumerate() {
                *value = ((rows.start * 1024 + local) % 251) as u8;
            }
        });
        assert_eq!(
            buffer.as_slice(),
            (0..1024 * 257).map(|i| (i % 251) as u8).collect::<Vec<_>>()
        );
        configure(0).unwrap();
    }
    #[test]
    fn canceled_and_restored_buffers_cannot_publish_an_old_result() {
        configure(1).unwrap();
        let (release, held) = mpsc::channel();
        let mut bytes = Buffer::default();
        bytes.defer(move |_| {
            held.recv().unwrap();
            vec![99]
        });
        bytes.clear();
        bytes.extend_from_slice(&[7, 8]);
        release.send(()).unwrap();
        configure(0).unwrap();
        assert_eq!(bytes.as_slice(), &[7, 8]);
        assert!(configure(5).is_err());
    }
}

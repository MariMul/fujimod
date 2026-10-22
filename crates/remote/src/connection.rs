//! A connection resolves references and shares the same action futures on every host.
use crate::{backend::Backend, memory::Memory, *};
use std::rc::Rc;

pub struct Connection<B> {
    pub(crate) backend: B,
    pub(crate) facts: Rc<Facts>,
}
impl<B: Backend> Connection<B> {
    pub fn connect(backend: B, facts: Facts) -> Result<Self> {
        facts.validate()?;
        Ok(Self {
            backend,
            facts: Rc::new(facts),
        })
    }
    pub fn facts(&self) -> &Facts {
        &self.facts
    }
    async fn ready(&mut self, deadline: u64) -> Result<()> {
        loop {
            let mut memory = Memory {
                backend: &mut self.backend,
                facts: &self.facts,
                reads: ReadSet::default(),
            };
            let state = memory.state().await?;
            let reads = memory.reads;
            if state.ready {
                return Ok(());
            }
            if self.now() >= deadline {
                return Err(format!("Timed out waiting for input readiness: {state:?}"));
            }
            self.wait(&reads, deadline).await?;
        }
    }
    pub async fn press(&mut self, control: &Control, deadline: u64) -> Result<()> {
        crate::operations::press(self, control, deadline).await
    }
    pub async fn turn(&mut self, control: &Control, steps: i32, deadline: u64) -> Result<()> {
        crate::operations::turn(self, control, steps, deadline).await
    }
    pub async fn navigate(&mut self, menu: &Menu, deadline: u64) -> Result<Snapshot> {
        let facts = self.facts.clone();
        crate::operations::navigate(self, &facts, menu, deadline).await
    }
    pub async fn select(&mut self, option: &OptionReference, deadline: u64) -> Result<()> {
        let facts = self.facts.clone();
        crate::operations::select(self, &facts, option, deadline).await
    }
    pub async fn screenshot(&mut self, deadline: u64) -> Result<Capture> {
        crate::operations::screenshot(self, deadline).await
    }
}
struct View<'a, B> {
    backend: &'a mut B,
    facts: &'a Facts,
}
impl<B: Backend> Observation for View<'_, B> {
    fn now(&self) -> u64 {
        self.backend.now()
    }
    fn read<'a>(&'a mut self, reference: &'a Reference) -> LocalFuture<'a, Snapshot> {
        Box::pin(async move {
            Memory {
                backend: self.backend,
                facts: self.facts,
                reads: ReadSet::default(),
            }
            .snapshot(reference)
            .await
        })
    }
    fn read_memory<'a>(&'a mut self, address: u64, size: usize) -> LocalFuture<'a, MemoryRead> {
        Box::pin(async move {
            let mut memory = Memory {
                backend: self.backend,
                facts: self.facts,
                reads: ReadSet::default(),
            };
            let bytes = memory.bytes(address, size).await?;
            Ok(MemoryRead {
                address,
                bytes,
                reads: memory.reads,
            })
        })
    }
    fn frame(&mut self) -> LocalFuture<'_, Capture> {
        Box::pin(async move {
            self.backend
                .frame(self.facts, &mut ReadSet::default())
                .await
        })
    }
}
impl<B: Backend> Observation for Connection<B> {
    fn now(&self) -> u64 {
        self.backend.now()
    }
    fn read<'a>(&'a mut self, reference: &'a Reference) -> LocalFuture<'a, Snapshot> {
        Box::pin(async move {
            View {
                backend: &mut self.backend,
                facts: &self.facts,
            }
            .read(reference)
            .await
        })
    }
    fn read_memory<'a>(&'a mut self, address: u64, size: usize) -> LocalFuture<'a, MemoryRead> {
        Box::pin(async move {
            View {
                backend: &mut self.backend,
                facts: &self.facts,
            }
            .read_memory(address, size)
            .await
        })
    }
    fn frame(&mut self) -> LocalFuture<'_, Capture> {
        Box::pin(async move {
            self.backend
                .frame(&self.facts, &mut ReadSet::default())
                .await
        })
    }
}
impl<B: Backend> Remote for Connection<B> {
    fn wait<'a>(&'a mut self, reads: &'a ReadSet, deadline: u64) -> LocalFuture<'a, ()> {
        Box::pin(async move { self.backend.wait(reads, deadline).await })
    }
    fn set_input<'a>(&'a mut self, input: &'a Input, deadline: u64) -> LocalFuture<'a, ()> {
        Box::pin(async move {
            if input.state != InputState::Released {
                self.ready(deadline).await?;
            }
            // Resolve physical identities immediately before supplying this edge.
            let snapshot = self
                .read(&Reference::Control(input.control.clone()))
                .await?;
            if matches!(
                snapshot.value,
                Value::Control(ControlSnapshot {
                    supported: false,
                    ..
                })
            ) {
                return Err(format!("Control {} is unavailable", input.control.id()));
            }
            let mut resolved = input.clone();
            self.backend.input(&resolved, &self.facts).await?;
            loop {
                let mut reads = ReadSet::default();
                if self.backend.accepted(&resolved, &mut reads)? {
                    return Ok(());
                }
                if self.now() >= deadline {
                    return Err(format!("Timed out waiting for input acceptance: {input:?}"));
                }
                self.wait(&reads, deadline).await?;
            }
        })
    }
}

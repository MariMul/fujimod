//! Transport details below reference resolution. These are mechanisms, not actions.
use crate::{Capture, Input, LocalFuture, ReadSet, Result};
use fujimod_catalog::remote::Facts;

pub trait Backend {
    fn now(&self) -> u64;
    fn memory<'a>(
        &'a mut self,
        address: u64,
        size: usize,
        reads: &'a mut ReadSet,
    ) -> LocalFuture<'a, Vec<u8>>;
    fn input<'a>(&'a mut self, input: &'a Input, facts: &'a Facts) -> LocalFuture<'a, ()>;
    fn accepted(&mut self, input: &Input, reads: &mut ReadSet) -> Result<bool>;
    fn frame<'a>(
        &'a mut self,
        facts: &'a Facts,
        reads: &'a mut ReadSet,
    ) -> LocalFuture<'a, Capture>;
    /// One bounded continuation, returning at an observed change or the deadline.
    fn wait<'a>(&'a mut self, reads: &'a ReadSet, deadline: u64) -> LocalFuture<'a, ()>;
    fn control_position(&mut self, id: &str) -> Result<Option<String>>;
}

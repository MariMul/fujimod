//! Paused host instrumentation on the shared runtime, outside Session state.
use super::*;
impl Board {
    pub(super) fn clear_observation(&mut self) {
        if let Some(Backend::Graph(session)) = self.backend.as_ref() {
            if let Some(mut watch) = self.function_watch.take() {
                let _ = session.board.function_watch_end(&mut watch);
            }
            if let Some(mut watch) = self.memory_watch.take() {
                let _ = session.board.memory_watch_end(&mut watch);
            }
            for (core, mut points) in std::mem::take(&mut self.instruction_points) {
                let _ = session.board.instruction_points_end(core, &mut points);
            }
        }
    }
}

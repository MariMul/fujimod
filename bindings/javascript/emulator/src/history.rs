//! Same-owner keyframe IDs and presentation state owned by this transport.
use super::session::Session;
use fujimod_emulator::session::{
    checkpoint::Snapshot,
    history::{Boundary, Entry, History as CoreHistory, Input},
};
use fujimod_emulator_devices::graphics::scanout::Frame;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

const HISTORY_LIMIT: usize = 32;
const CHECKPOINT_LIMIT: usize = 8;
// Separate device-state budgets leave room for RAM versions, live media and cold construction.
const DEVICE_BYTE_LIMIT: usize = 128 << 20;

#[derive(serde::Serialize)]
struct LogView<'a> {
    schema: u32,
    owner: u32,
    generation: u64,
    entries: &'a [Entry],
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Log {
    schema: u32,
    owner: u32,
    generation: u64,
    entries: Vec<Entry>,
}
fn boundary(value: Boundary) -> Value {
    json!({"tick":value.tick.to_string(),"instructions":value.instructions.iter().map(u64::to_string).collect::<Vec<_>>()})
}

#[derive(Clone)]
struct Presentation {
    boundary: Boundary,
    frames: BTreeMap<usize, Arc<Frame>>,
}
struct Checkpoint {
    id: String,
    core: Snapshot,
    presentation: Presentation,
}
pub(super) struct History {
    core: CoreHistory,
    marks: VecDeque<Presentation>,
    checkpoints: VecDeque<Checkpoint>,
    next: u32,
}
impl History {
    fn new() -> Result<Self, String> {
        Ok(Self {
            core: CoreHistory::new(HISTORY_LIMIT, DEVICE_BYTE_LIMIT)?,
            marks: VecDeque::new(),
            checkpoints: VecDeque::new(),
            next: 0,
        })
    }
    fn branch(&mut self) -> Result<(), String> {
        self.core = CoreHistory::new(HISTORY_LIMIT, DEVICE_BYTE_LIMIT)?;
        self.marks.clear();
        Ok(())
    }
}
impl Session {
    fn presentation(&self) -> Presentation {
        Presentation {
            boundary: Boundary::capture(&self.core),
            frames: self.frames.clone(),
        }
    }
    pub(super) fn collect_frames(&mut self) {
        for frame in self.core.board.take_frames() {
            self.frames.insert(frame.output, Arc::new(frame));
        }
    }
    fn restored(&mut self) {
        self.failed = self.core.board.state()["failure"].is_string();
        self.scene_upload.clear();
        self.scene_dimensions = None;
        self.collect_frames();
    }
    pub(super) fn checkpoint(&mut self, handle: u32) -> Result<Value, String> {
        let next = self
            .history
            .as_ref()
            .map_or(0, |h| h.next)
            .checked_add(1)
            .ok_or("checkpoint ID space exhausted")?;
        let previous = self
            .history
            .as_ref()
            .and_then(|h| h.checkpoints.back())
            .map(|c| &c.core);
        let saved = self.core.checkpoint(previous)?;
        let presentation = self.presentation();
        let id = format!("{handle}:{}:{next}", self.generation);
        let mut result = json!({"id":id,"tick":saved.now().to_string(),"instructions":saved.instruction_counts().iter().map(u64::to_string).collect::<Vec<_>>()});
        if self.history.is_none() {
            self.history = Some(History::new()?);
        }
        let history = self.history.as_mut().expect("initialized history");
        history.checkpoints.push_back(Checkpoint {
            id,
            core: saved,
            presentation,
        });
        history.next = next;
        while history.checkpoints.len() > 1
            && (history.checkpoints.len() > CHECKPOINT_LIMIT
                || history
                    .checkpoints
                    .iter()
                    .map(|c| c.core.encoded_device_bytes())
                    .fold(0usize, usize::saturating_add)
                    > DEVICE_BYTE_LIMIT)
        {
            history.checkpoints.pop_front();
        }
        result["retainedIds"] = json!(
            history
                .checkpoints
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<_>>()
        );
        Ok(result)
    }
    pub(super) fn mark_input(&mut self) -> Result<(), String> {
        if self.history.is_some() {
            let presentation = self.presentation();
            let history = self.history.as_mut().expect("active history");
            history.core.mark(&self.core)?;
            history.marks.push_back(presentation);
            while history.marks.len() > history.core.retained_marks() {
                history.marks.pop_front();
            }
        }
        Ok(())
    }
    pub(super) fn apply_input(&mut self, input: Input) -> Result<(), String> {
        match &mut self.history {
            Some(history) => history.core.apply(&mut self.core, input),
            // No machine capture or retained log before the first checkpoint.
            None => CoreHistory::new(1, DEVICE_BYTE_LIMIT)?.apply(&mut self.core, input),
        }
    }
    pub(super) fn restore_checkpoint(&mut self, id: &str) -> Result<Value, String> {
        let history = self
            .history
            .as_mut()
            .ok_or("unknown or expired checkpoint ID")?;
        let saved = history
            .checkpoints
            .iter()
            .find(|c| c.id == id)
            .ok_or("unknown or expired checkpoint ID")?;
        self.core.restore(&saved.core)?;
        self.frames = saved.presentation.frames.clone();
        history.branch()?;
        self.restored();
        Ok(boundary(Boundary::capture(&self.core)))
    }
    pub(super) fn undo(&mut self) -> Result<Value, String> {
        let history = self.history.as_mut().ok_or("no retained Undo keyframe")?;
        let presentation = history
            .marks
            .back()
            .ok_or("no retained Undo keyframe")?
            .clone();
        let restored = history.core.undo(&mut self.core)?;
        history.marks.pop_back();
        self.frames = presentation.frames;
        self.restored();
        Ok(boundary(restored))
    }
    pub(super) fn rollback(&mut self, tick: u64) -> Result<Value, String> {
        let history = self
            .history
            .as_mut()
            .ok_or("rollback target precedes retained history")?;
        let index = history
            .marks
            .iter()
            .rposition(|p| p.boundary.tick <= tick)
            .ok_or("rollback target precedes retained history")?;
        let restored = history.core.rollback(&mut self.core, tick)?;
        self.frames = history.marks[index].frames.clone();
        history.marks.truncate(index + 1);
        self.restored();
        Ok(boundary(restored))
    }
    pub(super) fn replay(&mut self, handle: u32, bytes: &[u8]) -> Result<Value, String> {
        if bytes.is_empty() || bytes.len() > 64 << 20 {
            return Err("input log must contain CBOR and fit within 64 MiB".into());
        }
        let mut input = bytes;
        let log: Log =
            ciborium::from_reader(&mut input).map_err(|e| format!("input log decode: {e}"))?;
        if log.schema != 1 || !input.is_empty() {
            return Err("invalid input log schema or trailing bytes".into());
        }
        if log.owner != handle || log.generation != self.generation {
            return Err("input log belongs to another session".into());
        }
        if log.entries.len() > 4096 {
            return Err("replay exceeds 4096 entries".into());
        }
        let history = self
            .history
            .as_mut()
            .ok_or("checkpoint is required before replay")?;
        let previous = history.checkpoints.back().map(|c| &c.core);
        let live = self.core.checkpoint(previous)?;
        if let Err(error) = CoreHistory::replay(&mut self.core, &log.entries) {
            self.core
                .restore(&live)
                .map_err(|restore| format!("{error}; replay recovery failed: {restore}"))?;
            return Err(error);
        }
        history.branch()?;
        self.restored();
        Ok(boundary(Boundary::capture(&self.core)))
    }
    pub(super) fn history_log(&mut self, handle: u32) -> Result<Value, String> {
        let entries = self.history.as_ref().map_or(&[][..], |h| h.core.entries());
        ciborium::into_writer(
            &LogView {
                schema: 1,
                owner: handle,
                generation: self.generation,
                entries,
            },
            &mut self.result_bytes,
        )
        .map_err(|e| format!("input log encode: {e}"))?;
        Ok(json!({"format":"session-input-log-cbor","schema":1,"entries":entries.len()}))
    }
}

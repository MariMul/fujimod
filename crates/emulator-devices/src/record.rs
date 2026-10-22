//! Event-log records and effects returned by device models.
use std::{borrow::Cow, sync::Arc};

/// A structured scalar, sequence or mapping in an event-log record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Value {
    None,
    Int(i128),
    Float(f64),
    Bool(bool),
    Str(String),
    /// A constant string (evidence, limits, provenance), without allocation.
    Static(Cow<'static, str>),
    List(Vec<Value>),
    Tuple(Vec<Value>),
    Bytes(Vec<u8>),
    /// Ordered dictionaries in declared endpoint data and transaction records.
    Dict(Vec<(Value, Value)>),
}

impl Value {
    /// A source label is borrowed during execution and owned after decoding;
    /// restoring metadata never leaks strings into a global intern table.
    pub fn constant(value: &'static str) -> Self {
        Self::Static(Cow::Borrowed(value))
    }

    /// Python's `hex(value)`.
    pub fn hex(value: u128) -> Value {
        Value::Str(format!("{value:#x}"))
    }
    pub fn int(value: impl Into<i128>) -> Value {
        Value::Int(value.into())
    }
    pub fn opt(value: Option<i64>) -> Value {
        value.map_or(Value::None, |v| Value::Int(v as i128))
    }
    pub fn str(value: &str) -> Value {
        Value::Str(value.to_owned())
    }
    /// True for a string key equal to `key` (dict lookups by name).
    pub fn is_key(&self, key: &str) -> bool {
        matches!(self, Value::Str(s) if s == key)
            || matches!(self, Value::Static(s) if s.as_ref() == key)
    }
    /// A dict with constant string keys, in the reference's field order.
    pub fn dict(fields: Vec<(&'static str, Value)>) -> Value {
        Value::Dict(
            fields
                .into_iter()
                .map(|(k, v)| (Value::constant(k), v))
                .collect(),
        )
    }
}

// Runtimes may move devices across threads: record values must stay Send.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<Value>();
    assert_send::<Action>();
};

/// One named observation with fields in insertion order.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Record {
    pub name: Cow<'static, str>,
    /// Published observations are immutable; whole-board captures share their
    /// field trees instead of recursively copying every retained event.
    pub fields: Arc<[(Cow<'static, str>, Value)]>,
}

impl Record {
    pub fn new<K: Into<Cow<'static, str>>>(
        name: impl Into<Cow<'static, str>>,
        fields: impl IntoIterator<Item = (K, Value)>,
    ) -> Self {
        Self {
            name: name.into(),
            fields: fields.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        }
    }
}

/// An effect leaving the device, applied by the host in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// `board.queue_irq(cpu, line)`.
    QueueIrq(u8, u8),
    /// The device's own `_record(name, **fields)`: its trace and the
    /// board's event log.
    Record(Record),
    /// `board.record(name, **fields)`: the board's event log only.
    Log(Record),
    /// Call the device's `complete(token)` `delay` ticks from now, ordered
    /// with every other scheduled callback as `board.schedule(delay, cb)`
    /// orders them.
    Schedule { delay: u64, token: u64 },
}

/// A modeled refusal (`UnsupportedDevice` with this message).
#[derive(Clone, Debug, PartialEq)]
pub struct Unsupported(pub String);

impl From<&str> for Unsupported {
    fn from(message: &str) -> Self {
        Unsupported(message.to_owned())
    }
}

impl From<String> for Unsupported {
    fn from(message: String) -> Self {
        Unsupported(message)
    }
}

pub type Result<T> = std::result::Result<T, Unsupported>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_event_fields_preserve_json_and_binary_checkpoint_layout() {
        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        struct OwnedRecord {
            name: Cow<'static, str>,
            fields: Vec<(Cow<'static, str>, Value)>,
        }
        let owned = OwnedRecord {
            name: Cow::Owned("Completed graphics batch".into()),
            fields: vec![
                (Cow::Borrowed("token"), Value::Int(17)),
                (
                    Cow::Owned("destination".into()),
                    Value::dict(vec![
                        ("bytes", Value::Bytes((0..64).collect())),
                        (
                            "bounds",
                            Value::Tuple(vec![Value::Int(720), Value::Int(480)]),
                        ),
                    ]),
                ),
            ],
        };
        let record = Record::new(owned.name.clone(), owned.fields.clone());
        assert_eq!(
            serde_json::to_vec(&record).unwrap(),
            serde_json::to_vec(&owned).unwrap()
        );
        let mut before = Vec::new();
        let mut after = Vec::new();
        ciborium::into_writer(&owned, &mut before).unwrap();
        ciborium::into_writer(&record, &mut after).unwrap();
        assert_eq!(before, after);
        let restored: Record = ciborium::from_reader(before.as_slice()).unwrap();
        assert_eq!(restored, record);
        let decoded: OwnedRecord = ciborium::from_reader(after.as_slice()).unwrap();
        assert_eq!(decoded, owned);
    }
}

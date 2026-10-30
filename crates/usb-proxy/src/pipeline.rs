//! Ordered USB operations and reply guards for one controller.
use crate::{Completion, Error, Message, Operation, Result, protocol::expectation_holds};
use std::{
    collections::{HashMap, VecDeque},
    future::Future,
};

/// Maximum bytes retained from successful IN results for later expectations.
const KEPT_BYTES: usize = 16 * 1024 * 1024;

/// One controller's ordered pipeline at an exporter.
///
/// After an operation or expectation fails, later operations and expectations
/// are discarded until a sync message arrives.
#[derive(Default)]
pub struct Pipeline {
    failed: bool,
    discarded: u64,
    results: HashMap<u64, Vec<u8>>,
    result_order: VecDeque<u64>,
    kept: usize,
}

impl Pipeline {
    /// Whether `message` belongs to the pipeline rather than being a notice.
    pub fn carries(message: &Message) -> bool {
        matches!(
            message,
            Message::Execute { .. } | Message::Expect { .. } | Message::Sync { .. }
        )
    }

    /// Process one pipeline message. `execute` runs an Execute and returns its
    /// completion; it is called only when the pipeline has not failed.
    pub async fn handle<F, Fut>(&mut self, message: Message, execute: F) -> Result<Option<Message>>
    where
        F: FnOnce(Message) -> Fut,
        Fut: Future<Output = Completion>,
    {
        match message {
            Message::Sync { seq } => {
                let reply = Message::Synced {
                    seq,
                    discarded: self.discarded,
                };
                self.failed = false;
                self.discarded = 0;
                self.results.clear();
                self.result_order.clear();
                self.kept = 0;
                Ok(Some(reply))
            }
            Message::Execute {
                seq,
                device,
                timeout_ms,
                op,
            } => {
                if self.failed {
                    self.discarded = self.discarded.saturating_add(1);
                    return Ok(None);
                }
                let expected_out = match &op {
                    Operation::ControlOut { data, .. } | Operation::TransferOut { data, .. } => {
                        Some(data.len())
                    }
                    _ => None,
                };
                let mut completion = execute(Message::Execute {
                    seq,
                    device,
                    timeout_ms,
                    op,
                })
                .await;
                if completion.error.is_none()
                    && expected_out.is_some_and(|expected| completion.transferred != expected)
                {
                    completion.error = Some("short OUT transfer".into());
                }
                if completion.error.is_some() {
                    self.failed = true;
                } else if !completion.data.is_empty() {
                    self.keep(seq, completion.data.clone());
                }
                Ok(Some(Message::Result { seq, completion }))
            }
            Message::Expect {
                seq,
                of,
                offset,
                bytes,
                mask,
                length,
                alternatives,
            } => {
                if self.failed {
                    self.discarded = self.discarded.saturating_add(1);
                    return Ok(None);
                }
                let data = self.results.get(&of);
                let extent_matches = length
                    .is_none_or(|length| data.is_some_and(|data| data.len() == length as usize));
                let matches = extent_matches
                    && data.is_some_and(|data| {
                        let mask = mask.as_ref().map(|mask| mask.as_ref());
                        expectation_holds(data, offset as usize, &bytes, mask)
                            || alternatives.iter().any(|alternative| {
                                expectation_holds(data, offset as usize, alternative, mask)
                            })
                    });
                if matches {
                    return Ok(None);
                }

                self.failed = true;
                let start = offset as usize;
                let actual = data
                    .and_then(|data| {
                        start
                            .checked_add(bytes.len())
                            .and_then(|end| data.get(start..end))
                    })
                    .unwrap_or_default()
                    .to_vec();
                Ok(Some(Message::ExpectFailed { seq, actual }))
            }
            _ => Err(Error("invalid pipeline message".into())),
        }
    }

    fn keep(&mut self, seq: u64, data: Vec<u8>) {
        if let Some(previous) = self.results.get_mut(&seq) {
            self.kept -= previous.len();
            *previous = data;
        } else {
            self.result_order.push_back(seq);
            self.results.insert(seq, data);
        }
        self.kept += self.results[&seq].len();

        while self.kept > KEPT_BYTES {
            let Some(oldest) = self.result_order.pop_front() else {
                break;
            };
            if let Some(bytes) = self.results.remove(&oldest) {
                self.kept -= bytes.len();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_lite::future::block_on;
    use std::future::ready;

    fn execute_message(seq: u64, op: Operation) -> Message {
        Message::Execute {
            seq,
            device: "1".into(),
            timeout_ms: 1000,
            op,
        }
    }

    fn read(seq: u64) -> Message {
        execute_message(
            seq,
            Operation::TransferIn {
                endpoint: 0x82,
                length: 64,
            },
        )
    }

    fn write(seq: u64, endpoint: u8) -> Message {
        execute_message(
            seq,
            Operation::TransferOut {
                endpoint,
                data: vec![seq as u8],
            },
        )
    }

    fn expect(
        seq: u64,
        of: u64,
        bytes: Vec<u8>,
        mask: Option<Vec<u8>>,
        length: Option<u32>,
    ) -> Message {
        Message::Expect {
            seq,
            of,
            offset: 1,
            bytes,
            mask: mask.map(Into::into),
            length,
            alternatives: vec![],
        }
    }

    fn reply_shape(reply: Option<Message>) -> Option<Message> {
        reply.map(|message| match message {
            Message::Result { seq, .. } => Message::Result {
                seq,
                completion: Completion::default(),
            },
            other => other,
        })
    }

    #[test]
    fn runs_in_order_checks_expectations_discards_until_sync() {
        let mut ran = Vec::new();
        let mut pipeline = Pipeline::default();
        let messages = [
            read(1),
            expect(2, 1, vec![0x5a, 3], None, None),
            expect(3, 1, vec![0x50], Some(vec![0xf0]), None),
            write(4, 1),
            expect(5, 1, vec![0x5b], None, None),
            write(6, 1),
            read(7),
            Message::Sync { seq: 8 },
            write(9, 3),
            write(10, 1),
            Message::Sync { seq: 11 },
            write(12, 1),
            expect(13, 99, vec![1], None, None),
            Message::Sync { seq: 14 },
        ];

        let replies: Vec<_> = messages
            .into_iter()
            .map(|message| {
                block_on(pipeline.handle(message, |message| {
                    let Message::Execute { seq, op, .. } = message else {
                        unreachable!("only Execute messages are passed to the callback")
                    };
                    ran.push(seq);
                    let completion = match op {
                        Operation::TransferIn { .. } => Completion {
                            data: vec![seq as u8, 0x5a, 3],
                            transferred: 3,
                            ..Completion::default()
                        },
                        Operation::TransferOut { endpoint, data } => Completion {
                            transferred: data.len(),
                            error: (endpoint == 3).then(|| "stall".into()),
                            ..Completion::default()
                        },
                        _ => unreachable!(),
                    };
                    ready(completion)
                }))
                .unwrap()
            })
            .map(reply_shape)
            .collect();

        assert_eq!(ran, [1, 4, 9, 12]);
        assert_eq!(
            replies,
            [
                Some(Message::Result {
                    seq: 1,
                    completion: Completion::default(),
                }),
                None,
                None,
                Some(Message::Result {
                    seq: 4,
                    completion: Completion::default(),
                }),
                Some(Message::ExpectFailed {
                    seq: 5,
                    actual: vec![0x5a],
                }),
                None,
                None,
                Some(Message::Synced {
                    seq: 8,
                    discarded: 2,
                }),
                Some(Message::Result {
                    seq: 9,
                    completion: Completion::default(),
                }),
                None,
                Some(Message::Synced {
                    seq: 11,
                    discarded: 1,
                }),
                Some(Message::Result {
                    seq: 12,
                    completion: Completion::default(),
                }),
                Some(Message::ExpectFailed {
                    seq: 13,
                    actual: vec![],
                }),
                Some(Message::Synced {
                    seq: 14,
                    discarded: 0,
                }),
            ]
        );
    }

    #[test]
    fn reply_guards_require_exact_extent_and_one_complete_alternative() {
        for (data, accepted) in [
            (vec![0, 0], true),
            (vec![1, 255], true),
            (vec![1, 0], false),
            (vec![0, 255], false),
            (vec![0], false),
            (vec![0, 0, 0], false),
        ] {
            let mut ran = Vec::new();
            let mut pipeline = Pipeline::default();
            block_on(pipeline.handle(read(1), |_| {
                ready(Completion {
                    transferred: data.len(),
                    data: data.clone(),
                    ..Completion::default()
                })
            }))
            .unwrap();
            let guard = block_on(pipeline.handle(
                Message::Expect {
                    seq: 2,
                    of: 1,
                    offset: 0,
                    bytes: vec![0, 0],
                    mask: None,
                    length: Some(2),
                    alternatives: vec![vec![1, 255].into()],
                },
                |_| ready(Completion::default()),
            ))
            .unwrap();
            block_on(pipeline.handle(write(3, 1), |message| {
                if let Message::Execute { seq, .. } = message {
                    ran.push(seq);
                }
                ready(Completion::default())
            }))
            .unwrap();
            assert_eq!(ran, if accepted { vec![3] } else { vec![] });
            assert_eq!(
                matches!(guard, Some(Message::ExpectFailed { .. })),
                !accepted
            );
        }
    }

    #[test]
    fn short_successful_out_prevents_dependent_operations() {
        let mut ran = Vec::new();
        let mut pipeline = Pipeline::default();
        let completion = block_on(pipeline.handle(write(1, 1), |message| {
            if let Message::Execute { seq, .. } = message {
                ran.push(seq);
            }
            ready(Completion {
                transferred: 0,
                ..Completion::default()
            })
        }))
        .unwrap();
        assert!(matches!(completion,
            Some(Message::Result { completion: Completion { error: Some(error), .. }, .. })
                if error == "short OUT transfer"));
        block_on(pipeline.handle(read(2), |message| {
            if let Message::Execute { seq, .. } = message {
                ran.push(seq);
            }
            ready(Completion::default())
        }))
        .unwrap();
        assert_eq!(ran, [1]);
    }

    #[test]
    fn keeps_the_newest_results_within_the_byte_limit() {
        let mut pipeline = Pipeline::default();
        let half = KEPT_BYTES / 2;
        pipeline.keep(10, vec![1; half]);
        pipeline.keep(20, vec![2; half]);
        assert_eq!(pipeline.kept, KEPT_BYTES);

        pipeline.keep(30, vec![3]);
        assert!(!pipeline.results.contains_key(&10));
        assert!(pipeline.results.contains_key(&20));
        assert!(pipeline.results.contains_key(&30));
        assert_eq!(pipeline.kept, half + 1);
    }

    #[test]
    fn carries_only_pipeline_messages() {
        assert!(Pipeline::carries(&read(1)));
        assert!(Pipeline::carries(&expect(2, 1, vec![1], None, None)));
        assert!(Pipeline::carries(&Message::Sync { seq: 3 }));
        assert!(!Pipeline::carries(&Message::ProgressDone));
        assert!(!Pipeline::carries(&Message::Goodbye {
            reason: "done".into(),
        }));
    }
}

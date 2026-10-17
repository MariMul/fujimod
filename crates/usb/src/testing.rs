//! Optional strict replay of synthetic USB exchanges through the real ff80 core.
//! This module is excluded unless the `testing` feature is enabled.
use crate::{
    Error, Result,
    connection::Io,
    error::TransferStage,
    ff80::{Session, framing::RequestKind},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Exchange {
    Control([u8; 12]),
    Write(Vec<u8>),
    Read {
        maximum: usize,
        data: Vec<u8>,
    },
    ReadFailure {
        maximum: usize,
        error: nusb::transfer::TransferError,
        partial: Vec<u8>,
    },
}
#[derive(Clone)]
pub struct Script {
    state: Arc<Mutex<State>>,
}
struct State {
    position: usize,
    steps: VecDeque<Exchange>,
}
impl Script {
    pub fn new(steps: impl IntoIterator<Item = Exchange>) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                position: 0,
                steps: steps.into_iter().collect(),
            })),
        }
    }
    pub fn remaining(&self) -> usize {
        self.state.lock().expect("replay lock").steps.len()
    }
    pub async fn begin_session(&self, max_reply_bytes: u32) -> Result<Session> {
        Session::replay(self.clone(), max_reply_bytes).await
    }
    fn take(&self) -> (usize, Option<Exchange>) {
        let mut state = self.state.lock().expect("replay lock");
        let position = state.position;
        state.position += 1;
        (position, state.steps.pop_front())
    }
    fn expect(&self, actual: Exchange) -> Result<()> {
        let (position, expected) = self.take();
        if expected.as_ref() == Some(&actual) {
            Ok(())
        } else {
            Err(Error::ReplayMismatch {
                position,
                expected: expected.map(Box::new),
                actual: Box::new(actual),
            })
        }
    }
}
impl Io for Script {
    async fn control(&mut self, _kind: RequestKind, bytes: [u8; 12]) -> Result<()> {
        self.expect(Exchange::Control(bytes))
    }
    async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.expect(Exchange::Write(bytes.to_vec()))
    }
    async fn read(&mut self, maximum: usize) -> Result<Vec<u8>> {
        let (position, expected) = self.take();
        match expected {
            Some(Exchange::Read {
                maximum: expected,
                data,
            }) if expected == maximum => Ok(data),
            Some(Exchange::ReadFailure {
                maximum: expected,
                error,
                partial,
            }) if expected == maximum => Err(Error::Transfer {
                stage: TransferStage::BulkIn,
                source: error,
                transferred: Some(partial.len()),
                partial,
            }),
            expected => Err(Error::ReplayMismatch {
                position,
                expected: expected.map(Box::new),
                actual: Box::new(Exchange::Read {
                    maximum,
                    data: vec![],
                }),
            }),
        }
    }
}

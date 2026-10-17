use super::*;
use crate::{
    error::TransferStage,
    ff80::commands::memory::{Address, DebugMemoryLimits, MemoryRange},
};
use futures_lite::future::{block_on, poll_once};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

enum Step {
    Control([u8; 12]),
    Write(Vec<u8>),
    Read(usize, Result<Vec<u8>>),
    PendingRead,
}
struct MockIo(Rc<RefCell<VecDeque<Step>>>);
impl Io for MockIo {
    async fn control(&mut self, _: RequestKind, bytes: [u8; 12]) -> Result<()> {
        match self.0.borrow_mut().pop_front().expect("unexpected control") {
            Step::Control(expected) => assert_eq!(bytes, expected),
            _ => panic!("expected a different exchange"),
        }
        Ok(())
    }
    async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        match self.0.borrow_mut().pop_front().expect("unexpected write") {
            Step::Write(expected) => assert_eq!(bytes, expected),
            _ => panic!("expected a different exchange"),
        }
        Ok(())
    }
    async fn read(&mut self, maximum: usize) -> Result<Vec<u8>> {
        let step = self.0.borrow_mut().pop_front().expect("unexpected read");
        match step {
            Step::Read(expected, result) => {
                assert_eq!(maximum, expected);
                result
            }
            Step::PendingRead => std::future::pending().await,
            _ => panic!("expected a different exchange"),
        }
    }
}
fn nz(n: u32) -> NonZeroU32 {
    NonZeroU32::new(n).unwrap()
}
fn read_request() -> ReadMemory {
    ReadMemory::new(
        MemoryRange::new(Address(0x12345678), nz(4)).unwrap(),
        DebugMemoryLimits {
            read: nz(256),
            write: nz(256),
        },
    )
    .unwrap()
}
fn opened(extra: Vec<Step>) -> (Core<MockIo>, Rc<RefCell<VecDeque<Step>>>) {
    let mut steps = VecDeque::from([
        Step::Control([0xc0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0]),
        Step::Read(8, Ok(vec![0x34, 0x12, 0x56, 0x78, 0, 0, 1, 0])),
    ]);
    steps.extend(extra);
    let shared = Rc::new(RefCell::new(steps));
    let core = block_on(Core::open(MockIo(shared.clone()), 1024)).unwrap();
    (core, shared)
}
fn command_steps(bytes: Vec<u8>, maximum: u32, result: Result<Vec<u8>>) -> Vec<Step> {
    vec![
        Step::Control(envelope(
            RequestKind::WriteCommand,
            bytes.len() as u32,
            0x1234,
        )),
        Step::Write(bytes),
        Step::Control(envelope(RequestKind::ReadCommand, maximum, 0x1234)),
        Step::Read(maximum as usize, result),
    ]
}

#[test]
fn session_id_decodes_little_endian_but_requests_encode_big_endian() {
    let (mut core, steps) = opened(vec![
        Step::Control([0xc2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0x12, 0x34]),
        Step::Read(2, Ok(vec![0x34, 0x12])),
        Step::Control([0xc4, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0x12, 0x34]),
        Step::Read(2, Ok(vec![0, 0])),
    ]);
    assert_eq!(core.info.id, 0x1234);
    assert_eq!(core.info.advertised_packet_bytes.get(), 256);
    assert_eq!(core.info.raw_open_reply[2..4], [0x56, 0x78]);
    assert_eq!(block_on(core.ping()).unwrap(), [0x34, 0x12]);
    block_on(core.close()).unwrap();
    assert!(steps.borrow().is_empty());
}

#[test]
fn direct_read_performs_one_command_and_returns_only_data() {
    let request = read_request();
    let mut reply = request.encode();
    reply.extend_from_slice(&[1, 2, 3, 4]);
    let (mut core, steps) = opened(command_steps(request.encode(), 40, Ok(reply)));
    assert_eq!(block_on(core.read_memory(request)).unwrap(), [1, 2, 3, 4]);
    assert_eq!(core.state, SessionState::Ready);
    assert!(steps.borrow().is_empty());
}

#[test]
fn direct_write_requires_a_matching_acknowledgement() {
    let command = WriteMemory::new(
        Address(4),
        vec![1, 2, 3, 4],
        DebugMemoryLimits {
            read: nz(256),
            write: nz(256),
        },
    )
    .unwrap();
    let reply = command.encode()[..36].to_vec();
    let (mut core, steps) = opened(command_steps(command.encode(), 36, Ok(reply)));
    block_on(core.write_memory(command)).unwrap();
    assert_eq!(core.state, SessionState::Ready);
    assert!(steps.borrow().is_empty());
}

#[test]
fn dropped_future_prevents_reusing_an_outstanding_read() {
    let (mut core, steps) = opened(vec![
        Step::Control(envelope(RequestKind::Ping, 2, 0x1234)),
        Step::PendingRead,
    ]);
    let mut pending = Box::pin(core.ping());
    assert!(block_on(poll_once(pending.as_mut())).is_none());
    drop(pending);
    assert_eq!(core.state, SessionState::CompletionUnknown);
    assert!(matches!(
        block_on(core.ping()),
        Err(Error::CompletionUnknown)
    ));
    assert!(matches!(
        block_on(core.close()),
        Err(Error::CompletionUnknown)
    ));
    assert!(steps.borrow().is_empty());
}

#[test]
fn device_error_is_completed_not_an_instruction_to_wait() {
    let request = read_request();
    let mut reply = request.encode()[..20].to_vec();
    reply[4..8].copy_from_slice(&1u32.to_le_bytes());
    reply[8..12].copy_from_slice(&0xdeadbeefu32.to_le_bytes());
    let mut script = command_steps(request.encode(), 40, Ok(reply));
    script.extend([
        Step::Control(envelope(RequestKind::Ping, 2, 0x1234)),
        Step::Read(2, Ok(vec![0, 0])),
    ]);
    let (mut core, steps) = opened(script);
    assert!(matches!(
        block_on(core.read_memory(request)),
        Err(Error::DeviceStatus {
            primary: 1,
            secondary: 0xdeadbeef,
            ..
        })
    ));
    assert_eq!(core.state, SessionState::Ready);
    block_on(core.ping()).unwrap();
    assert!(steps.borrow().is_empty());
}

#[test]
fn malformed_reply_poisons_session_and_is_not_retried() {
    let request = read_request();
    let mut reply = request.encode();
    reply.extend_from_slice(&[1, 2, 3, 4]);
    reply[28] ^= 1;
    let (mut core, steps) = opened(command_steps(request.encode(), 40, Ok(reply)));
    assert!(matches!(
        block_on(core.read_memory(request)),
        Err(Error::Protocol(ProtocolError::ParameterEcho))
    ));
    assert!(matches!(
        block_on(core.read_memory(request)),
        Err(Error::CompletionUnknown)
    ));
    assert!(steps.borrow().is_empty());
}

#[test]
fn short_reply_and_disconnect_preserve_uncertain_state() {
    for result in [
        Ok(vec![0; 3]),
        Err(Error::Transfer {
            stage: TransferStage::BulkIn,
            source: nusb::transfer::TransferError::Disconnected,
            transferred: Some(2),
            partial: vec![4, 5],
        }),
    ] {
        let request = read_request();
        let (mut core, steps) = opened(command_steps(request.encode(), 40, result));
        let error = block_on(core.read_memory(request)).unwrap_err();
        if let Error::Transfer {
            partial,
            transferred,
            ..
        } = error
        {
            assert_eq!(partial, [4, 5]);
            assert_eq!(transferred, Some(2));
        }
        assert_eq!(core.state, SessionState::CompletionUnknown);
        assert!(matches!(
            block_on(core.ping()),
            Err(Error::CompletionUnknown)
        ));
        assert!(steps.borrow().is_empty());
    }
}

#[test]
fn negotiated_limit_rejects_before_any_io() {
    let (mut core, steps) = opened(vec![]);
    core.info.advertised_packet_bytes = nz(2);
    assert!(matches!(
        block_on(core.read_memory(read_request())),
        Err(Error::LengthLimit {
            requested: 4,
            limit: 2
        })
    ));
    assert_eq!(core.state, SessionState::Ready);
    assert!(steps.borrow().is_empty());
}

#[test]
fn host_reply_budget_rejects_before_any_io() {
    let (mut core, steps) = opened(vec![]);
    core.max_reply_bytes = 39;
    assert!(matches!(
        block_on(core.read_memory(read_request())),
        Err(Error::LengthLimit {
            requested: 40,
            limit: 39
        })
    ));
    assert_eq!(core.state, SessionState::Ready);
    assert!(steps.borrow().is_empty());
}

#[test]
fn zero_advertised_limit_is_rejected() {
    let script = Rc::new(RefCell::new(VecDeque::from([
        Step::Control(envelope(RequestKind::Open, 8, 0)),
        Step::Read(8, Ok(vec![0; 8])),
    ])));
    assert!(matches!(
        block_on(Core::open(MockIo(script), 1024)),
        Err(Error::Protocol(ProtocolError::ZeroPacketLimit))
    ));
}

#[test]
fn device_error_with_wrong_function_does_not_unlock_session() {
    let request = read_request();
    let mut reply = request.encode()[..20].to_vec();
    reply[4..8].copy_from_slice(&1u32.to_le_bytes());
    reply[16] = 3;
    let (mut core, steps) = opened(command_steps(request.encode(), 40, Ok(reply)));
    assert!(matches!(
        block_on(core.read_memory(request)),
        Err(Error::Protocol(ProtocolError::FunctionEcho { .. }))
    ));
    assert_eq!(core.state, SessionState::CompletionUnknown);
    assert!(steps.borrow().is_empty());
}

#[test]
fn short_lifecycle_reply_and_timeout_block_following_commands() {
    for result in [Ok(vec![0]), Err(Error::Timeout(TransferStage::BulkIn))] {
        let (mut core, steps) = opened(vec![
            Step::Control(envelope(RequestKind::Ping, 2, 0x1234)),
            Step::Read(2, result),
        ]);
        assert!(block_on(core.ping()).is_err());
        assert_eq!(core.state, SessionState::CompletionUnknown);
        assert!(matches!(
            block_on(core.ping()),
            Err(Error::CompletionUnknown)
        ));
        assert!(steps.borrow().is_empty());
    }
}

#[test]
fn data_read_uses_exact_offset_and_collects_short_usb_transfers() {
    let (mut core, steps) = opened(vec![
        Step::Control([0xe1, 0, 0x89, 0xab, 0xcd, 0xef, 0, 0, 0, 5, 0x12, 0x34]),
        Step::Read(5, Ok(vec![1, 2])),
        Step::Read(3, Ok(vec![3, 4, 5])),
    ]);
    assert_eq!(
        block_on(core.read_data(0x89abcdef, nz(5))).unwrap(),
        [1, 2, 3, 4, 5]
    );
    assert_eq!(core.state, SessionState::Ready);
    assert!(steps.borrow().is_empty());
}
#[test]
fn failed_data_read_keeps_completed_bytes_and_blocks_reuse() {
    let (mut core, steps) = opened(vec![
        Step::Control(super::super::framing::data_envelope(
            RequestKind::ReadData,
            0,
            5,
            0x1234,
        )),
        Step::Read(5, Ok(vec![1, 2])),
        Step::Read(3, Ok(vec![])),
    ]);
    assert!(
        matches!(block_on(core.read_data(0,nz(5))),Err(Error::PartialRead { data_len:2, data, .. }) if data == [1,2])
    );
    assert_eq!(core.state, SessionState::CompletionUnknown);
    assert!(steps.borrow().is_empty());
}
#[test]
fn data_upload_does_not_implicitly_prepare_or_finalize() {
    let (mut core, steps) = opened(vec![
        Step::Control(envelope(RequestKind::WriteData, 3, 0x1234)),
        Step::Write(vec![1, 2, 3]),
        Step::Control(envelope(RequestKind::Ready, 32, 0x1234)),
        Step::Read(32, Ok(vec![42, 0, 0, 0])),
    ]);
    block_on(core.write_data(&[1, 2, 3])).unwrap();
    assert_eq!(block_on(core.data_status()).unwrap(), 42);
    assert!(steps.borrow().is_empty());
}

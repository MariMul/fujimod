use super::*;
fn nz(n: u32) -> NonZeroU32 {
    NonZeroU32::new(n).unwrap()
}
fn range(at: u32, n: u32) -> MemoryRange {
    MemoryRange::new(Address(at), nz(n)).unwrap()
}

#[test]
fn windows_check_aperture_and_require_explicit_wrapping() {
    assert!(
        DataWindow::new(
            Address(0x2000),
            range(0x1000, 0x2000),
            nz(512),
            OffsetMode::Checked
        )
        .is_err()
    );
    let window = DataWindow::new(
        Address(0x2000),
        range(0x1000, 0x2000),
        nz(512),
        OffsetMode::Wrapping,
    )
    .unwrap();
    assert_eq!(window.offset(Address(0x1000)).unwrap(), 0xfffff000);
    assert!(window.validate(range(0x1000, 0x2000)).is_ok());
    assert!(window.validate(range(0xfff, 1)).is_err());
    assert!(window.validate(range(0x2fff, 2)).is_err());
    let end = DataWindow::new(
        Address(u32::MAX),
        range(u32::MAX, 1),
        nz(1),
        OffsetMode::Checked,
    )
    .unwrap();
    assert!(end.validate(range(u32::MAX, 1)).is_ok());
}

#[cfg(feature = "testing")]
mod replay {
    use super::*;
    use crate::{
        ff80::{
            SessionState,
            framing::{RequestKind, data_envelope, envelope},
        },
        testing::{Exchange as E, Script},
    };
    use futures_lite::future::block_on;

    fn opened(steps: Vec<E>, budget: u32) -> (Session, Script) {
        let mut script = vec![
            E::Control(envelope(RequestKind::Open, 8, 0)),
            E::Read {
                maximum: 8,
                data: vec![0x34, 0x12, 0, 0, 0, 0, 0, 1],
            },
        ];
        script.extend(steps);
        let script = Script::new(script);
        (
            block_on(script.begin_session(budget, None)).unwrap(),
            script,
        )
    }
    fn read(offset: u32, bytes: &[u8]) -> Vec<E> {
        vec![
            E::Control(data_envelope(
                RequestKind::ReadData,
                offset,
                bytes.len() as u32,
                0x1234,
            )),
            E::Read {
                maximum: bytes.len(),
                data: bytes.to_vec(),
            },
        ]
    }
    #[test]
    fn chunks_use_capacity_and_wrapping_mapping_without_implicit_setup() {
        let mut steps = read(0xfffff000, b"abc");
        steps.extend(read(0xfffff003, b"def"));
        steps.extend(read(0xfffff006, b"g"));
        let (mut session, script) = opened(steps, 64);
        let window = DataWindow::new(
            Address(0x2000),
            range(0x1000, 64),
            nz(3),
            OffsetMode::Wrapping,
        )
        .unwrap();
        let mut reader = DataReader::new(&mut session, window, range(0x1000, 7)).unwrap();
        assert_eq!(block_on(reader.next_chunk()).unwrap().unwrap(), b"abc");
        assert_eq!(block_on(reader.next_chunk()).unwrap().unwrap(), b"def");
        assert_eq!(block_on(reader.next_chunk()).unwrap().unwrap(), b"g");
        assert!(block_on(reader.next_chunk()).unwrap().is_none());
        assert_eq!(reader.bytes_completed(), 7);
        assert_eq!(reader.into_inner().state(), SessionState::Ready);
        assert_eq!(script.remaining(), 0);
    }
    #[test]
    fn host_budget_bounds_bulk_commands_independently_of_debug_packet_size() {
        let mut steps = read(0, b"12345678");
        steps.extend(read(8, b"90"));
        let (mut session, script) = opened(steps, 8);
        let window = DataWindow::new(
            Address(0x1000),
            range(0x1000, 64),
            nz(32),
            OffsetMode::Checked,
        )
        .unwrap();
        let mut reader = DataReader::new(&mut session, window, range(0x1000, 10)).unwrap();
        assert_eq!(block_on(reader.next_chunk()).unwrap().unwrap(), b"12345678");
        assert_eq!(block_on(reader.next_chunk()).unwrap().unwrap(), b"90");
        assert_eq!(script.remaining(), 0);
    }
    #[test]
    fn failure_preserves_completed_chunks_and_current_partial_without_retry() {
        let mut steps = read(0, b"abcd");
        steps.extend([
            E::Control(data_envelope(RequestKind::ReadData, 4, 4, 0x1234)),
            E::Read {
                maximum: 4,
                data: b"e".to_vec(),
            },
            E::ReadFailure {
                maximum: 3,
                error: nusb::transfer::TransferError::Disconnected,
                partial: b"f".to_vec(),
            },
        ]);
        let (mut session, script) = opened(steps, 64);
        let window = DataWindow::new(
            Address(0x1000),
            range(0x1000, 64),
            nz(4),
            OffsetMode::Checked,
        )
        .unwrap();
        let mut reader = DataReader::new(&mut session, window, range(0x1000, 8)).unwrap();
        assert_eq!(block_on(reader.next_chunk()).unwrap().unwrap(), b"abcd");
        assert!(
            matches!(block_on(reader.next_chunk()), Err(Error::PartialRead { data_len: 2, data, .. }) if data == b"ef")
        );
        assert_eq!(reader.bytes_completed(), 4);
        assert!(matches!(
            block_on(reader.next_chunk()),
            Err(Error::CompletionUnknown)
        ));
        assert_eq!(reader.into_inner().state(), SessionState::CompletionUnknown);
        assert_eq!(script.remaining(), 0);
    }
    #[test]
    fn early_stop_returns_ready_session_without_reading_ahead() {
        let (session, script) = opened(read(0, b"abcd"), 64);
        let window = DataWindow::new(
            Address(0x1000),
            range(0x1000, 64),
            nz(4),
            OffsetMode::Checked,
        )
        .unwrap();
        let mut reader = DataReader::new(session, window, range(0x1000, 8)).unwrap();
        assert_eq!(block_on(reader.next_chunk()).unwrap().unwrap(), b"abcd");
        assert_eq!(reader.into_inner().state(), SessionState::Ready);
        assert_eq!(script.remaining(), 0);
    }
}

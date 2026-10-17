use super::*;
fn nz(n: u32) -> NonZeroU32 {
    NonZeroU32::new(n).unwrap()
}
#[test]
fn definition_rejects_incomplete_capacity_or_address_before_io() {
    assert!(Mailbox::new(0, 0x80, nz(32)).is_err());
    assert!(Mailbox::new(0, u32::MAX - 32, nz(256)).is_err());
    assert!(Mailbox::new(0, 0x80, nz(33)).is_ok());
}

#[cfg(feature = "testing")]
mod replay {
    use super::*;
    use crate::{
        Error,
        app80::CommandCode,
        ff80::SessionState,
        testing::{Exchange as E, Script},
    };
    use futures_lite::future::block_on;

    fn envelope(kind: u8, size: u32) -> [u8; 12] {
        let mut b = [0; 12];
        b[0] = kind;
        b[6..10].copy_from_slice(&size.to_be_bytes());
        b[10..12].copy_from_slice(&0x1234u16.to_be_bytes());
        b
    }
    fn opened(steps: Vec<E>, budget: u32) -> (Session, Script) {
        let mut open = envelope(0xc0, 8);
        open[10..12].fill(0);
        let mut all = vec![
            E::Control(open),
            E::Read {
                maximum: 8,
                data: vec![0x34, 0x12, 0, 0, 0, 0, 1, 0],
            },
        ];
        all.extend(steps);
        let script = Script::new(all);
        (block_on(script.begin_session(budget)).unwrap(), script)
    }
    fn write(offset: u32, bytes: &[u8]) -> Vec<E> {
        let command = WriteConfiguration::new(
            ConfigurationLocation {
                view: ConfigurationView::Ram,
                page: 0,
                offset,
            },
            bytes.to_vec(),
            nz(256),
        )
        .unwrap();
        let encoded = command.encode();
        vec![
            E::Control(envelope(0xd0, encoded.len() as u32)),
            E::Write(encoded.clone()),
            E::Control(envelope(0xd1, 36)),
            E::Read {
                maximum: 36,
                data: encoded[..36].to_vec(),
            },
        ]
    }
    #[test]
    fn exchange_stages_then_publishes_then_reads_without_poll_or_finalization() {
        let command = Request::new(CommandCode(0x22), &[0xff]).unwrap();
        let mut steps = write(0x81, command.arguments());
        steps.extend(write(0x80, &command.encode()));
        let read = ReadConfiguration::new(
            ConfigurationLocation {
                view: ConfigurationView::Ram,
                page: 0,
                offset: 0x80,
            },
            nz(33),
            nz(256),
        )
        .unwrap()
        .encode();
        let mut result = [7; 33];
        result[0] = 1; // Not a request to wait again.
        let mut response = read.clone();
        response.extend(result);
        steps.extend([
            E::Control(envelope(0xd0, 36)),
            E::Write(read),
            E::Control(envelope(0xd1, 69)),
            E::Read {
                maximum: 69,
                data: response,
            },
        ]);
        let (mut session, script) = opened(steps, 128);
        let mailbox = Mailbox::new(0, 0x80, nz(256)).unwrap();
        assert_eq!(
            block_on(mailbox.exchange(&mut session, &command))
                .unwrap()
                .raw(),
            &result
        );
        assert_eq!(session.state(), SessionState::Ready);
        assert_eq!(script.remaining(), 0);
    }
    #[test]
    fn publish_error_is_a_completed_error_not_a_host_pending_instruction() {
        let command = Request::new(CommandCode(2), &[]).unwrap();
        let mut steps = write(0x81, command.arguments());
        let mut publish = write(0x80, &command.encode());
        let E::Read { data, .. } = publish.last_mut().unwrap() else {
            unreachable!()
        };
        data[4..8].copy_from_slice(&1u32.to_le_bytes());
        data[8..12].copy_from_slice(&0xffffu32.to_le_bytes());
        steps.extend(publish);
        let (mut session, script) = opened(steps, 128);
        let result = block_on(
            Mailbox::new(0, 0x80, nz(256))
                .unwrap()
                .exchange(&mut session, &command),
        );
        assert!(matches!(
            result,
            Err(Error::DeviceStatus {
                primary: 1,
                secondary: 0xffff,
                ..
            })
        ));
        assert_eq!(session.state(), SessionState::Ready);
        assert_eq!(script.remaining(), 0);
    }
    #[test]
    fn loss_after_staging_never_replays_or_reads_mailbox() {
        let command = Request::new(CommandCode(6), &[]).unwrap();
        let mut steps = write(0x81, command.arguments());
        steps.push(E::ControlFailure {
            bytes: envelope(0xd0, 69),
            error: nusb::transfer::TransferError::Disconnected,
        });
        let (mut session, script) = opened(steps, 128);
        assert!(
            block_on(
                Mailbox::new(0, 0x80, nz(256))
                    .unwrap()
                    .exchange(&mut session, &command)
            )
            .is_err()
        );
        assert_eq!(session.state(), SessionState::CompletionUnknown);
        assert!(matches!(
            block_on(session.ping()),
            Err(Error::CompletionUnknown)
        ));
        assert_eq!(script.remaining(), 0);
    }
    #[test]
    fn cancelling_publish_completion_leaves_session_fenced() {
        let command = Request::new(CommandCode(2), &[]).unwrap();
        let mut steps = write(0x81, command.arguments());
        let mut publish = write(0x80, &command.encode());
        *publish.last_mut().unwrap() = E::PendingRead { maximum: 36 };
        steps.extend(publish);
        let (mut session, script) = opened(steps, 128);
        let mailbox = Mailbox::new(0, 0x80, nz(256)).unwrap();
        {
            let mut future = std::pin::pin!(mailbox.exchange(&mut session, &command));
            assert!(block_on(futures_lite::future::poll_once(&mut future)).is_none());
        }
        assert_eq!(session.state(), SessionState::CompletionUnknown);
        assert!(matches!(
            block_on(session.ping()),
            Err(Error::CompletionUnknown)
        ));
        assert_eq!(script.remaining(), 0);
    }
    #[test]
    fn final_reply_budget_is_checked_before_any_setup_write() {
        let (mut session, script) = opened(vec![], 68);
        let result = block_on(
            Mailbox::new(0, 0x80, nz(256))
                .unwrap()
                .exchange(&mut session, &Request::new(CommandCode(0), &[]).unwrap()),
        );
        assert!(matches!(
            result,
            Err(Error::LengthLimit {
                requested: 69,
                limit: 68
            })
        ));
        assert_eq!(session.state(), SessionState::Ready);
        assert_eq!(script.remaining(), 0);
    }
}

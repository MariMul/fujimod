use super::*;
use crate::{backend::Backend, testing::facts};
use fujimod_usb::testing::{Exchange, Script};

fn envelope(kind: u8, size: u32) -> [u8; 12] {
    let mut bytes = [0; 12];
    bytes[0] = kind;
    bytes[6..10].copy_from_slice(&size.to_be_bytes());
    bytes[11] = 1;
    bytes
}
fn opened(steps: impl IntoIterator<Item = Exchange>) -> Script {
    let mut open = [0; 12];
    open[0] = 0xc0;
    open[9] = 8;
    Script::new(
        [
            Exchange::Control(open),
            Exchange::Read {
                maximum: 8,
                data: vec![1, 0, 0, 0, 0, 0, 1, 0],
            },
        ]
        .into_iter()
        .chain(steps),
    )
}
fn command(function: u32, parameters: [u8; 16], payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 36];
    bytes[..4].copy_from_slice(&0x200001u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&function.to_le_bytes());
    bytes[20..36].copy_from_slice(&parameters);
    bytes.extend(payload);
    bytes
}
fn exchanges(request: Vec<u8>, reply: Vec<u8>) -> Vec<Exchange> {
    vec![
        Exchange::Control(envelope(0xd0, request.len() as u32)),
        Exchange::Write(request),
        Exchange::Control(envelope(0xd1, reply.len() as u32)),
        Exchange::Read {
            maximum: reply.len(),
            data: reply,
        },
    ]
}
fn options() -> Options {
    Options {
        memory_limits: DebugMemoryLimits {
            read: NonZeroU32::new(4).unwrap(),
            write: NonZeroU32::new(4).unwrap(),
        },
        event_bytes: NonZeroU32::new(64).unwrap(),
        poll_interval: Duration::from_millis(1),
        output: "lcd".into(),
    }
}
#[test]
fn range_reads_use_alias_and_chunk_limits() {
    futures_lite::future::block_on(async {
        let mut steps = vec![];
        for (address, data) in [
            (0x40001000u32, vec![1, 2, 3, 4]),
            (0x40001004u32, vec![5, 6]),
        ] {
            let mut params = [0; 16];
            params[8..12].copy_from_slice(&address.to_le_bytes());
            params[12..16].copy_from_slice(&(data.len() as u32).to_le_bytes());
            steps.extend(exchanges(
                command(2, params, &[]),
                command(2, params, &data),
            ));
        }
        let script = opened(steps);
        let session = script.begin_session(1024, None).await.unwrap();
        let mut facts = facts();
        facts.firmware.uncached_alias = 0x40000000;
        let mut remote = Ff80Remote::new(session, facts, options()).unwrap();
        let bytes = remote
            .backend
            .memory(0x1000, 6, &mut ReadSet::default())
            .await
            .unwrap();
        assert_eq!(bytes, [1, 2, 3, 4, 5, 6]);
        assert_eq!(script.remaining(), 0);
    });
}
#[test]
fn debug_names_are_explicit_and_failed_events_are_not_replayed() {
    futures_lite::future::block_on(async {
        let payload = b"EXE_M ON\n";
        let mut parameters = [0; 16];
        parameters[12..].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        let mut reply = command(5, parameters, &[]);
        reply[4..8].copy_from_slice(&1u32.to_le_bytes());
        let script = opened(exchanges(command(5, parameters, payload), reply));
        let session = script.begin_session(1024, None).await.unwrap();
        let mut facts = facts();
        facts
            .firmware
            .debug_keys
            .iter_mut()
            .find(|k| k.name == "MENU")
            .unwrap()
            .name = "EXE_M".into();
        facts
            .firmware
            .input_bindings
            .insert("menu".into(), "EXE_M".into());
        facts
            .firmware
            .controls
            .controls
            .get_mut("menu")
            .unwrap()
            .keys = vec!["KEY_OK".into()];
        let mut remote = Ff80Remote::new(session, facts.clone(), options()).unwrap();
        assert!(
            remote
                .backend
                .input(
                    &Input {
                        control: control("menu"),
                        state: InputState::Pressed
                    },
                    &facts
                )
                .await
                .is_err()
        );
        assert_eq!(script.remaining(), 0);
    });
}

#[test]
fn scanner_names_never_supply_missing_debug_bindings() {
    futures_lite::future::block_on(async {
        let script = opened([]);
        let session = script.begin_session(1024, None).await.unwrap();
        let mut facts = facts();
        facts.firmware.input_bindings.remove("menu");
        let mut remote = Ff80Remote::new(session, facts.clone(), options()).unwrap();
        let error = remote
            .backend
            .input(
                &Input {
                    control: control("menu"),
                    state: InputState::Pressed,
                },
                &facts,
            )
            .await
            .unwrap_err();
        assert!(error.contains("explicit debug command binding"));
        assert_eq!(script.remaining(), 0);
    });
}

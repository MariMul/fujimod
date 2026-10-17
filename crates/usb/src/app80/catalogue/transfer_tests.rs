use super::*;
use fujimod_defs::service::FirmwareIdentity;
fn catalogue(reverse: bool) -> Catalogue {
    let layouts = [
        ("transfer_prepare", 1),
        ("transfer_select_operation", 2),
        ("transfer_ready", 1),
        ("transfer_select_channel", 2),
        ("transfer_start", 1),
    ];
    let entries:Vec<_>=layouts.into_iter().enumerate().map(|(n,(codec,count))|{
        let slots:Vec<_>=(0..count).map(|i|if reverse {31-i}else{i}).collect();
        serde_json::json!({"code":180+n,"codec":codec,"label":codec,"slots":slots,"enabled_modes":4,"next_mode":0,"firmware_kind":1,"handler":0,"parameter_table":0})
    }).collect();
    let json = serde_json::json!({"schema":1,"firmware":{"id":"transfer-b","model":"Synthetic","version":"2","sha256":"ab".repeat(32)},"mailbox":{"page":3,"offset":576,"configuration_limit":512},"entries":entries});
    Catalogue::new(
        Definition::from_json(
            &json.to_string(),
            &FirmwareIdentity::new("transfer-b", &"ab".repeat(32)).unwrap(),
        )
        .unwrap(),
    )
}
#[test]
fn canonical_transfer_selectors_and_outputs_are_firmware_selected() {
    for reverse in [false, true] {
        let c = catalogue(reverse);
        for (command, code, expected, index) in [
            (Command::TransferPrepare, 180, vec![255], 0),
            (Command::TransferReady, 182, vec![255], 0),
            (Command::TransferStart, 184, vec![255], 0),
            (
                Command::TransferSelectOperation {
                    operation: TransferOperation::Operation0,
                },
                181,
                vec![0, 255],
                1,
            ),
            (
                Command::TransferSelectOperation {
                    operation: TransferOperation::Operation1,
                },
                181,
                vec![1, 255],
                1,
            ),
            (
                Command::TransferSelectOperation {
                    operation: TransferOperation::Operation2,
                },
                181,
                vec![2, 255],
                1,
            ),
            (
                Command::TransferSelectOperation {
                    operation: TransferOperation::Operation3,
                },
                181,
                vec![3, 255],
                1,
            ),
            (
                Command::TransferSelectOperation {
                    operation: TransferOperation::Operation6,
                },
                181,
                vec![255, 255],
                1,
            ),
            (
                Command::TransferSelectChannel {
                    channel: TransferChannel::Operation1,
                },
                183,
                vec![0, 255],
                1,
            ),
            (
                Command::TransferSelectChannel {
                    channel: TransferChannel::Operation3,
                },
                183,
                vec![1, 255],
                1,
            ),
            (
                Command::TransferSelectChannel {
                    channel: TransferChannel::Operation4,
                },
                183,
                vec![2, 255],
                1,
            ),
        ] {
            let p = c.prepare(ServiceModes::new(4).unwrap(), command).unwrap();
            assert_eq!(p.request.code(), CommandCode(code));
            let logical: Vec<_> = p
                .slots
                .iter()
                .map(|&s| p.request.arguments()[usize::from(s)])
                .collect();
            assert_eq!(logical, expected);
            for raw in 0..=255 {
                let mut bytes = p.request.encode();
                bytes[1 + usize::from(p.slots[index])] = raw;
                let reply = p.decode(Reply::decode(&bytes).unwrap()).unwrap();
                assert_eq!(reply.raw().raw(), &bytes);
                let result = match raw {
                    0 => TransferResult::Success,
                    1 => TransferResult::Failed,
                    n => TransferResult::Unknown(n),
                };
                let value = match command {
                    Command::TransferPrepare => ReplyValue::TransferPrepared {
                        result: match raw {
                            0 => TransferPrepareResult::Success,
                            2 => TransferPrepareResult::PreparationError,
                            3 => TransferPrepareResult::Status7,
                            n => TransferPrepareResult::Unknown(n),
                        },
                    },
                    Command::TransferReady => ReplyValue::TransferReadiness { result },
                    Command::TransferStart => ReplyValue::TransferStarted { result },
                    Command::TransferSelectOperation { .. } => {
                        ReplyValue::TransferOperationSelected { result }
                    }
                    Command::TransferSelectChannel { .. } => {
                        ReplyValue::TransferChannelSelected { result }
                    }
                    _ => unreachable!(),
                };
                assert_eq!(reply.value(), &value);
            }
        }
    }
}

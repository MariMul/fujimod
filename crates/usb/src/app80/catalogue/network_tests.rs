use super::*;
use fujimod_defs::service::FirmwareIdentity;
fn catalogue(reverse: bool) -> Catalogue {
    let layouts = [
        ("nand_read_blocks", 3),
        ("option41", 1),
        ("wlan_control", 1),
        ("radio_control", 4),
        ("signal_strength", 7),
        ("iperf", 1),
    ];
    let entries:Vec<_>=layouts.into_iter().enumerate().map(|(n,(codec,count))|{
  let slots:Vec<_>=(0..count).map(|i|if reverse {31-i}else{i}).collect();
  serde_json::json!({"code":200+n,"codec":codec,"label":codec,"slots":slots,"enabled_modes":4,"next_mode":0,"firmware_kind":1,"handler":0,"parameter_table":0})
 }).collect();
    let json = serde_json::json!({"schema":1,"firmware":{"id":"radio-b","model":"Synthetic","version":"2","sha256":"ab".repeat(32)},"mailbox":{"page":3,"offset":576,"configuration_limit":512},"entries":entries});
    Catalogue::new(
        Definition::from_json(
            &json.to_string(),
            &FirmwareIdentity::new("radio-b", &"ab".repeat(32)).unwrap(),
        )
        .unwrap(),
    )
}
fn prepare(c: &Catalogue, command: Command) -> PreparedCommand {
    c.prepare(ServiceModes::new(4).unwrap(), command).unwrap()
}
fn logical(p: &PreparedCommand) -> Vec<u8> {
    p.slots
        .iter()
        .map(|&s| p.request.arguments()[usize::from(s)])
        .collect()
}
fn decode(p: &PreparedCommand, index: usize, byte: u8) -> CommandReply {
    let mut raw = p.request.encode();
    raw[usize::from(p.slots[index]) + 1] = byte;
    p.decode(Reply::decode(&raw).unwrap()).unwrap()
}
#[test]
fn radio_fields_remain_distinct_checked_and_firmware_selected() {
    assert!(RadioChannel::new(15).is_err());
    assert!(RadioPower::new(128).is_err());
    for reverse in [false, true] {
        let c = catalogue(reverse);
        for (mode, m) in [(RadioMode::Mode0, 0), (RadioMode::Mode1, 1)] {
            for channel in [0, 14] {
                for power in [0, 11, 12, 127] {
                    for (option, bit) in [(Toggle::Disabled, 0), (Toggle::Enabled, 128)] {
                        let p = prepare(
                            &c,
                            Command::RadioControl {
                                action: RadioAction::Primary {
                                    mode,
                                    channel: RadioChannel::new(channel).unwrap(),
                                    power: RadioPower::new(power).unwrap(),
                                    option,
                                },
                            },
                        );
                        assert_eq!(logical(&p), [m, channel, power | bit, 0]);
                        assert_eq!(p.request.code(), CommandCode(203));
                        assert_eq!(
                            p.decode(Reply::decode(&p.request.encode()).unwrap())
                                .unwrap()
                                .value(),
                            &ReplyValue::NoPayload
                        );
                    }
                }
            }
            let p = prepare(
                &c,
                Command::RadioControl {
                    action: RadioAction::Secondary {
                        mode,
                        channel: RadioChannel::new(14).unwrap(),
                    },
                },
            );
            assert_eq!(logical(&p), [m, 14, 0, 1]);
        }
        for (enabled, wire) in [(Toggle::Disabled, 0), (Toggle::Enabled, 1)] {
            assert_eq!(
                logical(&prepare(&c, Command::WlanControl { enabled })),
                [wire]
            );
        }
    }
}
#[test]
fn nand_and_option_results_are_data_not_callback_dispositions() {
    for reverse in [false, true] {
        let c = catalogue(reverse);
        for (spare, wire) in [(NandSpare::Omit, 0), (NandSpare::Include, 1)] {
            for block_group in [0, 255] {
                let p = prepare(&c, Command::NandReadBlocks { spare, block_group });
                assert_eq!(logical(&p), [wire, block_group, 255]);
                for raw in 0..=255 {
                    let expected = match raw {
                        0 => NandReadResult::Success,
                        1 => NandReadResult::RejectedOrWaitError,
                        2 => NandReadResult::OtherError,
                        n => NandReadResult::Unknown(n),
                    };
                    assert_eq!(
                        decode(&p, 2, raw).value(),
                        &ReplyValue::NandRead { status: expected }
                    );
                }
            }
        }
        let p = prepare(&c, Command::Option41);
        assert_eq!(logical(&p), [255]);
        for raw in 0..=255 {
            let expected = match raw {
                1 => Option41State::Zero,
                0 => Option41State::NonZero,
                n => Option41State::Unknown(n),
            };
            assert_eq!(
                decode(&p, 0, raw).value(),
                &ReplyValue::Option41 { state: expected }
            );
        }
    }
}
#[test]
fn peer_address_is_owned_and_opaque_observations_preserve_every_byte() {
    for n in [0, 5, 7] {
        assert!(PeerAddress::new(&vec![0; n]).is_err());
    }
    let mut input = [1, 2, 3, 4, 5, 6];
    let peer = PeerAddress::new(&input).unwrap();
    input.fill(0);
    assert_eq!(peer.data(), &[1, 2, 3, 4, 5, 6]);
    for reverse in [false, true] {
        let c = catalogue(reverse);
        let p = prepare(&c, Command::SignalStrength { address: peer });
        assert_eq!(logical(&p), [1, 2, 3, 4, 5, 6, 255]);
        let iperf = prepare(&c, Command::Iperf);
        assert_eq!(logical(&iperf), [255]);
        for raw in 0..=255 {
            assert_eq!(
                decode(&p, 6, raw).value(),
                &ReplyValue::SignalStrength { raw_value: raw }
            );
            assert_eq!(
                decode(&iperf, 0, raw).value(),
                &ReplyValue::Iperf { raw_result: raw }
            );
        }
    }
}

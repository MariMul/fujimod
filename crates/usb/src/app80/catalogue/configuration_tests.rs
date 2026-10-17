use super::*;
use fujimod_defs::service::FirmwareIdentity;
fn catalogue(reverse: bool) -> Catalogue {
    let entries: Vec<_> = [("configuration_flags",11),("configuration_mode",2),("configuration_pack_options",4),("configuration_read_options",4)]
        .into_iter().enumerate().map(|(n,(codec,count))| {
            let slots: Vec<_> = (0..count).map(|i| if reverse {31-i} else {i}).collect();
            serde_json::json!({"code":200+n,"codec":codec,"label":codec,"slots":slots,"enabled_modes":4,"next_mode":0,"firmware_kind":1,"handler":0,"parameter_table":0})
        }).collect();
    let doc = serde_json::json!({"schema":1,"firmware":{"id":"config-b","model":"Synthetic","version":"2","sha256":"ab".repeat(32)},"mailbox":{"page":3,"offset":576,"configuration_limit":512},"entries":entries});
    Catalogue::new(
        Definition::from_json(
            &doc.to_string(),
            &FirmwareIdentity::new("config-b", &"ab".repeat(32)).unwrap(),
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
        .map(|&i| p.request.arguments()[usize::from(i)])
        .collect()
}
#[test]
fn flags_selectors_are_distinct_and_apply_exposes_no_output() {
    for reverse in [false, true] {
        let c = catalogue(reverse);
        for (bank, b) in [(ConfigurationBank::Bank0, 0), (ConfigurationBank::Bank1, 1)] {
            for (option, o) in [(Toggle::Disabled, 0), (Toggle::Enabled, 1)] {
                let read = prepare(&c, Command::ReadConfigurationFlags { bank, option });
                assert_eq!(read.request.code(), CommandCode(200));
                assert_eq!(logical(&read), [vec![0, b, o], vec![255; 8]].concat());
                assert_eq!(
                    read.decode(Reply::decode(&read.request.encode()).unwrap())
                        .unwrap()
                        .value(),
                    &ReplyValue::ConfigurationFlags { raw_data: [255; 8] }
                );
                let apply = prepare(&c, Command::ApplyConfigurationFlags { bank, option });
                assert_eq!(logical(&apply), [vec![1, b, o], vec![0; 8]].concat());
                assert_eq!(
                    apply
                        .decode(Reply::decode(&apply.request.encode()).unwrap())
                        .unwrap()
                        .value(),
                    &ReplyValue::NoPayload
                );
            }
        }
    }
}
#[test]
fn mode_and_packed_fields_are_forwarded_without_host_interpretation() {
    for reverse in [false, true] {
        let c = catalogue(reverse);
        for (flag, f) in [(Toggle::Disabled, 0), (Toggle::Enabled, 1)] {
            for (mode, m) in [
                (ConfigurationMode::Mode0, 0),
                (ConfigurationMode::Mode1, 1),
                (ConfigurationMode::Mode2, 2),
                (ConfigurationMode::Mode3, 3),
            ] {
                let p = prepare(&c, Command::SelectConfigurationMode { flag, mode });
                assert_eq!(logical(&p), [f, m]);
                assert_eq!(
                    p.decode(Reply::decode(&p.request.encode()).unwrap())
                        .unwrap()
                        .value(),
                    &ReplyValue::NoPayload
                );
            }
            for (option, o) in [(Toggle::Disabled, 0), (Toggle::Enabled, 1)] {
                for byte in 0..=255 {
                    let p = prepare(
                        &c,
                        Command::PackConfigurationOptions {
                            flag,
                            option,
                            upper: byte,
                            lower: 255 - byte,
                        },
                    );
                    assert_eq!(logical(&p), [f, o, byte, 255 - byte]);
                    assert_eq!(
                        p.decode(Reply::decode(&p.request.encode()).unwrap())
                            .unwrap()
                            .value(),
                        &ReplyValue::NoPayload
                    );
                }
            }
        }
    }
}
#[test]
fn raw_configuration_snapshots_preserve_order_and_do_not_infer_validity() {
    for reverse in [false, true] {
        let c = catalogue(reverse);
        let p = prepare(
            &c,
            Command::ReadConfigurationFlags {
                bank: ConfigurationBank::Bank1,
                option: Toggle::Enabled,
            },
        );
        let mut raw = p.request.encode();
        let expected = [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xff];
        for (i, byte) in expected.into_iter().enumerate() {
            raw[1 + usize::from(p.slots[3 + i])] = byte;
        }
        let reply = p.decode(Reply::decode(&raw).unwrap()).unwrap();
        assert_eq!(
            reply.value(),
            &ReplyValue::ConfigurationFlags { raw_data: expected }
        );
        assert_eq!(reply.raw().raw(), &raw);
        let p = prepare(&c, Command::ReadConfigurationOptions);
        assert_eq!(logical(&p), [255; 4]);
        for value in [0, 1, 0x89ab_cdef, u32::MAX] {
            let mut raw = p.request.encode();
            for (i, byte) in value.to_be_bytes().into_iter().enumerate() {
                raw[1 + usize::from(p.slots[i])] = byte;
            }
            assert_eq!(
                p.decode(Reply::decode(&raw).unwrap()).unwrap().value(),
                &ReplyValue::ConfigurationOptions { raw_value: value }
            );
        }
    }
}

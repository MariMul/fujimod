use super::*;
use fujimod_catalog::settings::{
    NamedSetting, SETTINGS_SCHEMA_URI, SettingsBank, SettingsField, SettingsSource,
};

/// An update holding one literal-only method-4 stream, stored inverted.
fn update(section: &[u8], flash_offset: usize, native: usize) -> Vec<u8> {
    let mut body = Vec::new();
    for chunk in section.chunks(127) {
        body.push(chunk.len() as u8);
        body.extend(chunk);
    }
    let packed = body.len() as u32 + 20;
    let mut stream: Vec<u8> = [section.len() as u32, packed, packed.div_ceil(0x4000), 0x4000, 4]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    stream.extend(body);
    let mut raw = vec![0; flash_offset + native];
    raw.extend(stream);
    fujimod_firmware::invert(&raw)
}
fn range(address: u64, bytes: &[u8]) -> UpdateRange {
    UpdateRange {
        address,
        size: bytes.len() as u64,
        sha256: fujimod_firmware::sha256(bytes),
    }
}
fn fixture() -> (Vec<u8>, FirmwareSettings) {
    let descriptor: Vec<u8> = [2u32, 40, 931].into_iter().flat_map(u32::to_le_bytes).collect();
    let table = [0x10, 0, 9, 0, 0xf0, 0, 7, 0, 0xff, 0xff, 0, 0];
    let mut section = descriptor.clone();
    section.extend([0; 4]);
    section.extend(table);
    let layout = FirmwareSettings {
        schema_uri: SETTINGS_SCHEMA_URI.into(),
        schema: 1,
        id: "sample-1-settings".into(),
        firmware_id: "sample-1".into(),
        source: SettingsSource {
            flash_offset: 0x10,
            address: 0x1000,
            size: section.len() as u64,
            sha256: fujimod_firmware::sha256(&section),
        },
        bank: SettingsBank {
            descriptor: range(0x1000, &descriptor),
            kind: 2,
            blocks: 40,
            first_block: 931,
            logical_first: 41,
            bytes: 0x400,
        },
        defaults: Some(range(0x1010, &table)),
        fields: vec![SettingsField {
            offset: 0x202,
            bytes: vec![0x55, 0xaa],
            basis: "Settings signature.".into(),
        }],
        settings: BTreeMap::from([(
            "debug".into(),
            NamedSetting {
                offset: 0xf7,
                values: BTreeMap::from([("off".into(), vec![0]), ("on".into(), vec![1])]),
            },
        )]),
    };
    (update(&section, 4, 0x10), layout)
}

#[test]
fn banks_take_stock_defaults_fields_and_requested_settings() {
    let (source, layout) = fixture();
    let flash = flash_image(&source, 4).unwrap();
    let requested = BTreeMap::from([("debug".into(), "on".into())]);
    let bank = build(&layout, &flash, &requested).unwrap();
    assert_eq!((bank.first_block, bank.blocks, bank.logical_first), (931, 40, 41));
    assert_eq!(bank.bytes.len(), 0x400);
    assert_eq!((bank.bytes[0x10], bank.bytes[0xf0]), (9, 7));
    assert_eq!(&bank.bytes[0x202..0x204], [0x55, 0xaa]);
    assert_eq!(value(&layout, &bank.bytes, "debug").as_deref(), Some("on"));
}

#[test]
fn unknown_settings_and_changed_updates_refuse() {
    let (source, layout) = fixture();
    let flash = flash_image(&source, 4).unwrap();
    let unknown = BTreeMap::from([("language".into(), "en".into())]);
    assert!(build(&layout, &flash, &unknown).unwrap_err().contains("language"));
    let value = BTreeMap::from([("debug".into(), "maybe".into())]);
    assert!(build(&layout, &flash, &value).unwrap_err().contains("maybe"));
    let mut changed = flash.clone();
    *changed.last_mut().unwrap() ^= 1;
    assert!(build(&layout, &changed, &BTreeMap::new()).is_err());
}

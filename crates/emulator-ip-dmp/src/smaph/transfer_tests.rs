use super::*;
use fujimod_emulator_devices::test_host::recording::RecordingHost;
#[test]
fn native_pixmap_copy_preserves_words_flips_rows_and_keeps_pitch_padding() {
    let mut host = RecordingHost {
        ram_base: 0x1000,
        ram: vec![0xa5; 0x4000],
        ..Default::default()
    };
    for y in 0..16 {
        for x in 0..32 {
            let i = pixel_index(x, y, 32, true) * 4;
            host.ram[i..i + 4].copy_from_slice(&[255, x as u8, y as u8, 31]);
        }
    }
    let mut state = State::default();
    state.regs.extend([
        (0xc00, 0x1000),
        (0xc04, 0x3000),
        (0xc08, 0x00100020),
        (0xc0c, 144),
        (0xc10, 1),
    ]);
    let job = state.copy_job(&Cx::new(&mut host, 0, 0, 0), 1).unwrap();
    assert_eq!(job.writes.len(), 1);
    let (address, bytes) = &job.writes[0];
    assert_eq!(*address, 0x3000);
    assert_eq!(bytes.len(), 144 * 16);
    for y in 0..16 {
        for x in 0..32 {
            assert_eq!(
                &bytes[y * 144 + x * 4..y * 144 + x * 4 + 4],
                &[255, x as u8, (15 - y) as u8, 31]
            );
        }
        assert_eq!(&bytes[y * 144 + 128..(y + 1) * 144], &[0xa5; 16]);
    }
    assert!(host.ram[0x2000..].iter().all(|b| *b == 0xa5));
}

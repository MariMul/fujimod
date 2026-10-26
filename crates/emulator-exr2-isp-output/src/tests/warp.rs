use crate::*;
use fujimod_emulator_devices::Device;
use fujimod_emulator_devices::exr2::irq_routes::Irq;
use fujimod_emulator_devices::test_host::*;
#[test]
fn warp_memory_dma_keeps_alignment_rows_and_acknowledges_status() {
    let mut h = TestHost::new();
    h.ram[0x8000..0x8010]
        .copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 11, 12, 13, 14, 15, 16, 17, 18]);
    h.ram[0x9000..0x9040].fill(0xa5);
    let mut d = Warp::new(
        fujimod_emulator_imaging::exr2_isp::options::warp::Options {
            completion_ticks: 5,
            stop_ticks: 2,
        },
        (Irq { core: 0, line: 74 }, Default::default()),
    )
    .unwrap();
    for (o, v) in [
        (0x10, 0x8000),
        (0x14, 4),
        (0x18, 2 << 16 | 4),
        (0x1c, 0x9000),
        (0x20, 8),
        (0x28, 4 << 16 | 6),
        (0x40, 4096),
        (0x44, 4096),
        (0x4c, 0x10001),
        (0x50, 1 << 16 | 3),
        (0x54, 1 << 16 | 3),
        (0x148, 0x21),
        (0, 1),
    ] {
        write(&mut d, &mut h, 0xfff65000 + o, v);
    }
    advance(&mut d, &mut h, 4);
    assert_eq!(h.ram[0x9000], 0xa5);
    let identity = d.fingerprint().unwrap();
    let diagnostic = d.diagnostic_state();
    assert!(diagnostic[1]["pixels"].is_object());
    assert_eq!(
        d.diagnostic_buffer("dma-output").unwrap(),
        &[1, 2, 3, 4, 5, 6, 7, 8, 11, 12, 13, 14, 15, 16, 17, 18]
    );
    assert!(d.diagnostic_buffer("missing").is_err());
    assert_eq!(d.fingerprint().unwrap(), identity);
    advance_from_checkpoint(&mut d, &mut h, 5);
    assert_eq!(&h.ram[0x9000..0x9008], &h.ram[0x8000..0x8008]);
    assert_eq!(&h.ram[0x9010..0x9018], &h.ram[0x8008..0x8010]);
    assert!(h.ram[0x9020..0x9040].iter().all(|&v| v == 0xa5));
    assert_eq!(h.irqs, [(0, 74)]);
    assert_eq!(read(&mut d, &mut h, 0xfff65144), 0x20);
    write(&mut d, &mut h, 0xfff65144, 0x20);
    assert_eq!(read(&mut d, &mut h, 0xfff65144), 0);
    write(&mut d, &mut h, 0xfff65000, 0);
    advance(&mut d, &mut h, 6);
    assert_eq!(read(&mut d, &mut h, 0xfff65000), 0);
    advance(&mut d, &mut h, 7);
    assert_eq!(read(&mut d, &mut h, 0xfff65000), 0x10000);
}

#[test]
fn worker_warp_retains_deadlines_padding_status_and_checkpoint_bytes() {
    fujimod_emulator_devices::work::configure(2).unwrap();
    warp_memory_dma_keeps_alignment_rows_and_acknowledges_status();
    fujimod_emulator_devices::work::configure(0).unwrap();
}

#[test]
fn warp_bounds_a_logical_source_view_that_crosses_its_row_pitch() {
    let mut h = TestHost::new();
    for (i, byte) in h.ram[0x8000..0x8020].iter_mut().enumerate() {
        *byte = i as u8;
    }
    h.ram[0x9000..0x9020].fill(0xa5);
    let mut d = Warp::new(
        fujimod_emulator_imaging::exr2_isp::options::warp::Options {
            completion_ticks: 5,
            stop_ticks: 2,
        },
        (Irq { core: 0, line: 74 }, Default::default()),
    )
    .unwrap();
    for (o, v) in [
        (0x10, 0x8000),
        (0x14, 4),
        (0x18, 2 << 16 | 6),
        (0x1c, 0x9000),
        (0x20, 4),
        (0x28, 2 << 16 | 4),
        (0x40, 4096),
        (0x44, 4096),
        (0x4c, 0x10001),
        (0x50, 1 << 16 | 4),
        (0x54, 1 << 16 | 3),
        (0x148, 0x21),
        (0, 1),
    ] {
        write(&mut d, &mut h, 0xfff65000 + o, v);
    }
    advance(&mut d, &mut h, 5);
    assert_eq!(&h.ram[0x9000..0x9008], &[0, 1, 2, 3, 4, 8, 6, 7]);
    assert_eq!(&h.ram[0x9008..0x9010], &[8, 9, 10, 11, 12, 16, 14, 15]);
    assert!(h.ram[0x9010..0x9020].iter().all(|&byte| byte == 0xa5));
}

#[test]
fn warp_rejects_a_selected_source_endpoint_past_physical_dram() {
    assert_eq!(
        fujimod_emulator_image_processing::resample::Endpoint::source_bytes(
            6,
            2,
            8,
            4,
            2,
            [4, 1],
            0,
        )
        .unwrap(),
        20
    );
    assert!(fujimod_emulator_imaging::exr2_isp::dma::physical(0x3ffffff0, 20).is_err());
}

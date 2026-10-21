use super::super::*;
use super::input::cfa;
use super::support::*;
use crate::Device;
#[test]
fn still_isp_reconstructs_addressed_pixels_and_refuses_unequal_black() {
    let mut h = TestHost::new();
    let mut d = StillProcessor::new(
        still::Options {
            completion_ticks: 4,
            cfa: cfa(),
        },
        Irq { core: 0, line: 61 },
    )
    .unwrap();
    for (o, v) in [
        (0x38, 6 << 16 | 6),
        (0x44, 6),
        (0x48, 8),
        (0x50, 0x8000),
        (0x58, 0x9000),
        (0x64, 5 << 16 | 5),
    ] {
        write(&mut d, &mut h, 0xfff40000 + o, v);
    }
    write(&mut d, &mut h, 0xfff38054, 0x1000100);
    write(&mut d, &mut h, 0xfff38058, 0x1000100);
    write(&mut d, &mut h, 0xfff40004, 1);
    advance(&mut d, &mut h, 3);
    assert!(h.effects.is_empty());
    advance_from_checkpoint(&mut d, &mut h, 4);
    assert_eq!(&h.ram[0x9000..0x9004], &[0, 0, 128, 128]);
    assert_eq!(h.irqs, [(0, 61)]);
    let first = d.checkpoint().unwrap();
    h.ram[0x8000..0x8048].fill(32);
    write(&mut d, &mut h, 0xfff40004, 1);
    let second = d.checkpoint().unwrap();
    assert!(!std::sync::Arc::ptr_eq(
        &first.blobs()[0],
        &second.blobs()[0]
    ));
    assert_eq!(&*first.blobs()[0], &[0; 72]);
    assert_eq!(&*second.blobs()[0], &[32; 72]);
    advance(&mut d, &mut h, 8);
    d.restore_checkpoint(&first).unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &first.blobs()[0],
        &d.checkpoint().unwrap().blobs()[0]
    ));
    write(&mut d, &mut h, 0xfff38004, 1);
    let mut cx = h.cx();
    d.write(&mut cx, 0xfff40004, 4, 1);
    assert!(cx.refusal().is_some());
}

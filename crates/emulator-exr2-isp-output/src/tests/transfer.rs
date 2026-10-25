use crate::transfer::ImageTransfer;
use fujimod_emulator_devices::{Device, exr2::irq_routes::Irq, test_host::*};
#[test]
fn image_transfer_reset_is_clocked_and_unknown_execution_refuses() {
    let mut d = ImageTransfer::new(Irq { core: 0, line: 73 });
    let mut h = TestHost::new();
    write(&mut d, &mut h, 0xfff64000, 0);
    advance(&mut d, &mut h, 1);
    assert_eq!(read(&mut d, &mut h, 0xfff64000), 0);
    advance_from_checkpoint(&mut d, &mut h, 2);
    assert_eq!(read(&mut d, &mut h, 0xfff64000), 0x10000);
    for (o, v) in [
        (0, 0x10030),
        (0xc, 1920),
        (0x10, 1088 << 16 | 1920),
        (0x18, 0),
        (0x1c, 0x31),
        (4, 0x072fb400),
        (8, 0x02000000),
    ] {
        write(&mut d, &mut h, 0xfff64000 + o, v);
    }
    let ram = h.ram.clone();
    let mut cx = h.cx();
    d.write(&mut cx, 0xfff64000, 4, 0x10131);
    assert!(cx.refusal().unwrap().contains("layout is unsupported"));
    drop(cx);
    assert_eq!(h.ram, ram);
    assert!(h.irqs.is_empty());
    assert_eq!(d.save()["registers"][1], 0x072fb400);
    assert_eq!(d.save()["registers"][2], 0x02000000);
}

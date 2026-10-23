use crate::combination;
use fujimod_emulator_devices::Device;
use fujimod_emulator_devices::exr2::isp::{Irq, dma};

#[test]
fn combination_fingerprint_keeps_reset_timing_before_any_writes() {
    let combination = |delay| {
        combination::Combination::new(
            combination::Options {
                reset_ticks: delay,
                clock: dma::Clock::Fixed { ticks: 10 },
                arithmetic: combination::Arithmetic::Unmodeled,
            },
            Irq { core: 0, line: 68 },
        )
        .unwrap()
    };
    let (a, b) = (combination(2), combination(3));
    assert_eq!(a.save(), b.save());
    assert_ne!(a.fingerprint().unwrap(), b.fingerprint().unwrap());
}

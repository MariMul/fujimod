use super::*;
use crate::bus::InterruptLine;
use crate::exr2::pulse::{Options as PulseOptions, PulseUnit, Wiring as PulseWiring};
use crate::parts::pins::{Pin, Pins};
use crate::parts::spi::{Endpoint, Shift};
use crate::test_host::TestHost;

struct TestPins;

impl Pins for TestPins {
    fn output_level(&self, _: Pin) -> Result<Option<bool>, String> {
        Ok(Some(false))
    }

    fn input(&mut self, _: &mut Cx<'_>, _: Pin, _: bool) -> Result<(), String> {
        Ok(())
    }
}

fn motor() -> MotorOptions {
    MotorOptions {
        initial_position: 0,
        travel_limits: [-100, 100],
        boundary: TravelBoundary::Refuse,
        ticks_per_step: 1_000,
        settle_ticks: 4_000,
        home: None,
        counters: Vec::new(),
    }
}

fn unit() -> LensUnit {
    let pulse = PulseUnit::new(
        PulseOptions {
            counter_channels: Vec::new(),
            timer_channels: Vec::new(),
            quadrature: false,
            initial_phase: 0,
            capture_channel: None,
        },
        PulseWiring { irqs: [None; 12] },
    )
    .unwrap();
    LensUnit::new(
        Options {
            protocol: Protocol::Banked {
                positive_direction_high: true,
                stop_response: StopResponse::Silent,
                clock: [BankedClock::Fixed; 2],
            },
            motors: [motor(), motor()],
            encoder: None,
            zoom: None,
        },
        Wiring {
            pulses: pulse.pins(),
            gpio: Box::new(TestPins),
            completion: [
                InterruptLine::Core { core: 0, line: 14 },
                InterruptLine::Core { core: 0, line: 12 },
            ],
            position: None,
        },
    )
    .unwrap()
}

fn transmit(unit: &mut LensUnit, host: &mut TestHost, word: u16, at: u64) {
    host.now = at;
    let mut cx = host.cx();
    let reply = unit
        .transfer(
            &mut cx,
            Shift {
                word,
                bits: 16,
                receive: false,
            },
            at,
        )
        .unwrap();
    assert_eq!(reply, 0);
    assert_eq!(cx.refusal(), None);
}

#[test]
fn zero_quantity_completes_at_frame_end_before_a_later_focus_move() {
    let mut unit = unit();
    let mut host = TestHost::new();

    transmit(&mut unit, &mut host, 0x3e40, 0);
    assert_eq!(unit.position(0), Some(0));
    assert_eq!(unit.next_deadline(0), None);
    assert_eq!(host.irqs, [(0, 14)]);

    transmit(&mut unit, &mut host, 0x04e0, 1_691);
    transmit(&mut unit, &mut host, 0x0800, 1_691);
    transmit(&mut unit, &mut host, 0x3e80, 1_691);
    assert_eq!(unit.position(0), Some(0));
    assert_eq!(unit.next_deadline(1_691), Some(2_691));
    assert_eq!(host.irqs, [(0, 14)]);
}

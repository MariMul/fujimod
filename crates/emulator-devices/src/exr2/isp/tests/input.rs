use super::super::*;
use super::support::*;
use crate::Device;
pub(super) fn cfa() -> Vec<u8> {
    (0..36).map(|i| (i % 3) as u8).collect()
}
fn input_options() -> input::Options {
    input::Options {
        reset_ticks: 2,
        command_ticks: 3,
        phase_ticks: 2,
        command: Some(input::Command::PackedWords),
        command_controls: vec![0],
        phase_ack: input::PhaseAck::Enabled,
        clock: None,
        sensor_period_ticks: None,
        raw_dma_ticks: 5,
        readback_ticks: 7,
        cfa: Some(cfa()),
    }
}
fn input_wiring() -> input::Wiring {
    input::Wiring {
        frame_period: None,
        statistics_grid: None,

        phase_irqs: [Irq { core: 0, line: 39 }, Irq { core: 1, line: 41 }],
        raw_irq: Irq { core: 0, line: 36 },
        readback_irq: Irq { core: 0, line: 38 },
        shutter_output: None,
        sensor: None,
        parameters: None,
        memory_outputs: None,
        edges: Default::default(),
        start_tick: 0,
    }
}
#[test]
fn input_reset_phase_and_arbitrary_relocated_command() {
    let mut h = TestHost::new();
    let mut d = ImageInput::new(input_options(), input_wiring()).unwrap();
    write(&mut d, &mut h, 0xfff10000, 0);
    advance(&mut d, &mut h, 1);
    assert_eq!(read(&mut d, &mut h, 0xfff10000), 0);
    advance(&mut d, &mut h, 2);
    assert_eq!(read(&mut d, &mut h, 0xfff10000), 1);
    for (o, v) in [(0x13c, 1), (0xc4, 3), (0xc4, 2)] {
        write(&mut d, &mut h, 0xfff10000 + o, v);
    }
    advance(&mut d, &mut h, 3);
    assert_eq!(read(&mut d, &mut h, 0xfff100c4), 2);
    advance(&mut d, &mut h, 4);
    assert_eq!(read(&mut d, &mut h, 0xfff100c4), 3);
    // Three-byte words have five ignored gap bytes, unlike opcode81 writes.
    h.ram[0x8a00..0x8a08].copy_from_slice(&[0xe7, 0x12, 0x34, 0xa5, 1, 2, 3, 4]);
    for (o, v) in [
        (0x170, 0x101000),
        (0x174, 0x8a00),
        (0x15c, 7),
        (0x9c, 2),
        (0x154, 0x10001),
        (0x94, 1),
    ] {
        write(&mut d, &mut h, 0xfff10000 + o, v);
    }
    h.ram[0x8a00] = 0;
    advance(&mut d, &mut h, 6);
    assert_eq!(read(&mut d, &mut h, 0xfff10094), 1);
    advance(&mut d, &mut h, 7);
    assert_eq!(read(&mut d, &mut h, 0xfff10094), 0);
    assert!(h.irqs.is_empty());
}

#[test]
fn clearing_command_start_stops_the_transmitter_and_allows_rearm() {
    let mut h = TestHost::new();
    let mut d = ImageInput::new(input_options(), input_wiring()).unwrap();
    h.ram[0x8a00..0x8a08].copy_from_slice(&[0xe7, 0x12, 0x34, 0, 0, 0, 0, 0]);
    for (o, v) in [
        (0x170, 0x101000),
        (0x174, 0x8a00),
        (0x15c, 7),
        (0x9c, 2),
        (0x154, 0x10001),
        (0x94, 1),
    ] {
        write(&mut d, &mut h, 0xfff10000 + o, v);
    }
    assert_eq!(d.next_deadline(0), Some(3));
    advance(&mut d, &mut h, 1);
    write(&mut d, &mut h, 0xfff10094, 0);
    assert_eq!(read(&mut d, &mut h, 0xfff10094), 0);
    assert_eq!(d.next_deadline(1), None);
    // The stopped channel disarms and reports its idle DMA status.
    write(&mut d, &mut h, 0xfff10154, 0);
    assert_eq!(read(&mut d, &mut h, 0xfff10160), 0);
    for (o, v) in [(0x154, 0x10001), (0x94, 1)] {
        write(&mut d, &mut h, 0xfff10000 + o, v);
    }
    assert_eq!(d.next_deadline(1), Some(4));
    advance(&mut d, &mut h, 4);
    assert_eq!(read(&mut d, &mut h, 0xfff10094), 0);
    assert!(h.irqs.is_empty());
}

#[test]
fn packed_opcode_writes_still_refuse_nonzero_padding() {
    let mut h = TestHost::new();
    let mut options = input_options();
    options.command = Some(input::Command::PackedWrites);
    let mut d = ImageInput::new(options, input_wiring()).unwrap();
    h.ram[0x8a00..0x8a08].copy_from_slice(&[0x81, 0x12, 0x34, 0xa5, 0, 0, 0, 0]);
    for (o, v) in [(0x170, 0x101000), (0x174, 0x8a00), (0x15c, 7), (0x9c, 2)] {
        write(&mut d, &mut h, 0xfff10000 + o, v);
    }
    let mut cx = h.cx();
    d.write(&mut cx, 0xfff10154, 4, 0x10001);
    assert_eq!(
        cx.refusal(),
        Some("input command opcode/padding is unsupported")
    );
    assert_eq!(d.next_deadline(0), None);
    assert!(h.irqs.is_empty());
}
#[test]
fn phase_clock_coalesces_irqs_but_preserves_physical_edges() {
    let mut h = TestHost::new();
    let mut o = input_options();
    o.clock = Some(input::Clock {
        mode: input::ClockMode::InputEnable,
        source: input::ClockSource::Periodic {
            period_ticks: 10,
            first_delay_ticks: None,
        },
        secondary: true,
        companion_secondary: true,
    });
    let w = input_wiring();
    let edges = w.edges.clone();
    let mut d = ImageInput::new(o, w).unwrap();
    assert_eq!(d.next_deadline(0), None);
    for (o, v) in [(0x48, 1), (0x38c, 0x10003)] {
        write(&mut d, &mut h, 0xfff10000 + o, v);
    }
    assert_eq!(d.next_deadline(0), Some(10));
    advance(&mut d, &mut h, 9);
    assert!(h.irqs.is_empty());
    advance(&mut d, &mut h, 10);
    assert_eq!(h.irqs, [(0, 39), (1, 41)]);
    assert_eq!(edges.lock().unwrap()[0].unwrap().tick, 10);
    advance(&mut d, &mut h, 30);
    assert_eq!(h.irqs.len(), 2);
    assert_eq!(
        edges.lock().unwrap()[0],
        Some(crate::scene::FrameEdge { index: 3, tick: 30 })
    );
    write(&mut d, &mut h, 0xfff10388, 1);
    advance(&mut d, &mut h, 40);
    assert_eq!(h.irqs.len(), 3);
    assert_eq!(edges.lock().unwrap()[0].unwrap().index, 4);
    write(&mut d, &mut h, 0xfff10048, 0);
    assert_eq!(d.next_deadline(40), None);
}
#[test]
fn another_source_joins_the_scan_edge_and_command_dma_does_not_pause_it() {
    let mut o = input_options();
    o.command_ticks = 20;
    o.clock = Some(input::Clock {
        mode: input::ClockMode::InputEnable,
        source: input::ClockSource::Periodic {
            period_ticks: 10,
            first_delay_ticks: None,
        },
        secondary: true,
        companion_secondary: true,
    });
    let w = input_wiring();
    let edges = w.edges.clone();
    let mut d = ImageInput::new(o, w).unwrap();
    let mut h = TestHost::new();
    for (o, v) in [(0x48, 1), (0x38c, 0x10201)] {
        write(&mut d, &mut h, 0xfff10000 + o, v);
    }
    advance(&mut d, &mut h, 5);
    write(&mut d, &mut h, 0xfff1038c, 0x10203);
    h.ram[0x8a00..0x8a08].copy_from_slice(&[0xe7, 0x12, 0x34, 0, 0, 0, 0, 0]);
    for (o, v) in [
        (0x170, 0x101000),
        (0x174, 0x8a00),
        (0x15c, 7),
        (0x9c, 2),
        (0x154, 0x10001),
        (0x94, 1),
    ] {
        write(&mut d, &mut h, 0xfff10000 + o, v);
    }
    assert_eq!(d.next_deadline(5), Some(10));
    advance(&mut d, &mut h, 10);
    assert_eq!(h.irqs, [(0, 39), (1, 41)]);
    assert!(edges.lock().unwrap().iter().all(|e| e.unwrap().tick == 10));
    assert_eq!(read(&mut d, &mut h, 0xfff10094), 1);
    write(&mut d, &mut h, 0xfff10388, 0);
    advance(&mut d, &mut h, 20);
    assert_eq!(edges.lock().unwrap()[1].unwrap().tick, 20);
    advance(&mut d, &mut h, 25);
    assert_eq!(read(&mut d, &mut h, 0xfff10094), 0);
}
#[test]
fn physical_sensor_latch_and_both_input_sources_share_one_edge() {
    use crate::scene::{Frame, FramePixels, SensorFrames};
    use std::sync::{Arc, Mutex};
    let frames = Arc::new(Mutex::new(SensorFrames::new(
        Frame::new(2, 1, FramePixels::Raw16(vec![100, 200].into())).unwrap(),
    )));
    let mut w = input_wiring();
    w.frame_period = Some(Arc::new(Mutex::new(10)));
    w.sensor = Some(
        crate::parts::sensor::Sensor::new(
            crate::parts::sensor::Options {
                width: 2,
                height: 1,
                image_area: [0, 0, 2, 1],
                cfa: cfa(),
                black: 0,
                full_scale: 4095,
                reference_white: [256; 3],
            },
            frames.clone(),
        )
        .unwrap(),
    );
    let edges = w.edges.clone();
    let mut o = input_options();
    o.sensor_period_ticks = Some(100);
    o.clock = Some(input::Clock {
        mode: input::ClockMode::InputEnable,
        source: input::ClockSource::Sensor {},
        secondary: true,
        companion_secondary: true,
    });
    let mut d = ImageInput::new(o, w).unwrap();
    let mut h = TestHost::new();
    for (o, v) in [(0x48, 1), (0x38c, 0x10201)] {
        write(&mut d, &mut h, 0xfff10000 + o, v);
    }
    advance(&mut d, &mut h, 5);
    write(&mut d, &mut h, 0xfff1038c, 0x10203);
    assert_eq!(d.next_deadline(5), Some(10));
    assert_eq!(frames.lock().unwrap().edge, None);
    advance(&mut d, &mut h, 10);
    let edge = frames.lock().unwrap().edge.unwrap();
    assert_eq!(*edges.lock().unwrap(), [Some(edge), Some(edge)]);
    write(&mut d, &mut h, 0xfff10388, 0);
    advance(&mut d, &mut h, 20);
    assert_eq!(*edges.lock().unwrap(), [frames.lock().unwrap().edge; 2]);
    assert!(frames.lock().unwrap().edge.unwrap().index > edge.index);
}
#[test]
fn register_sources_can_explicitly_follow_a_synthetic_sensor_without_input_enable() {
    use crate::scene::{Frame, FramePixels, SensorFrames};
    use std::sync::{Arc, Mutex};
    let frames = Arc::new(Mutex::new(SensorFrames::new(
        Frame::new(2, 1, FramePixels::Raw16(vec![100, 200])).unwrap(),
    )));
    let mut w = input_wiring();
    w.sensor = Some(
        crate::parts::sensor::Sensor::new(
            crate::parts::sensor::Options {
                width: 2,
                height: 1,
                image_area: [0, 0, 2, 1],
                cfa: cfa(),
                black: 0,
                full_scale: 4095,
                reference_white: [256; 3],
            },
            frames.clone(),
        )
        .unwrap(),
    );
    let edges = w.edges.clone();
    let mut o = input_options();
    o.sensor_period_ticks = Some(10);
    o.clock = Some(input::Clock {
        mode: input::ClockMode::RegisterPeriods,
        source: input::ClockSource::Sensor {},
        secondary: true,
        companion_secondary: false,
    });
    let mut d = ImageInput::new(o, w).unwrap();
    let mut h = TestHost::new();
    write(&mut d, &mut h, 0xfff10000, 0);
    advance(&mut d, &mut h, 2);
    write(&mut d, &mut h, 0xfff10024, 4);
    write(&mut d, &mut h, 0xfff1038c, 0x203);
    assert_eq!(read(&mut d, &mut h, 0xfff10048), 0);
    assert_eq!(d.save()[0]["phase_due"], serde_json::json!([null, null]));
    assert_eq!(d.next_deadline(2), Some(10));
    advance(&mut d, &mut h, 9);
    assert_eq!(*edges.lock().unwrap(), [None; 2]);
    advance(&mut d, &mut h, 10);
    let first = frames.lock().unwrap().edge.unwrap();
    assert_eq!(*edges.lock().unwrap(), [Some(first); 2]);
    assert_eq!(read(&mut d, &mut h, 0xfff10384), 0x203);
    write(&mut d, &mut h, 0xfff10388, 0);
    assert_eq!(read(&mut d, &mut h, 0xfff10384), 1);
    advance(&mut d, &mut h, 20);
    assert_eq!(edges.lock().unwrap()[0], frames.lock().unwrap().edge);
    assert_eq!(edges.lock().unwrap()[1], frames.lock().unwrap().edge);
    assert_eq!(h.irqs, [(0, 39), (1, 41), (1, 41)]);
    // A scalar write and late enable do not restart the physical source.
    write(&mut d, &mut h, 0xfff10024, 4096);
    write(&mut d, &mut h, 0xfff1038c, 1);
    write(&mut d, &mut h, 0xfff10388, 0);
    advance(&mut d, &mut h, 30);
    advance(&mut d, &mut h, 35);
    write(&mut d, &mut h, 0xfff1038c, 0x203);
    assert_eq!(d.next_deadline(35), Some(40));
    advance(&mut d, &mut h, 40);
    assert_eq!(edges.lock().unwrap()[1].unwrap().index, 4);
    assert_eq!(read(&mut d, &mut h, 0xfff10384), 0x203);
    write(&mut d, &mut h, 0xfff10388, 0);
    let saved = d.save();
    assert_eq!(d.next_deadline(40), Some(50));
    assert_eq!(d.save(), saved);
    advance(&mut d, &mut h, 50);
    let continued = d.save();
    d.restore(&saved).unwrap();
    advance(&mut d, &mut h, 50);
    assert_eq!(d.save(), continued);
    assert!(h.effects.iter().all(|e| *e == "irq"));
}
#[test]
fn explicit_sensor_clock_refuses_missing_sources_and_unused_periodic_fields() {
    let mut options = input_options();
    options.clock = Some(input::Clock {
        mode: input::ClockMode::RegisterPeriods,
        source: input::ClockSource::Sensor {},
        secondary: true,
        companion_secondary: false,
    });
    assert!(ImageInput::new(options, input_wiring()).is_err());
    assert!(
        serde_json::from_value::<input::Clock>(serde_json::json!({
            "mode":"register_periods", "source":{"kind":"sensor","period_ticks":4},
            "secondary":true, "companion_secondary":false,
        }))
        .is_err()
    );
}
#[test]
fn raw_sensor_freezes_frame_packs12_preserves_padding_and_waits_for_ack() {
    use crate::scene::{Frame, FramePixels, SensorFrames};
    let frames = std::sync::Arc::new(std::sync::Mutex::new(SensorFrames::new(
        Frame::new(2, 1, FramePixels::Raw16(vec![0x123, 0xabc])).unwrap(),
    )));
    let sensor = crate::parts::sensor::Sensor::new(
        crate::parts::sensor::Options {
            width: 2,
            height: 1,
            image_area: [0, 0, 2, 1],
            cfa: cfa(),
            black: 0,
            full_scale: 4095,
            reference_white: [256; 3],
        },
        frames.clone(),
    )
    .unwrap();
    let mut o = input_options();
    o.sensor_period_ticks = Some(10);
    let mut w = input_wiring();
    w.sensor = Some(sensor);
    let mut d = ImageInput::new(o, w).unwrap();
    let mut h = TestHost::new();
    h.ram[0x8000..0x8020].fill(0xa5);
    for (o, v) in [
        (0x14, 1),
        (0x18, 0),
        (0x24, 2),
        (4, 0x10001),
        (0x354, 0x10300000),
        (0x358, 0x8000),
        // X-M1's whole descriptor setter leaves upper stack bits intact.
        // They must not alter the selected bit-0 transport or its pixels.
        (0x35c, 0x00662f91),
        (0x360, 16),
    ] {
        write(&mut d, &mut h, 0xfff10000 + o, v);
    }
    let saved = d.save();
    d.restore(&saved).unwrap();
    assert_eq!(read(&mut d, &mut h, 0xfff1035c), 1);
    advance(&mut d, &mut h, 10);
    frames
        .lock()
        .unwrap()
        .set_frame(Frame::new(2, 1, FramePixels::Raw16(vec![0, 0])).unwrap());
    advance(&mut d, &mut h, 14);
    assert_eq!(h.ram[0x8000], 0xa5);
    advance(&mut d, &mut h, 15);
    assert_eq!(&h.ram[0x8000..0x8004], &[0x23, 0xc1, 0xab, 0xa5]);
    assert_eq!(h.effects, ["dma", "irq"]);
    advance(&mut d, &mut h, 20);
    assert_eq!(d.next_deadline(20), Some(30));
    write(&mut d, &mut h, 0xfff10350, 1);
    advance(&mut d, &mut h, 30);
    assert_eq!(d.next_deadline(30), Some(35));
    advance(&mut d, &mut h, 35);
    assert_eq!(&h.ram[0x8000..0x8003], &[0, 0, 0]);
}
fn raw_window_input(packed: bool) -> (ImageInput, TestHost, crate::scene::SensorFramesHandle) {
    use crate::scene::{Frame, FramePixels, SensorFrames};
    let frames = std::sync::Arc::new(std::sync::Mutex::new(SensorFrames::new(
        Frame::new(
            8,
            4,
            FramePixels::Raw16((0..32).map(|i| (i / 8 * 256 + i % 8 * 16) as u16).collect()),
        )
        .unwrap(),
    )));
    let mut w = input_wiring();
    w.sensor = Some(
        crate::parts::sensor::Sensor::new(
            crate::parts::sensor::Options {
                width: 8,
                height: 4,
                image_area: [0, 0, 8, 4],
                cfa: cfa(),
                black: 0,
                full_scale: 4095,
                reference_white: [256; 3],
            },
            frames.clone(),
        )
        .unwrap(),
    );
    let mut o = input_options();
    o.sensor_period_ticks = Some(10);
    let mut d = ImageInput::new(o, w).unwrap();
    let mut h = TestHost::new();
    h.ram[0x7ff0..0x8400].fill(0xa5);
    for (offset, value) in [
        (0x14, 0x10003),
        (0x18, 0x10001),
        // Active columns are independent of the processing window below.
        (0x24, 4),
        (0x28, 1),
        (0x2c, 0),
        (4, 1 | u32::from(packed) << 16),
        (0x354, 0x10300000),
        (0x358, 0x8000),
        (0x35c, 1),
        (0x360, 16),
    ] {
        write(&mut d, &mut h, 0xfff10000 + offset, value);
    }
    (d, h, frames)
}
#[test]
fn raw_dma_uses_the_programmed_window_and_freezes_its_span_until_ack() {
    use crate::scene::{Frame, FramePixels};
    for packed in [false, true] {
        let (mut d, mut h, frames) = raw_window_input(packed);
        let rows = if packed {
            [
                vec![0x10, 0x01, 0x12, 0x30, 0x01, 0x14],
                vec![0x10, 0x02, 0x22, 0x30, 0x02, 0x24],
            ]
        } else {
            [
                vec![0x10, 1, 0x20, 1, 0x30, 1, 0x40, 1],
                vec![0x10, 2, 0x20, 2, 0x30, 2, 0x40, 2],
            ]
        };
        let pitch = if packed { 400 } else { 144 };
        advance(&mut d, &mut h, 10);
        // Neither a later image nor reprogramming can resize an armed DMA.
        frames
            .lock()
            .unwrap()
            .set_frame(Frame::new(1, 1, FramePixels::Raw16(vec![0])).unwrap());
        write(&mut d, &mut h, 0xfff10014, 1);
        write(&mut d, &mut h, 0xfff10018, 0);
        write(&mut d, &mut h, 0xfff10024, 2);
        let checkpoint = d.checkpoint().unwrap();
        assert!(h.effects.is_empty());
        assert!(h.ram[0x7ff0..0x8400].iter().all(|b| *b == 0xa5));
        let saved = d.save();
        assert_eq!(
            saved[0]["dma"]["raster"],
            serde_json::json!({
                "address":0x8000, "width":4, "height":2, "pitch":pitch,
            })
        );
        advance(&mut d, &mut h, 14);
        assert!(h.ram[0x7ff0..0x8400].iter().all(|b| *b == 0xa5));
        advance(&mut d, &mut h, 15);
        let first = h.ram[0x7ff0..0x8400].to_vec();
        for (y, row) in rows.iter().enumerate() {
            let begin = 0x8000 + y * pitch;
            assert_eq!(&h.ram[begin..begin + row.len()], row);
            assert!(
                h.ram[begin + row.len()..begin + pitch]
                    .iter()
                    .all(|b| *b == 0xa5)
            );
        }
        assert_eq!(h.effects, ["dma", "dma", "irq"]);
        assert_eq!(h.irqs, [(0, 36)]);
        let continued = d.save();
        d.restore(&saved).unwrap();
        h.ram[0x7ff0..0x8400].fill(0xa5);
        advance(&mut d, &mut h, 15);
        assert_eq!(d.save(), continued);
        assert_eq!(&h.ram[0x7ff0..0x8400], first);
        d.restore_checkpoint(&checkpoint).unwrap();
        h.ram[0x7ff0..0x8400].fill(0xa5);
        advance(&mut d, &mut h, 15);
        assert_eq!(d.save(), continued);
        assert_eq!(&h.ram[0x7ff0..0x8400], first);
        advance(&mut d, &mut h, 20);
        assert_eq!(d.next_deadline(20), Some(30));
        write(&mut d, &mut h, 0xfff10350, 1);
        advance(&mut d, &mut h, 30);
        assert_eq!(d.save()[0]["dma"]["raster"]["width"], 2);
        assert_eq!(d.save()[0]["dma"]["raster"]["height"], 1);
        advance(&mut d, &mut h, 35);
        assert_eq!(
            &h.ram[0x8000..0x8000 + if packed { 3 } else { 4 }],
            vec![0; if packed { 3 } else { 4 }]
        );
    }
}
#[test]
fn worker_raw_readout_retains_latched_frames_padding_deadline_ack_and_restore() {
    crate::work::configure(2).unwrap();
    raw_sensor_freezes_frame_packs12_preserves_padding_and_waits_for_ack();
    raw_dma_uses_the_programmed_window_and_freezes_its_span_until_ack();
    raw_dma_refuses_out_of_raster_windows_and_unmodeled_transport_selections();
    crate::work::configure(0).unwrap();
}
#[test]
fn raw_dma_refuses_out_of_raster_windows_and_unmodeled_transport_selections() {
    for (offset, value, reason) in [
        (0x24, 3, "bounded input window"), // Odd active width cannot form packed pairs.
        (0x14, 0x80003, "bounded input window"),
        (0x14, 1, "bounded input window"),
        (0x18, 0x40000, "bounded input window"),
        (0x18, 4, "bounded input window"),
        (0x354, 0x10200000, "transport"),
        (0x354, 0x10300001, "transport"),
        (0x35c, 0, "transport"),
        (4, 0x10003, "transport"),
    ] {
        let (mut d, mut h, _) = raw_window_input(true);
        write(&mut d, &mut h, 0xfff10000 + offset, value);
        h.now = 10;
        let mut cx = h.cx();
        d.advance(&mut cx);
        assert!(cx.refusal().unwrap().contains(reason));
        assert!(h.effects.is_empty());
        assert!(h.ram[0x7ff0..0x8400].iter().all(|b| *b == 0xa5));
        assert!(d.save()[0]["dma"]["deadline"].is_null());
    }
}
#[test]
fn raw_readback_waits_for_both_outputs_then_eof() {
    let mut h = TestHost::new();
    let outputs: readback::MemoryOutputHandle = Default::default();
    for i in 0..2 {
        outputs
            .lock()
            .unwrap()
            .configure(
                i,
                Some(dma::Raster {
                    address: 0x9000 + (i * 0x1000) as u64,
                    width: 6,
                    height: 6,
                    pitch: 16,
                }),
            )
            .unwrap();
    }
    let bank: parameters::ParameterHandle = Default::default();
    {
        let mut p = bank.lock().unwrap();
        p.words[0x200 / 4] = 0x1000100;
        p.words[0x204 / 4] = 0x100;
    }
    let mut w = input_wiring();
    w.parameters = Some(bank);
    w.memory_outputs = Some(outputs.clone());
    let mut d = ImageInput::new(input_options(), w).unwrap();
    for (o, v) in [
        (0x394, 0x1000000),
        (0x398, 6 << 16),
        (0x39c, 0x40000),
        (0x3a0, 6),
        (0x3a4, 12),
        (0x3a8, 0x8000),
        (0x3ac, 0xc003),
        (0x3b8, 0xfb000000),
        (0x390, 1),
    ] {
        write(&mut d, &mut h, 0xfff10000 + o, v);
    }
    assert_eq!(d.next_deadline(0), Some(8));
    h.now = 7;
    {
        let mut out = outputs.lock().unwrap();
        for i in 0..2 {
            assert!(out.transfers[i].write(&mut h.cx()).unwrap());
            Irq {
                core: 0,
                line: 44 + i as u32,
            }
            .raise(&mut h.cx())
            .unwrap();
        }
    }
    advance(&mut d, &mut h, 8);
    assert_eq!(h.irqs, [(0, 44), (0, 45), (0, 38)]);
    assert_eq!(read(&mut d, &mut h, 0xfff10000) & 2, 2);
    assert!(!outputs.lock().unwrap().active);
}

#[test]
fn raw_readback_flows_through_both_output_channels_before_eof() {
    use crate::imaging::analysis::ImageLayout;
    use crate::scene::ImageFrames;
    let mut h = TestHost::new();
    h.ram[0x9000..0x9100].fill(0xa5);
    h.ram[0xa000..0xa100].fill(0xa5);
    let memory: readback::MemoryOutputHandle = Default::default();
    let frames = ImageFrames::new(ImageLayout {
        width: 8,
        height: 2,
        stride: 16,
    })
    .unwrap();
    let channels = output::OutputChannels::default();
    let routes = dma::IrqRoutes::default();
    routes.set(44, Some(0)).unwrap();
    routes.set(45, Some(0)).unwrap();
    let make = |channel, base, irq| {
        OutputChannel::new(
            output::Options {
                stop_ticks: 2,
                clock: super::super::dma::Clock::Fixed { ticks: 5 },
                geometry: output::GeometryPolicy::RefuseIncomplete,
                diagnostic_reads: false,
            },
            output::Wiring {
                channel,
                base,
                irq,
                channels: channels.clone(),
                routes: routes.clone(),
                frames: frames.clone(),
                memory_outputs: Some(memory.clone()),
            },
        )
        .unwrap()
    };
    let mut a = make(0, 0xfff12000, 44);
    let mut b = make(1, 0xfff13000, 45);
    for (d, base, address) in [(&mut a, 0xfff12000, 0x9000), (&mut b, 0xfff13000, 0xa000)] {
        for (offset, value) in [
            (0xb0, (5 << 16) | 7),
            (0xb4, 7),
            (0xc0, address),
            (0xc4, 16),
            (0, 0x10001),
        ] {
            write(d, &mut h, base + offset, value);
        }
    }
    let bank: parameters::ParameterHandle = Default::default();
    bank.lock().unwrap().words[0x200 / 4] = 0x1000100;
    bank.lock().unwrap().words[0x204 / 4] = 0x100;
    let mut wiring = input_wiring();
    wiring.parameters = Some(bank);
    wiring.memory_outputs = Some(memory.clone());
    let mut input = ImageInput::new(input_options(), wiring).unwrap();
    for (offset, value) in [
        (0x394, 0x1000000),
        (0x398, 6 << 16),
        (0x39c, 0x40000),
        (0x3a0, 6),
        (0x3a4, 12),
        (0x3a8, 0x8000),
        (0x3ac, 0xc003),
        (0x3b8, 0xfb000000),
        (0x390, 1),
    ] {
        write(&mut input, &mut h, 0xfff10000 + offset, value);
    }
    assert_eq!(a.next_deadline(0), Some(0));
    advance(&mut a, &mut h, 0);
    advance(&mut b, &mut h, 0);
    assert_eq!(a.next_deadline(0), Some(7));
    advance(&mut a, &mut h, 6);
    assert!(h.effects.is_empty());
    advance(&mut a, &mut h, 7);
    assert_eq!(h.effects.len(), 14);
    assert!(h.effects[..12].iter().all(|e| *e == "dma"));
    assert_eq!(h.effects[12..], ["irq", "irq"]);
    for address in [0x9000, 0xa000] {
        for row in 0..6 {
            assert_ne!(
                &h.ram[address + row * 32..address + row * 32 + 16],
                &[0xa5; 16]
            );
            assert_eq!(
                &h.ram[address + row * 32 + 16..address + (row + 1) * 32],
                &[0xa5; 16]
            );
        }
    }
    write(&mut a, &mut h, 0xfff120d4, 1);
    write(&mut b, &mut h, 0xfff130d4, 1);
    advance(&mut input, &mut h, 8);
    assert_eq!(h.irqs, [(0, 44), (0, 45), (0, 38)]);
    // A second START can arrive before the output model observes the idle
    // input. Its new deadline must latch the newly programmed IRQ owners.
    routes.set(44, Some(1)).unwrap();
    routes.set(45, Some(1)).unwrap();
    write(&mut input, &mut h, 0xfff10390, 1);
    assert_eq!(a.next_deadline(8), Some(8));
    advance(&mut a, &mut h, 8);
    advance(&mut a, &mut h, 15);
    advance(&mut input, &mut h, 16);
    assert_eq!(
        h.irqs,
        [(0, 44), (0, 45), (0, 38), (1, 44), (1, 45), (0, 38)]
    );
}

#[test]
fn register_input_needs_no_color_layout_until_readback_is_connected() {
    let mut options = input_options();
    options.cfa = None;
    assert!(ImageInput::new(options.clone(), input_wiring()).is_ok());
    let mut wiring = input_wiring();
    wiring.memory_outputs = Some(Default::default());
    assert!(ImageInput::new(options, wiring).is_err());
}

#[test]
fn source_mask_clock_keeps_line_geometry_out_of_the_frame_period() {
    let mut o = input_options();
    o.clock = Some(input::Clock {
        mode: input::ClockMode::SourceMask,
        source: input::ClockSource::Periodic {
            period_ticks: 200000,
            first_delay_ticks: None,
        },
        secondary: true,
        companion_secondary: true,
    });
    let mut d = ImageInput::new(o, input_wiring()).unwrap();
    let mut h = TestHost::new();
    write(&mut d, &mut h, 0xfff10000, 0);
    advance(&mut d, &mut h, 2);
    write(&mut d, &mut h, 0xfff10024, 1040);
    write(&mut d, &mut h, 0xfff1038c, 0x10003);
    assert_eq!(d.next_deadline(0), Some(200002));
    advance(&mut d, &mut h, 1040);
    assert!(h.irqs.is_empty());
    advance(&mut d, &mut h, 200002);
    assert_eq!(h.irqs, [(0, 39), (1, 41)]);
    assert_eq!(read(&mut d, &mut h, 0xfff10384), 0x201);
    write(&mut d, &mut h, 0xfff10388, 0);
    write(&mut d, &mut h, 0xfff10024, 1100);
    assert_eq!(d.next_deadline(200000), Some(400002));
    advance(&mut d, &mut h, 400002);
    assert_eq!(h.irqs, [(0, 39), (1, 41), (1, 41)]);
}

#[test]
fn programmed_input_extent_is_a_shared_peer_value_and_restores_with_registers() {
    let grid = std::sync::Arc::new(std::sync::Mutex::new([1, 1]));
    let mut wiring = input_wiring();
    wiring.statistics_grid = Some(grid.clone());
    let mut d = ImageInput::new(input_options(), wiring).unwrap();
    let mut m = TestHost::new();
    write(&mut d, &mut m, 0xfff10028, 2495);
    write(&mut d, &mut m, 0xfff1002c, 437);
    let saved = d.checkpoint().unwrap();
    let fingerprint = d.fingerprint().unwrap();
    assert_eq!(*grid.lock().unwrap(), [2496, 438]);
    write(&mut d, &mut m, 0xfff1002c, 231);
    assert_eq!(*grid.lock().unwrap(), [2496, 232]);
    d.restore_checkpoint(&saved).unwrap();
    assert_eq!(*grid.lock().unwrap(), [2496, 438]);
    assert_eq!(fingerprint, d.fingerprint().unwrap());
}

#[test]
fn physical_scan_period_changes_phase_edges_without_host_observation_effects() {
    let mut o = input_options();
    o.clock = Some(input::Clock {
        mode: input::ClockMode::InputEnable,
        source: input::ClockSource::Periodic {
            period_ticks: 100,
            first_delay_ticks: Some(90),
        },
        secondary: true,
        companion_secondary: true,
    });
    let period = std::sync::Arc::new(std::sync::Mutex::new(10));
    let mut w = input_wiring();
    w.frame_period = Some(period.clone());
    let edges = w.edges.clone();
    let mut d = ImageInput::new(o, w).unwrap();
    let mut h = TestHost::new();
    for (o, v) in [(0x48, 1), (0x38c, 0x10003)] {
        write(&mut d, &mut h, 0xfff10000 + o, v);
    }
    assert_eq!(d.next_deadline(0), Some(10));
    advance(&mut d, &mut h, 10);
    assert_eq!(edges.lock().unwrap()[0].unwrap().tick, 10);
    write(&mut d, &mut h, 0xfff10388, 1);
    *period.lock().unwrap() = 4;
    let before = d.save();
    assert_eq!(d.next_deadline(10), Some(10));
    assert_eq!(d.next_deadline(10), Some(10));
    assert_eq!(d.save(), before);
    advance(&mut d, &mut h, 10);
    assert_eq!(d.next_deadline(10), Some(14));
    advance(&mut d, &mut h, 13);
    assert_eq!(edges.lock().unwrap()[0].unwrap().tick, 10);
    advance(&mut d, &mut h, 14);
    assert_eq!(edges.lock().unwrap()[0].unwrap().tick, 14);
}

#[test]
fn raw_window_crops_sensor_coordinates_and_preserves_aligned_row_padding() {
    use crate::scene::{Frame, FramePixels, SensorFrames};
    for packed in [false, true] {
        let frames = std::sync::Arc::new(std::sync::Mutex::new(SensorFrames::new(
            Frame::new(6, 4, FramePixels::Raw16((0..24).collect())).unwrap(),
        )));
        let sensor = crate::parts::sensor::Sensor::new(
            crate::parts::sensor::Options {
                width: 6,
                height: 4,
                image_area: [0, 0, 6, 4],
                cfa: cfa(),
                black: 0,
                full_scale: 4095,
                reference_white: [256; 3],
            },
            frames,
        )
        .unwrap();
        let mut o = input_options();
        o.sensor_period_ticks = Some(10);
        let mut w = input_wiring();
        w.sensor = Some(sensor);
        let mut d = ImageInput::new(o, w).unwrap();
        let mut h = TestHost::new();
        h.ram[0x8000..0x8400].fill(0xa5);
        for (offset, value) in [
            (4, 1 | if packed { 0x10000 } else { 0 }),
            (0x14, 2 << 16 | 1),
            (0x18, 1 << 16 | 1),
            (0x24, 2),
            (0x354, 0x10300000),
            (0x358, 0x8000),
            (0x35c, 1),
            (0x360, 16),
        ] {
            write(&mut d, &mut h, 0xfff10000 + offset, value);
        }
        advance(&mut d, &mut h, 10);
        assert!(h.effects.is_empty());
        advance(&mut d, &mut h, 15);
        let (pitch, first, second): (usize, Vec<u8>, Vec<u8>) = if packed {
            (400, vec![8, 0x90, 0], vec![14, 0xf0, 0])
        } else {
            (144, vec![8, 0, 9, 0], vec![14, 0, 15, 0])
        };
        assert_eq!(&h.ram[0x8000..0x8000 + first.len()], &first);
        assert_eq!(
            &h.ram[0x8000 + pitch..0x8000 + pitch + second.len()],
            &second
        );
        assert!(
            h.ram[0x8000 + first.len()..0x8000 + pitch]
                .iter()
                .all(|v| *v == 0xa5)
        );
        assert_eq!(h.ram[0x8000 + pitch * 2], 0xa5);
        assert_eq!(h.irqs, [(0, 36)]);
        write(&mut d, &mut h, 0xfff10350, 1);
        write(&mut d, &mut h, 0xfff10014, 5 << 16 | 1);
        h.effects.clear();
        h.now = 20;
        let mut cx = h.cx();
        d.advance(&mut cx);
        assert!(cx.refusal().unwrap().contains("bounded input window"));
        assert!(h.effects.is_empty());
    }
}

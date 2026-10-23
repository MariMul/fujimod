use fujimod_emulator_devices::exr2::isp::dma::IrqRoutes;
use crate::*;
use super::liveview_support::*;
use super::producer::producer;
use fujimod_emulator_devices::Cx;
use fujimod_emulator_devices::Device;
use fujimod_emulator_devices::imaging::analysis::ImageLayout;
use fujimod_emulator_devices::scene::{FrameEdge, ImageFrames};
fn output(base: u64, irq: u32) -> (OutputChannel, ImageFrames, IrqRoutes) {
    let frames = ImageFrames::new(ImageLayout {
        width: 8,
        height: 2,
        stride: 16,
    })
    .unwrap();
    let routes = routes(irq);
    let d = OutputChannel::new(
        output::Options {
            stop_ticks: 2,
            clock: fujimod_emulator_devices::exr2::isp::dma::Clock::Fixed { ticks: 5 },
            geometry: output::GeometryPolicy::RefuseIncomplete,
            diagnostic_reads: true,
        },
        output::Wiring {
            base,
            irq,
            channel: usize::from(irq == 0x2d),
            channels: output::OutputChannels::default(),
            frames: frames.clone(),
            memory_outputs: None,
            routes: routes.clone(),
        },
    )
    .unwrap();
    (d, frames, routes)
}
fn arm_output(d: &mut OutputChannel, m: &mut Memory, base: u64) {
    for (o, v) in [
        (4, 0x10007),
        (0xb0, 0x10007),
        (0xb4, 7),
        (0xc0, 0x4000),
        (0xc4, 8),
        (0xd8, 1),
        (0, 0x10001),
    ] {
        write(d, m, base + o, v);
    }
}
#[test]
fn output_latches_pixels_and_address_before_irq_and_waits_for_ack() {
    for (base, irq) in [(0xfff12000, 0x2c), (0xfff13000, 0x2d)] {
        let (mut d, frames, _) = output(base, irq);
        let mut m = Memory::default();
        arm_output(&mut d, &mut m, base);
        frames
            .publish(FrameEdge { index: 1, tick: 10 }, &[7; 32])
            .unwrap();
        assert_eq!(d.next_deadline(10), Some(10));
        advance(&mut d, &mut m, 10);
        assert_eq!(d.next_deadline(10), Some(15));
        write(&mut d, &mut m, base + 0xc0, 0x8000);
        // The programmed destination is for a later transfer. Observation
        // identifies the in-flight source and latched address without DMA.
        let before = m.effects.len();
        let state = d.save();
        assert_eq!(state["registers"][0xc0 / 4], 0x8000);
        assert_eq!(state["source_frame"], 1);
        assert_eq!(state["span"], serde_json::json!([0x4000, 0x4020]));
        assert_eq!(state["completed"], false);
        assert_eq!(state, d.save());
        assert_eq!(m.effects.len(), before);
        frames
            .publish(FrameEdge { index: 2, tick: 11 }, &[9; 32])
            .unwrap();
        advance(&mut d, &mut m, 14);
        assert!(m.effects.is_empty());
        advance(&mut d, &mut m, 15);
        assert_eq!(&m.bytes[0x4000..0x4020], &[7; 32]);
        assert_eq!(m.effects, [Effect::Dma(0x4000, 32), Effect::Irq(0, irq)]);
        assert_eq!(d.next_deadline(15), None);
        frames
            .publish(FrameEdge { index: 3, tick: 20 }, &[11; 32])
            .unwrap();
        advance(&mut d, &mut m, 20);
        assert_eq!(d.next_deadline(20), None);
        write(&mut d, &mut m, base + 0xd4, 1);
        frames
            .publish(FrameEdge { index: 4, tick: 30 }, &[13; 32])
            .unwrap();
        advance(&mut d, &mut m, 30);
        advance(&mut d, &mut m, 35);
        assert_eq!(&m.bytes[0x8000..0x8020], &[13; 32]);
    }
}
#[test]
fn output_stop_route_withdrawal_and_refusals() {
    let base = 0xfff12000;
    let (mut d, frames, routes) = output(base, 0x2c);
    let mut m = Memory::default();
    arm_output(&mut d, &mut m, base);
    reject(&mut d, &mut m, base + 0xb8, 5);
    reject(&mut d, &mut m, base + 0xb0, 6);
    reject(&mut d, &mut m, base + 0xd8, 0);
    frames
        .publish(FrameEdge { index: 1, tick: 0 }, &[4; 32])
        .unwrap();
    advance(&mut d, &mut m, 0);
    routes.set(0x2c, None).unwrap();
    advance(&mut d, &mut m, 5);
    assert!(m.effects.is_empty());
    write(&mut d, &mut m, base, 0);
    assert_eq!(d.next_deadline(5), Some(7));
    advance(&mut d, &mut m, 6);
    assert_eq!(read(&mut d, &mut m, base), 0);
    advance(&mut d, &mut m, 7);
    assert_eq!(read(&mut d, &mut m, base), 1);
    assert_eq!(d.next_deadline(7), None);
}
#[test]
fn output_banks_refuse_physical_alias_overlap() {
    let frames = ImageFrames::new(ImageLayout {
        width: 8,
        height: 2,
        stride: 16,
    })
    .unwrap();
    let channels = output::OutputChannels::default();
    let routes = routes(0x2c);
    routes.set(0x2d, Some(0)).unwrap();
    let make = |base, irq, channel| {
        OutputChannel::new(
            output::Options {
                stop_ticks: 2,
                clock: fujimod_emulator_devices::exr2::isp::dma::Clock::Fixed { ticks: 5 },
                geometry: output::GeometryPolicy::RefuseIncomplete,
                diagnostic_reads: false,
            },
            output::Wiring {
                base,
                irq,
                channel,
                channels: channels.clone(),
                frames: frames.clone(),
                memory_outputs: None,
                routes: routes.clone(),
            },
        )
        .unwrap()
    };
    let mut a = make(0xfff12000, 0x2c, 0);
    let mut b = make(0xfff13000, 0x2d, 1);
    let mut m = Memory::default();
    arm_output(&mut a, &mut m, 0xfff12000);
    arm_output(&mut b, &mut m, 0xfff13000);
    frames
        .publish(FrameEdge { index: 1, tick: 0 }, &[4; 32])
        .unwrap();
    advance(&mut a, &mut m, 0);
    let mut cx = Cx::new(&mut m, 0, 0, 1);
    b.advance(&mut cx);
    assert!(cx.refusal().unwrap().contains("overlap"));
    assert!(m.effects.is_empty());
}
#[test]
fn paired_output_writes_both_payloads_before_either_irq_and_waits_for_both_acks() {
    let frames = ImageFrames::new(ImageLayout {
        width: 8,
        height: 2,
        stride: 16,
    })
    .unwrap();
    let channels = output::OutputChannels::default();
    let routes = routes(0x2c);
    routes.set(0x2d, Some(0)).unwrap();
    let make = |base, irq, channel| {
        OutputChannel::new(
            output::Options {
                stop_ticks: 2,
                clock: fujimod_emulator_devices::exr2::isp::dma::Clock::Fixed { ticks: 5 },
                geometry: output::GeometryPolicy::RefuseIncomplete,
                diagnostic_reads: false,
            },
            output::Wiring {
                base,
                irq,
                channel,
                channels: channels.clone(),
                frames: frames.clone(),
                memory_outputs: None,
                routes: routes.clone(),
            },
        )
        .unwrap()
    };
    let mut a = make(0xfff12000, 0x2c, 0);
    let mut b = make(0xfff13000, 0x2d, 1);
    let mut m = Memory::default();
    arm_output(&mut a, &mut m, 0xfff12000);
    arm_output(&mut b, &mut m, 0xfff13000);
    write(&mut b, &mut m, 0xfff130c0, 0x8000);
    frames
        .publish(FrameEdge { index: 1, tick: 0 }, &[4; 32])
        .unwrap();
    advance(&mut a, &mut m, 0);
    advance(&mut b, &mut m, 0);
    advance(&mut a, &mut m, 5);
    advance(&mut b, &mut m, 5);
    assert_eq!(
        m.effects,
        [
            Effect::Dma(0x4000, 32),
            Effect::Dma(0x8000, 32),
            Effect::Irq(0, 0x2c),
            Effect::Irq(0, 0x2d)
        ]
    );
    write(&mut a, &mut m, 0xfff120d4, 1);
    frames
        .publish(FrameEdge { index: 2, tick: 10 }, &[5; 32])
        .unwrap();
    advance(&mut a, &mut m, 10);
    advance(&mut b, &mut m, 10);
    assert_eq!(a.next_deadline(10), None);
    assert_eq!(b.next_deadline(10), None);
    write(&mut b, &mut m, 0xfff130d4, 1);
    frames
        .publish(FrameEdge { index: 3, tick: 20 }, &[6; 32])
        .unwrap();
    advance(&mut a, &mut m, 20);
    advance(&mut b, &mut m, 20);
    advance(&mut b, &mut m, 25);
    assert_eq!(m.effects.len(), 8);
    assert_eq!(&m.bytes[0x4000..0x4020], &[6; 32]);
    assert_eq!(&m.bytes[0x8000..0x8020], &[6; 32]);
}
#[test]
fn unequal_output_rasters_wait_for_later_dma_before_publishing_both_irqs() {
    let frames = ImageFrames::new(ImageLayout {
        width: 8,
        height: 2,
        stride: 16,
    })
    .unwrap();
    let channels = output::OutputChannels::default();
    let routes = routes(0x2c);
    routes.set(0x2d, Some(0)).unwrap();
    let make = |base, irq, channel| {
        OutputChannel::new(
            output::Options {
                stop_ticks: 2,
                clock: fujimod_emulator_devices::exr2::isp::dma::Clock::Throughput {
                    bytes_per_tick: 16,
                },
                geometry: output::GeometryPolicy::RefuseIncomplete,
                diagnostic_reads: false,
            },
            output::Wiring {
                base,
                irq,
                channel,
                channels: channels.clone(),
                frames: frames.clone(),
                memory_outputs: None,
                routes: routes.clone(),
            },
        )
        .unwrap()
    };
    let mut a = make(0xfff12000, 0x2c, 0);
    let mut b = make(0xfff13000, 0x2d, 1);
    let mut m = Memory::default();
    arm_output(&mut a, &mut m, 0xfff12000);
    arm_output(&mut b, &mut m, 0xfff13000);
    write(&mut b, &mut m, 0xfff130c0, 0x8000);
    for (o, v) in [(0xb0, 0x1000f), (0xb4, 15), (0xc4, 16)] {
        write(&mut b, &mut m, 0xfff13000 + o, v);
    }
    frames
        .publish(FrameEdge { index: 1, tick: 0 }, &[4; 32])
        .unwrap();
    advance(&mut a, &mut m, 0);
    advance(&mut b, &mut m, 0);
    assert_eq!(a.next_deadline(0), Some(4));
    assert_eq!(b.next_deadline(0), Some(4));
    advance(&mut a, &mut m, 2);
    assert!(m.effects.is_empty());
    advance(&mut b, &mut m, 4);
    assert_eq!(
        m.effects,
        [
            Effect::Dma(0x4000, 32),
            Effect::Dma(0x8000, 64),
            Effect::Irq(0, 0x2c),
            Effect::Irq(0, 0x2d)
        ]
    );
    assert_eq!(&m.bytes[0x4000..0x4020], &[4; 32]);
    assert_eq!(&m.bytes[0x8000..0x8040], &[4; 64]);
}
#[test]
fn worker_output_keeps_latched_pixels_cancellation_and_paired_publication() {
    fujimod_emulator_devices::work::configure(2).unwrap();
    output_latches_pixels_and_address_before_irq_and_waits_for_ack();
    output_stop_route_withdrawal_and_refusals();
    output_banks_refuse_physical_alias_overlap();
    paired_output_writes_both_payloads_before_either_irq_and_waits_for_both_acks();
    unequal_output_rasters_wait_for_later_dma_before_publishing_both_irqs();
    fujimod_emulator_devices::work::configure(0).unwrap();
}
#[test]
fn repeating_output_and_statistics_do_not_allocate_per_frame() {
    let base = 0xfff12000;
    let (mut output, image, _) = output(base, 0x2c);
    let mut om = Memory::default();
    arm_output(&mut output, &mut om, base);
    om.effects.reserve(256);
    let (mut statistics, frames, statistics_image, _, mut sm) = producer(true);
    sm.effects.reserve(512);
    ALLOCATIONS.with(|n| n.set(Some(0)));
    for index in 1..=64 {
        let tick = index * 20;
        image.publish(FrameEdge { index, tick }, &[7; 32]).unwrap();
        advance(&mut output, &mut om, tick);
        advance(&mut output, &mut om, tick + 5);
        write(&mut output, &mut om, base + 0xd4, 1);
        statistics_image
            .publish(FrameEdge { index, tick }, &[5; 16])
            .unwrap();
        frames.publish(FrameEdge { index, tick }).unwrap();
        advance(&mut statistics, &mut sm, tick);
        advance(&mut statistics, &mut sm, tick + 5);
        for ch in 0..2 {
            let clear = 0xfff1888c + ch * 0x100;
            let flag = 0xfff188f4 + ch * 0x100;
            write(&mut statistics, &mut sm, clear, 0);
            write(&mut statistics, &mut sm, flag, 1);
            let now = sm.now + 2;
            advance(&mut statistics, &mut sm, now);
            read(&mut statistics, &mut sm, clear);
            read(&mut statistics, &mut sm, flag);
        }
    }
    let count = ALLOCATIONS.with(|n| n.replace(None).unwrap());
    assert_eq!(count, 0);
}
#[test]
fn native_preview_cadence_uses_physical_input_and_keeps_image_edges_and_stats_phase() {
    use fujimod_emulator_devices::imaging::analysis::ImageLayout;
    use fujimod_emulator_devices::parts::pins::{InputPins, Pin, Selection};
    use fujimod_emulator_devices::scene::{
        ImageFrames, StatisticsCadence, StatisticsFrames, StatisticsSpan, clocked,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct Input(Arc<AtomicBool>);
    impl InputPins for Input {
        fn level(&self, _: Pin) -> Result<bool, String> {
            Ok(self.0.load(Ordering::Relaxed))
        }
    }
    let level = Arc::new(AtomicBool::new(true));
    let images = ImageFrames::new(ImageLayout {
        width: 2,
        height: 1,
        stride: 4,
    })
    .unwrap();
    let statistics = StatisticsFrames::new(vec![StatisticsSpan {
        destination: 2,
        offset: 0,
        bytes: vec![0; 4],
    }])
    .unwrap();
    let mut source = clocked::ClockedFrames::new(
        clocked::Options {
            phase_source: None,
            period_ticks: 10,
            optics: None,
        },
        clocked::Wiring {
            sensor_clock: None,
            frame_period: None,

            phase_edges: None,
            position: None,
            renderer: None,
            images: images.clone(),
            statistics: statistics.clone(),
            start_tick: 0,
            activity: Some((
                Box::new(Input(level.clone())),
                Selection {
                    pin: Pin { port: 0, pin: 2 },
                    high: false,
                },
            )),
        },
    )
    .unwrap();
    source
        .configure(
            10,
            StatisticsCadence::Activity {
                idle_frames: 3,
                active_frames: 1,
            },
            0,
        )
        .unwrap();
    let before = source.save();
    assert!(
        source
            .configure(1, StatisticsCadence::EveryNth { frames: 0 }, 0)
            .is_err()
    );
    assert_eq!(source.save(), before);
    let mut memory = super::liveview_support::Memory::default();
    for tick in [10, 20] {
        super::liveview_support::advance(&mut source, &mut memory, tick);
    }
    assert_eq!(images.0.lock().unwrap().edge.unwrap().index, 2);
    assert!(statistics.0.lock().unwrap().edge.is_none());
    super::liveview_support::advance(&mut source, &mut memory, 30);
    assert_eq!(statistics.0.lock().unwrap().edge.unwrap().index, 3);
    level.store(false, Ordering::Relaxed);
    super::liveview_support::advance(&mut source, &mut memory, 40);
    assert_eq!(statistics.0.lock().unwrap().edge.unwrap().index, 4);
    level.store(true, Ordering::Relaxed);
    source
        .configure(
            20,
            StatisticsCadence::Activity {
                idle_frames: 3,
                active_frames: 1,
            },
            41,
        )
        .unwrap();
    assert_eq!(source.next_deadline(41), Some(50));
    super::liveview_support::advance(&mut source, &mut memory, 50);
    assert_eq!(statistics.0.lock().unwrap().edge.unwrap().index, 4);
    assert_eq!(source.next_deadline(50), Some(70));
    super::liveview_support::advance(&mut source, &mut memory, 70);
    assert_eq!(statistics.0.lock().unwrap().edge.unwrap().index, 6);
}
#[test]
fn external_frames_latch_host_pixels_only_at_due_edges_and_animate_in_guest_time() {
    use fujimod_emulator_devices::imaging::analysis::ImageLayout;
    use fujimod_emulator_devices::scene::{ImageFrames, StatisticsFrames, StatisticsSpan, clocked};
    let images = ImageFrames::new(ImageLayout {
        width: 4,
        height: 2,
        stride: 8,
    })
    .unwrap();
    let statistics = StatisticsFrames::new(vec![StatisticsSpan {
        destination: 2,
        offset: 0,
        bytes: vec![0; 4],
    }])
    .unwrap();
    let mut source = clocked::ClockedFrames::new(
        clocked::Options {
            phase_source: None,
            period_ticks: 10,
            optics: None,
        },
        clocked::Wiring {
            sensor_clock: None,
            frame_period: None,

            phase_edges: None,
            position: None,
            renderer: None,
            images: images.clone(),
            statistics: statistics.clone(),
            activity: None,
            start_tick: 0,
        },
    )
    .unwrap();
    let mut memory = super::liveview_support::Memory::default();
    images.prepare_rgb(1, 1, &[255, 0, 0]).unwrap();
    assert_eq!(images.0.lock().unwrap().edge, None);
    super::liveview_support::advance(&mut source, &mut memory, 9);
    assert_eq!(images.0.lock().unwrap().edge, None);
    super::liveview_support::advance(&mut source, &mut memory, 10);
    let red = {
        let image = images.0.lock().unwrap();
        image.pixels.render(image.layout, 0).unwrap()
    };
    assert_eq!(&red[..4], &[77, 77, 85, 255]);
    images.prepare_rgb(1, 1, &[0, 0, 255]).unwrap();
    assert_eq!(
        {
            let image = images.0.lock().unwrap();
            image.pixels.render(image.layout, 0).unwrap()
        },
        red
    );
    let before = source.save();
    assert_eq!(source.save(), before);
    super::liveview_support::advance(&mut source, &mut memory, 20);
    assert_ne!(
        {
            let image = images.0.lock().unwrap();
            image.pixels.render(image.layout, 0).unwrap()
        },
        red
    );
    assert_eq!(
        statistics.0.lock().unwrap().edge,
        images.0.lock().unwrap().edge
    );
    images.use_analytic();
    super::liveview_support::advance(&mut source, &mut memory, 30);
    let a = {
        let image = images.0.lock().unwrap();
        image.pixels.render(image.layout, 0).unwrap()
    };
    super::liveview_support::advance(&mut source, &mut memory, 39);
    assert_eq!(
        {
            let image = images.0.lock().unwrap();
            image.pixels.render(image.layout, 0).unwrap()
        },
        a
    );
    super::liveview_support::advance(&mut source, &mut memory, 40);
    assert_ne!(
        {
            let image = images.0.lock().unwrap();
            image.pixels.render(image.layout, 0).unwrap()
        },
        a
    );
}

#[test]
fn wired_frame_clock_publishes_pixels_and_statistics_on_the_same_physical_edge() {
    use fujimod_emulator_devices::scene::{FrameEdge, ImageFrames, StatisticsFrames, clocked};
    let images = ImageFrames::new(fujimod_emulator_devices::imaging::analysis::ImageLayout {
        width: 2,
        height: 1,
        stride: 4,
    })
    .unwrap();
    let statistics = StatisticsFrames::new(vec![]).unwrap();
    let edges: fujimod_emulator_devices::scene::FramePhases = Default::default();
    let mut source = clocked::ClockedFrames::new(
        clocked::Options {
            period_ticks: 10,
            phase_source: Some(1),
            optics: None,
        },
        clocked::Wiring {
            sensor_clock: None,

            frame_period: None,

            renderer: None,
            images: images.clone(),
            statistics: statistics.clone(),
            activity: None,
            start_tick: 0,
            position: None,
            phase_edges: Some(edges.clone()),
        },
    )
    .unwrap();
    let mut memory = super::liveview_support::Memory::default();
    assert_eq!(source.next_deadline(10), None);
    images.prepare_rgb(1, 1, &[255, 0, 0]).unwrap();
    edges.lock().unwrap()[0] = Some(FrameEdge { index: 1, tick: 17 });
    assert_eq!(source.next_deadline(17), None);
    let edge = FrameEdge { index: 3, tick: 27 };
    edges.lock().unwrap()[1] = Some(edge);
    assert_eq!(source.next_deadline(27), Some(27));
    super::liveview_support::advance(&mut source, &mut memory, 27);
    assert_eq!(images.0.lock().unwrap().edge, Some(edge));
    assert_eq!(statistics.0.lock().unwrap().edge, Some(edge));
    assert_eq!(
        &{
            let image = images.0.lock().unwrap();
            image.pixels.render(image.layout, 0).unwrap()
        }[..4],
        &[77, 77, 85, 255]
    );
    assert_eq!(source.next_deadline(28), None);
    let saved = source.checkpoint().unwrap();
    let fingerprint = source.fingerprint().unwrap();
    let (
        clock,
        frames,
        cadence,
        optics,
        renderer,
        _,
        external_period,
        frame_period,
        sensor_clock,
        seen_sensor,
        seen_external,
        last_physical,
    ): (
        fujimod_emulator_devices::scene::FrameCadence,
        u64,
        fujimod_emulator_devices::scene::StatisticsCadence,
        bool,
        bool,
        Option<usize>,
        u64,
        bool,
        bool,
        Option<FrameEdge>,
        Option<FrameEdge>,
        bool,
    ) = saved.decode(source.name()).unwrap();
    let wrong_phase = fujimod_emulator_devices::checkpoint::Checkpoint::encode(
        source.name(),
        &(
            clock,
            frames,
            cadence,
            optics,
            renderer,
            Some(0usize),
            external_period,
            frame_period,
            sensor_clock,
            seen_sensor,
            seen_external,
            last_physical,
        ),
    )
    .unwrap();
    assert!(source.restore_checkpoint(&wrong_phase).is_err());
    assert_eq!(source.fingerprint().unwrap(), fingerprint);
    source.restore_checkpoint(&saved).unwrap();
    assert_eq!(source.fingerprint().unwrap(), fingerprint);
    super::liveview_support::advance(&mut source, &mut memory, 30);
    assert_eq!(images.0.lock().unwrap().edge, Some(edge));
    assert!(
        source
            .configure(
                5,
                fujimod_emulator_devices::scene::StatisticsCadence::EveryNth { frames: 1 },
                30
            )
            .is_err()
    );
}

#[test]
fn wired_exposures_latch_physical_focus_until_the_next_input_edge() {
    use fujimod_emulator_devices::scene::{FrameEdge, ImageFrames, StatisticsFrames, clocked, optics};
    let images = ImageFrames::new(fujimod_emulator_devices::imaging::analysis::ImageLayout {
        width: 8,
        height: 1,
        stride: 16,
    })
    .unwrap();
    let rgb: Vec<u8> = (0..8)
        .flat_map(|x| [if x < 4 { 0 } else { 255 }; 3])
        .collect();
    images.prepare_rgb(8, 1, &rgb).unwrap();
    let edges: fujimod_emulator_devices::scene::FramePhases = Default::default();
    let position = optics::Position::default();
    let statistics = StatisticsFrames::new(vec![]).unwrap();
    let mut source = clocked::ClockedFrames::new(
        clocked::Options {
            period_ticks: 99,
            phase_source: Some(0),
            optics: Some(optics::Options {
                target_position: 0,
                pulses_per_radius: 8,
                maximum_radius: 32,
            }),
        },
        clocked::Wiring {
            sensor_clock: None,

            frame_period: None,

            renderer: None,
            images: images.clone(),
            statistics: statistics.clone(),
            activity: None,
            start_tick: 0,
            position: Some(position.clone()),
            phase_edges: Some(edges.clone()),
        },
    )
    .unwrap();
    let mut memory = super::liveview_support::Memory::default();
    edges.lock().unwrap()[0] = Some(FrameEdge { index: 1, tick: 10 });
    super::liveview_support::advance(&mut source, &mut memory, 10);
    let sharp = {
        let image = images.0.lock().unwrap();
        image.pixels.render(image.layout, 0).unwrap()
    };
    position.publish(64);
    let saved = source.save();
    assert_eq!(saved["focus_position"], 0);
    super::liveview_support::advance(&mut source, &mut memory, 11);
    assert_eq!(source.save(), saved);
    assert_eq!(
        {
            let image = images.0.lock().unwrap();
            image.pixels.render(image.layout, 0).unwrap()
        },
        sharp
    );
    edges.lock().unwrap()[0] = Some(FrameEdge { index: 2, tick: 20 });
    super::liveview_support::advance(&mut source, &mut memory, 20);
    assert_eq!(source.save()["focus_position"], 64);
    assert_ne!(
        {
            let image = images.0.lock().unwrap();
            image.pixels.render(image.layout, 0).unwrap()
        },
        sharp
    );
    let source_snapshot = source.checkpoint().unwrap();
    let image_snapshot = images.checkpoint().unwrap();
    let statistics_snapshot = statistics.checkpoint().unwrap();
    images.prepare_rgb(1, 1, &[0, 0, 255]).unwrap();
    edges.lock().unwrap()[0] = Some(FrameEdge { index: 3, tick: 25 });
    super::liveview_support::advance(&mut source, &mut memory, 25);
    source.restore_checkpoint(&source_snapshot).unwrap();
    images.restore_checkpoint(&image_snapshot).unwrap();
    statistics.restore_checkpoint(&statistics_snapshot).unwrap();
    position.publish(0);
    edges.lock().unwrap()[0] = Some(FrameEdge { index: 3, tick: 30 });
    super::liveview_support::advance(&mut source, &mut memory, 30);
    assert_eq!(
        {
            let image = images.0.lock().unwrap();
            image.pixels.render(image.layout, 0).unwrap()
        },
        sharp
    );
}

#[test]
fn sensor_frame_clock_changes_are_causal_and_external_pixels_keep_their_clock() {
    use fujimod_emulator_devices::imaging::analysis::ImageLayout;
    use fujimod_emulator_devices::scene::{ImageFrames, StatisticsFrames, clocked};
    let images = ImageFrames::new(ImageLayout {
        width: 2,
        height: 1,
        stride: 4,
    })
    .unwrap();
    let period = std::sync::Arc::new(std::sync::Mutex::new(4));
    let mut source = clocked::ClockedFrames::new(
        clocked::Options {
            phase_source: None,
            optics: None,
            period_ticks: 10,
        },
        clocked::Wiring {
            position: None,
            phase_edges: None,

            sensor_clock: None,
            frame_period: Some(period.clone()),
            renderer: None,
            images: images.clone(),
            statistics: StatisticsFrames::new(vec![
                fujimod_emulator_devices::scene::StatisticsSpan {
                    destination: 2,
                    offset: 0,
                    bytes: vec![0; 4],
                },
            ])
            .unwrap(),
            activity: None,
            start_tick: 0,
        },
    )
    .unwrap();
    // Fixed external post-ISP samples are not retimed by sensor registers.
    assert_eq!(source.next_deadline(0), Some(10));
    images.use_sensor();
    let before = source.save();
    assert_eq!(source.next_deadline(1), Some(1));
    assert_eq!(source.save(), before);
    // Sync before an edge is due: no renderer or pixels are needed yet.
    let mut memory = super::liveview_support::Memory::default();
    super::liveview_support::advance(&mut source, &mut memory, 1);
    assert_eq!(source.next_deadline(1), Some(5));
    *period.lock().unwrap() = 8;
    super::liveview_support::advance(&mut source, &mut memory, 2);
    assert_eq!(source.next_deadline(2), Some(5));
    images.prepare_rgb(1, 1, &[0; 3]).unwrap();
    assert_eq!(source.next_deadline(2), Some(2));
    super::liveview_support::advance(&mut source, &mut memory, 2);
    super::liveview_support::advance(&mut source, &mut memory, 5);
    assert_eq!(source.next_deadline(5), Some(15));
}
#[test]
fn radiometric_statistics_wait_for_the_physical_sensor_edge() {
    use fujimod_emulator_devices::imaging::{exposure::Calibration, radiometric::Renderer};
    use fujimod_emulator_devices::scene::{Frame, FramePixels, SensorFrames, StatisticsFrames, clocked};
    use std::sync::{Arc, Mutex};
    let frames = Arc::new(Mutex::new(SensorFrames::new(
        Frame::new(1, 1, FramePixels::Rgb24(vec![128; 3].into())).unwrap(),
    )));
    let images = ImageFrames::new(ImageLayout {
        width: 2,
        height: 1,
        stride: 4,
    })
    .unwrap();
    let statistics = StatisticsFrames::new(vec![fujimod_emulator_devices::scene::StatisticsSpan {
        destination: 2,
        offset: 0,
        bytes: vec![0; 4],
    }])
    .unwrap();
    let renderer = Renderer::new(
        Calibration {
            full_scale: 16380,
            reference_white: [256; 3],
            saturation_lux_seconds: 1.,
            color_sample_weights: [1; 3],
        },
        frames.clone(),
        Default::default(),
        Arc::new(Mutex::new(8.)),
        Default::default(),
        None,
    )
    .unwrap();
    let mut source = clocked::ClockedFrames::new(
        clocked::Options {
            phase_source: None,
            optics: None,
            period_ticks: 10,
        },
        clocked::Wiring {
            position: None,
            phase_edges: None,

            sensor_clock: Some(frames.clone()),
            frame_period: None,
            renderer: Some(renderer),
            images: images.clone(),
            statistics: statistics.clone(),
            activity: None,
            start_tick: 0,
        },
    )
    .unwrap();
    let mut memory = super::liveview_support::Memory::default();
    assert_eq!(source.next_deadline(100), None);
    frames.lock().unwrap().latch(105).unwrap();
    assert_eq!(source.next_deadline(104), Some(105));
    let saved = source.save();
    assert_eq!(source.next_deadline(105), Some(105));
    assert_eq!(source.save(), saved);
    super::liveview_support::advance(&mut source, &mut memory, 105);
    assert_eq!(
        images.0.lock().unwrap().edge,
        Some(FrameEdge {
            index: 1,
            tick: 105
        })
    );
    assert_eq!(
        statistics.0.lock().unwrap().edge,
        images.0.lock().unwrap().edge
    );
    assert_eq!(source.next_deadline(200), None);
    frames.lock().unwrap().latch(205).unwrap();
    super::liveview_support::advance(&mut source, &mut memory, 205);
    assert_eq!(
        images.0.lock().unwrap().edge,
        Some(FrameEdge {
            index: 2,
            tick: 205
        })
    );
    frames.lock().unwrap().latch(305).unwrap();
    frames.lock().unwrap().latch(405).unwrap();
    super::liveview_support::advance(&mut source, &mut memory, 405);
    assert_eq!(
        images.0.lock().unwrap().edge,
        Some(FrameEdge {
            index: 4,
            tick: 405
        })
    );
}

#[test]
fn delayed_external_observation_preserves_absolute_statistics_phase() {
    use fujimod_emulator_devices::scene::{
        StatisticsCadence, StatisticsFrames, StatisticsSpan, clocked,
    };
    let images = ImageFrames::new(ImageLayout {
        width: 2,
        height: 1,
        stride: 4,
    })
    .unwrap();
    let statistics = StatisticsFrames::new(vec![StatisticsSpan {
        destination: 2,
        offset: 0,
        bytes: vec![0; 4],
    }])
    .unwrap();
    let mut source = clocked::ClockedFrames::new(
        clocked::Options {
            phase_source: None,
            optics: None,
            period_ticks: 10,
        },
        clocked::Wiring {
            position: None,
            phase_edges: None,

            sensor_clock: None,
            frame_period: None,
            renderer: None,
            images: images.clone(),
            statistics: statistics.clone(),
            activity: None,
            start_tick: 0,
        },
    )
    .unwrap();
    source
        .configure(10, StatisticsCadence::EveryNth { frames: 4 }, 0)
        .unwrap();
    let mut memory = super::liveview_support::Memory::default();
    super::liveview_support::advance(&mut source, &mut memory, 40);
    assert_eq!(
        statistics.0.lock().unwrap().edge,
        Some(FrameEdge { index: 4, tick: 40 })
    );
    super::liveview_support::advance(&mut source, &mut memory, 80);
    assert_eq!(
        statistics.0.lock().unwrap().edge,
        Some(FrameEdge { index: 8, tick: 80 })
    );
}

#[test]
fn switching_frame_sources_waits_for_a_new_edge_and_keeps_external_clock_phase() {
    use fujimod_emulator_devices::imaging::{exposure::Calibration, radiometric::Renderer};
    use fujimod_emulator_devices::scene::{Frame, FramePixels, SensorFrames, StatisticsFrames, clocked};
    use std::sync::{Arc, Mutex};
    let frames = Arc::new(Mutex::new(SensorFrames::new(
        Frame::new(1, 1, FramePixels::Rgb24(vec![128; 3].into())).unwrap(),
    )));
    let images = ImageFrames::new(ImageLayout {
        width: 2,
        height: 1,
        stride: 4,
    })
    .unwrap();
    let statistics = StatisticsFrames::new(vec![fujimod_emulator_devices::scene::StatisticsSpan {
        destination: 2,
        offset: 0,
        bytes: vec![0; 4],
    }])
    .unwrap();
    let renderer = Renderer::new(
        Calibration {
            full_scale: 16380,
            reference_white: [256; 3],
            saturation_lux_seconds: 1.,
            color_sample_weights: [1; 3],
        },
        frames.clone(),
        Default::default(),
        Arc::new(Mutex::new(8.)),
        Default::default(),
        None,
    )
    .unwrap();
    let mut source = clocked::ClockedFrames::new(
        clocked::Options {
            phase_source: None,
            optics: None,
            period_ticks: 10,
        },
        clocked::Wiring {
            position: None,
            phase_edges: None,

            sensor_clock: Some(frames.clone()),
            frame_period: None,
            renderer: Some(renderer),
            images: images.clone(),
            statistics: statistics.clone(),
            activity: None,
            start_tick: 0,
        },
    )
    .unwrap();
    let mut memory = super::liveview_support::Memory::default();
    frames.lock().unwrap().latch(5).unwrap();
    super::liveview_support::advance(&mut source, &mut memory, 5);
    let physical = images.0.lock().unwrap().edge;
    assert_eq!(physical, Some(FrameEdge { index: 1, tick: 5 }));
    images.prepare_rgb(1, 1, &[255, 0, 0]).unwrap();
    assert_eq!(source.next_deadline(40), Some(40));
    super::liveview_support::advance(&mut source, &mut memory, 40);
    assert_eq!(images.0.lock().unwrap().edge, physical);
    assert_eq!(source.next_deadline(40), Some(50));
    frames.lock().unwrap().latch(45).unwrap();
    super::liveview_support::advance(&mut source, &mut memory, 49);
    assert_eq!(images.0.lock().unwrap().edge, physical);
    super::liveview_support::advance(&mut source, &mut memory, 50);
    let external = images.0.lock().unwrap().edge;
    assert_eq!(external, Some(FrameEdge { index: 6, tick: 50 }));
    assert_eq!(
        &{
            let image = images.0.lock().unwrap();
            image.pixels.render(image.layout, 0).unwrap()
        }[..4],
        &[77, 77, 85, 255]
    );
    images.use_sensor();
    assert_eq!(source.next_deadline(50), Some(50));
    super::liveview_support::advance(&mut source, &mut memory, 50);
    assert_eq!(images.0.lock().unwrap().edge, external);
    assert_eq!(source.next_deadline(50), None);
    frames.lock().unwrap().latch(55).unwrap();
    assert_eq!(source.next_deadline(54), Some(55));
    super::liveview_support::advance(&mut source, &mut memory, 54);
    assert_eq!(images.0.lock().unwrap().edge, external);
    super::liveview_support::advance(&mut source, &mut memory, 55);
    assert_eq!(
        images.0.lock().unwrap().edge,
        Some(FrameEdge { index: 7, tick: 55 })
    );
    assert_eq!(
        statistics.0.lock().unwrap().edge,
        images.0.lock().unwrap().edge
    );
}

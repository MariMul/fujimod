use super::*;
use std::sync::{Arc, Mutex};
use fujimod_emulator_devices::test_host::{TestHost, advance, write};
use fujimod_emulator_devices::imaging::{exposure::Calibration, radiometric::Renderer};
use fujimod_emulator_devices::scene::{Frame, FrameEdge, FramePixels, SensorFrames};

fn pair() -> ([OutputChannel; 2], ImageFrames, OutputChannels) {
    let frames = ImageFrames::new(ImageLayout {
        width: 8,
        height: 2,
        stride: 16,
    })
    .unwrap();
    let channels = OutputChannels::default();
    let routes = IrqRoutes::default();
    let outputs = std::array::from_fn(|channel| {
        let irq = 0x2c + channel as u32;
        routes.set(irq, Some(0)).unwrap();
        OutputChannel::new(
            Options {
                stop_ticks: 2,
                clock: dma::Clock::Fixed { ticks: 5 },
                geometry: GeometryPolicy::RefuseIncomplete,
                diagnostic_reads: false,
            },
            Wiring {
                base: 0xfff12000 + channel as u64 * 0x1000,
                irq,
                channel,
                channels: channels.clone(),
                routes: routes.clone(),
                frames: frames.clone(),
                memory_outputs: None,
            },
        )
        .unwrap()
    });
    (outputs, frames, channels)
}
fn arm(outputs: &mut [OutputChannel; 2], host: &mut TestHost) {
    for (channel, output) in outputs.iter_mut().enumerate() {
        let base = output.wiring.base;
        for (offset, value) in [
            (4, 0x10007),
            (0xb0, 0x10007),
            (0xb4, 7),
            (0xc0, 0x4000 + channel as u32 * 0x4000),
            (0xc4, 8),
            (0xd8, 1),
            (0, 0x10001),
        ] {
            write(output, host, base + offset, value);
        }
    }
}

#[test]
fn frozen_scene_jobs_keep_latched_bytes_pair_order_and_pending_restore() {
    fujimod_emulator_devices::work::configure(2).unwrap();
    let (mut outputs, images, channels) = pair();
    let mut host = TestHost::new();
    arm(&mut outputs, &mut host);
    let sensor = Arc::new(Mutex::new(SensorFrames::new(
        Frame::new(8, 2, FramePixels::Rgb24(vec![128; 48].into())).unwrap(),
    )));
    sensor.lock().unwrap().latch(10).unwrap();
    let mut renderer = Renderer::new(
        Calibration {
            full_scale: 16380,
            reference_white: [256; 3],
            saturation_lux_seconds: 1.,
            color_sample_weights: [1; 3],
        },
        sensor,
        Arc::new(Mutex::new(
            fujimod_emulator_devices::imaging::exposure::Exposure {
                seconds: 0.0001,
                analog_gain: 1.,
            },
        )),
        Arc::new(Mutex::new(8.)),
        Arc::new(Mutex::new(
            fujimod_emulator_devices::exr2::isp::parameters::Parameters {
                words: fujimod_emulator_devices::test_host::film_words(),
                film: Default::default(),
            },
        )),
        None,
    )
    .unwrap();
    let image = renderer.capture().unwrap().unwrap();
    // Derive the oracle from an independent restored image, leaving the queued
    // image's raster cache empty when it leaves the owner.
    let expected = fujimod_emulator_devices::imaging::radiometric::Image::from_checkpoint(
        &image.checkpoint().unwrap(),
    )
    .unwrap()
    .render(
        ImageLayout {
            width: 8,
            height: 2,
            stride: 16,
        },
        0,
    )
    .unwrap();
    {
        let mut frame = images.0.lock().unwrap();
        frame.edge = Some(FrameEdge { index: 1, tick: 10 });
        frame.pixels = fujimod_emulator_devices::scene::frames::Pixels::Sensor(image);
    }
    for output in &mut outputs {
        advance(output, &mut host, 10);
    }
    let identity = channels.fingerprint();
    let saved = channels.checkpoint().unwrap();
    let registers = outputs
        .each_ref()
        .map(|output| output.checkpoint().unwrap());
    assert_eq!(channels.fingerprint(), identity);
    assert!(host.effects.is_empty());
    assert!(
        outputs
            .iter()
            .all(|output| output.next_deadline(10) == Some(15))
    );
    // Neither a newer input nor a later register write can change this job.
    images
        .publish(FrameEdge { index: 2, tick: 11 }, &[99; 32])
        .unwrap();
    write(&mut outputs[0], &mut host, 0xfff120c0, 0xc000);
    advance(&mut outputs[0], &mut host, 14);
    assert!(host.effects.is_empty());
    advance(&mut outputs[0], &mut host, 15);
    assert_eq!(host.effects, ["dma", "dma", "irq", "irq"]);
    assert_eq!(host.irqs, [(0, 0x2c), (0, 0x2d)]);
    assert_eq!(&host.ram[0x4000..0x4020], expected.as_ref());
    assert_eq!(&host.ram[0x8000..0x8020], expected.as_ref());
    channels.restore_checkpoint(&saved).unwrap();
    for (output, state) in outputs.iter_mut().zip(registers) {
        output.restore_checkpoint(&state).unwrap();
    }
    let mut restored = TestHost::new();
    advance(&mut outputs[0], &mut restored, 15);
    assert_eq!(restored.ram, host.ram);
    assert_eq!(restored.effects, host.effects);
    assert_eq!(restored.irqs, host.irqs);
    fujimod_emulator_devices::work::configure(0).unwrap();
}

#[test]
fn failed_worker_result_survives_checkpoint_and_prevents_either_bank_dma() {
    fujimod_emulator_devices::work::configure(2).unwrap();
    let (mut outputs, images, channels) = pair();
    let mut host = TestHost::new();
    arm(&mut outputs, &mut host);
    images
        .publish(FrameEdge { index: 1, tick: 10 }, &[4; 32])
        .unwrap();
    for output in &mut outputs {
        advance(output, &mut host, 10);
    }
    channels.0.lock().unwrap().dma[1]
        .pixels
        .defer_fallible(|pixels| (pixels, Err("readout refused".into())));
    let saved = channels.checkpoint().unwrap();
    let fingerprint = channels.fingerprint();
    assert!(host.effects.is_empty());
    for restore in [false, true] {
        if restore {
            channels.restore_checkpoint(&saved).unwrap();
            assert_eq!(channels.fingerprint(), fingerprint);
        }
        advance(&mut outputs[0], &mut host, 14);
        host.now = 15;
        let mut cx = host.cx();
        outputs[0].advance(&mut cx);
        assert_eq!(cx.refusal(), Some("readout refused"));
        assert!(host.effects.is_empty());
        assert!(host.ram.iter().all(|&v| v == 0));
    }
    for output in &mut outputs {
        let base = output.wiring.base;
        write(output, &mut host, base, 0);
    }
    assert!(
        channels
            .0
            .lock()
            .unwrap()
            .dma
            .iter()
            .all(|t| t.check_result().is_ok())
    );
    fujimod_emulator_devices::work::configure(0).unwrap();
}

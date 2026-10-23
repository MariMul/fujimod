use fujimod_emulator_exr2_io::{adc, hdmi, usb};
use fujimod_emulator_exr2_audio::{self as audio};
use fujimod_emulator_exr2_system::{boot, core_release, dram, reset, timer};
use fujimod_emulator_devices::exr2::{clock, gpio, irq, isp, pulse};
use fujimod_emulator_exr2_companion::companion;
use fujimod_emulator_exr2_display::{formatter, timing};
use fujimod_emulator_exr2_serial::{dma, spi};
use fujimod_emulator_exr2_gpu::{self as gpu};
use fujimod_emulator_exr2_i2c::{self as i2c};
use fujimod_emulator_exr2_codecs::{jpeg, lz};
use fujimod_emulator_exr2_storage::{nand, sdhc, sdio};
use fujimod_emulator_devices::{Device, bus::InterruptLine, graph::Mapping};
use fujimod_emulator_parts_io::{audio_sink::AudioSink, usb_endpoint};
use fujimod_emulator_parts_controls::{encoder::Encoder, gpio_contact_response::GpioContactResponse};
use fujimod_emulator_parts_lens::{lens::{LensUnit, bridge::Bridge}, stabilizer::Stabilizer};
use fujimod_emulator_devices::parts::sd_card;
use fujimod_emulator_parts_io::wifi_module::WifiModule;
use fujimod_emulator_parts_sensor::sensor::Sensor;
use fujimod_emulator_storage::fat;
use fujimod_emulator_exr2_plan::{
    AdapterFactory, BuiltBoard, Card, CoreInterrupts, Model, Resources, SerialEndpoint, Wire,
};

macro_rules! wire {
    ($resources:expr, $id:expr, $name:expr, $kind:ident) => {
        match $resources.wire(&$name, $id)? {
            Wire::$kind(value) => value.clone(),
            _ => return Err(format!("{} requires a {} wire", $name, stringify!($kind))),
        }
    };
}
fn route(route: isp::Irq) -> Result<isp::Irq, String> {
    if route.core >= 2 || route.line >= 128 {
        return Err("EXR II interrupt route exceeds two cores/128 sources".into());
    }
    Ok(route)
}
fn line(line: InterruptLine) -> Result<InterruptLine, String> {
    if !matches!(
        line,
        InterruptLine::Core {
            core: 0..2,
            line: 0..128
        }
    ) {
        return Err("EXR II requires a per-core interrupt source".into());
    }
    Ok(line)
}
fn controllers(
    built: &BuiltBoard,
    names: Vec<String>,
) -> Result<Vec<(usize, irq::Inputs)>, String> {
    names
        .into_iter()
        .map(|name| {
            let node = built
                .node_ids
                .get(&name)
                .ok_or_else(|| format!("unknown interrupt controller {name}"))?;
            let controller = built
                .core_interrupts
                .iter()
                .find(|c| c.device == *node)
                .ok_or_else(|| format!("{name} is not an interrupt controller"))?;
            Ok((controller.core, controller.inputs.clone()))
        })
        .collect()
}

pub(super) struct Builder<'a> {
    pub name: &'a str,
    pub id: usize,
    pub windows: &'a [Mapping],
    pub resources: &'a mut Resources,
    pub built: &'a mut BuiltBoard,
    pub start_tick: u64,
    pub adapter: &'a mut dyn AdapterFactory,
}
impl Builder<'_> {
    pub(super) fn serial_devices(
        self,
        options: fujimod_emulator_exr2_plan::serial::Options,
        gpio: String,
    ) -> Result<Vec<(Option<Mapping>, Box<dyn Device>)>, String> {
        let Self {
            name,
            windows,
            resources,
            ..
        } = self;
        let physical = [
            (0xfff84000, 0x20),
            (0xfff46080, 0x10),
            (0xfff02080, 0x40),
            (0xfff98500, 8),
        ];
        if windows.len() != physical.len()
            || physical.iter().any(|&(base, size)| {
                !windows
                    .iter()
                    .any(|window| window.base == base && window.size == size)
            })
        {
            return Err(format!(
                "{name} requires its physical SPI0/UART/DMA/request-selector windows"
            ));
        }
        super::serial::BootSerialBlocks::new(
            options,
            super::serial::Wiring {
                gpio: resources.pads(&gpio)?,
            },
        )
        .map(super::serial::BootSerialBlocks::devices)
    }
    fn endpoint(
        endpoint: SerialEndpoint,
        resources: &mut Resources,
        id: usize,
    ) -> Result<Box<dyn fujimod_emulator_parts_io::spi::Endpoint>, String> {
        use fujimod_emulator_parts_io::spi::{FixedInput, Panel, RegisterFile};
        Ok(match endpoint {
            SerialEndpoint::LensUnit {
                options,
                pulses,
                gpio,
                completion,
                position,
                optics,
                focus,
            } => { let options: Box<fujimod_emulator_parts_lens::lens::Options> = crate::decode(options)?; 
                let mut lens = LensUnit::new(
                    *options,
                    fujimod_emulator_parts_lens::lens::Wiring {
                        pulses: resources.pulses(&pulses)?,
                        gpio: Box::new(resources.pads(&gpio)?),
                        completion: [line(completion[0])?, line(completion[1])?],
                        position: position
                            .map(|n| resources.optical_position(&n))
                            .transpose()?,
                    },
                )?;
                if let Some(optics) = optics {
                    lens.attach_aperture(
                        wire!(resources, id, optics.output, Aperture),
                        optics.positions,
                    )?;
                }
                if let Some(focus) = focus {
                    lens.attach_focus(
                        wire!(resources, id, focus.output, Focus),
                        focus.infinity_position,
                        focus.steps_per_mm,
                    )?;
                }
                Box::new(lens)
            }
            SerialEndpoint::Stabilizer { options } => { let options: fujimod_emulator_parts_lens::stabilizer::Options = crate::decode(options)?; Box::new(Stabilizer::new(options, ())?) },
            SerialEndpoint::Registers { options } => { let options: fujimod_emulator_parts_io::spi::RegisterOptions = crate::decode(options)?; Box::new(RegisterFile::new(options)?) },
            SerialEndpoint::Panel { options } => { let options: fujimod_emulator_parts_io::spi::PanelOptions = crate::decode(options)?; Box::new(Panel::new(options)) },
            SerialEndpoint::CommandDataOutput { options } => { let options: fujimod_emulator_parts_io::command_data_output::Options = crate::decode(options)?; Box::new(
                fujimod_emulator_parts_io::command_data_output::CommandDataOutput::new(
                    options,
                )?,
            ) },
            SerialEndpoint::Fixed { options } => { let options: fujimod_emulator_parts_io::spi::FixedOptions = crate::decode(options)?; Box::new(FixedInput::new(options)?) },
        })
    }
    pub fn device(self, model: Model) -> Result<Box<dyn Device>, String> {
        let Self {
            name,
            id,
            windows,
            resources,
            built,
            start_tick,
            adapter,
        } = self;
        let non_mmio = matches!(
            model,
            Model::Encoder { .. }
                | Model::ContactActuator { .. }
                | Model::GpioContactResponse { .. }
                | Model::I2cMotorBridge { .. }
                | Model::CoreRelease { .. }
                | Model::ExternalFrames { .. }
                | Model::HdmiSink { .. }
        );
        if !matches!(model, Model::Adapter { .. }) && non_mmio != windows.is_empty() {
            return Err(format!("{name} has incorrect MMIO/non-MMIO ownership"));
        }
        Ok(match model {
            Model::ContactActuator { options, gpio } => { let options: fujimod_emulator_parts_controls::contact_actuator::Options = crate::decode(options)?; 
                if !windows.is_empty() || options.contacts.iter().any(|pin| pin.port >= 16) {
                    return Err(
                        "contact actuator requires GPIO input pads and no register aperture".into(),
                    );
                }
                Box::new(fujimod_emulator_parts_controls::contact_actuator::ContactActuator::new(
                    options,
                    fujimod_emulator_parts_controls::contact_actuator::Wiring {
                        pins: Box::new(resources.pads(&gpio)?),
                    },
                )?)
            }
            Model::Power { .. } => return Err("boot serial blocks expand through BootSerialBlocks".into()),
            Model::Lz {
                options,
                base,
                controllers: names,
            } => { let options: fujimod_emulator_exr2_codecs::lz::Options = crate::decode(options)?; 
                if windows.len() != 1 || windows[0].base != base || windows[0].size != 0x100 {
                    return Err("decompressor requires its complete register window".into());
                }
                Box::new(lz::Decompressor::new(
                    options,
                    lz::Wiring {
                        base,
                        controllers: controllers(built, names)?,
                    },
                )?)
            }
            Model::MemoryDma {
                options,
                base,
                routes,
            } => { let options: fujimod_emulator_exr2_serial::dma::memory::Options = crate::decode(options)?; 
                if base.checked_add(0x304).is_none()
                    || windows.len() != 2
                    || windows[0].base != base
                    || windows[0].size != 0x40
                    || windows[1].base != base + 0x300
                    || windows[1].size != 4
                {
                    return Err("memory DMA requires channel 0 and its global word".into());
                }
                Box::new(dma::memory::MemoryDma::new(
                    options,
                    dma::memory::Wiring {
                        base,
                        routes: wire!(resources, id, routes, IrqRoutes),
                    },
                )?)
            }
            Model::SdhciClocks { words } => {
                if words.len() != windows.len()
                    || words.iter().any(|&(address, _)| {
                        !matches!(address, 0xfff5002c | 0xfff5102c)
                            || !windows.iter().any(|w| w.base == address && w.size == 2)
                    })
                {
                    return Err("SD clock-only nodes require their physical clock words".into());
                }
                Box::new(fujimod_emulator_ip::sdhci::Clocks::new(&words)?)
            }
            Model::Usb {
                options,
                cable,
                endpoints,
            } => { let options: fujimod_emulator_exr2_io::usb::Options = crate::decode(options)?; let endpoints: Option<fujimod_emulator_parts_io::usb_endpoint::Wiring> = crate::decode(endpoints)?; 
                if windows.len() != 2
                    || windows[0].base != 0xfff54000
                    || windows[0].size != 0x1000
                    || windows[1].base != 0xfff55000
                    || windows[1].size != 0x1000
                {
                    return Err("USB requires its physical core and PHY windows".into());
                }
                if let Some(wiring) = &endpoints {
                    line(wiring.irq)?;
                    line(wiring.dma_irq)?;
                    if wiring.irq == wiring.dma_irq {
                        return Err("USB core and DMA IRQ sources must differ".into());
                    }
                }
                Box::new(usb::UsbController::new(
                    options,
                    usb::Wiring {
                        cable: wire!(resources, id, cable, LogicLevel),
                        endpoints,
                    },
                )?)
            }
            Model::Scanout { options } => { let options: fujimod_emulator_exr2_display::device::Options = crate::decode(options)?; 
                let expected = [0xfff6b000, 0xfff6e000];
                if windows.len() != 2
                    || windows
                        .iter()
                        .zip(expected)
                        .any(|(w, b)| w.base != b || w.size != 0x1000)
                {
                    return Err("scanout requires both complete physical register windows".into());
                }
                Box::new(fujimod_emulator_exr2_display::Scanout::new(options)?)
            }
            Model::SerialAudioReceiver { options, routes } => { let options: fujimod_emulator_exr2_audio::receive::Options = crate::decode(options)?; 
                let expected = [
                    (dma::audio::BASE, 0x40),
                    (audio::BASES[0], 0x1000),
                    (audio::BASES[1], 0x1000),
                ];
                if windows.len() != expected.len()
                    || windows
                        .iter()
                        .zip(expected)
                        .any(|(w, (base, size))| w.base != base || w.size != size)
                {
                    return Err(
                        "audio receiver requires its DMA channel and both audio banks".into(),
                    );
                }
                Box::new(audio::receive::Receiver::new(
                    options,
                    wire!(resources, id, routes, IrqRoutes),
                )?)
            }
            Model::SerialAudioTransmitter {
                options,
                sinks,
                routes,
            } => { let options: fujimod_emulator_exr2_audio::transmit::Options = crate::decode(options)?; let sinks: [fujimod_emulator_parts_io::audio_sink::Options; 2] = crate::decode(sinks)?; 
                let expected = [
                    (dma::audio::BASE, 0x40),
                    (audio::BASES[0], 0x1000),
                    (audio::BASES[1], 0x1000),
                ];
                if windows.len() != expected.len()
                    || windows
                        .iter()
                        .zip(expected)
                        .any(|(w, (base, size))| w.base != base || w.size != size)
                {
                    return Err(
                        "audio transmitter requires its DMA channel and both audio banks".into(),
                    );
                }
                Box::new(audio::transmit::Transmitter::new(
                    options,
                    sinks.map(AudioSink::new),
                    wire!(resources, id, routes, IrqRoutes),
                )?)
            }
            Model::SerialDmaLink { .. } => {
                return Err("serial/DMA link expands through serial_dma::Nodes".into());
            }
            Model::XMountLens {
                transport,
                configuration,
                startup_ticks,
                pins,
                gpio,
                controllers: names,
            } => { let transport: fujimod_emulator_exr2_serial::serial_dma::Options = crate::decode(transport)?; let configuration: fujimod_emulator_parts_lens::x_mount_link::Configuration = crate::decode(configuration)?; let pins: fujimod_emulator_exr2_serial::x_mount_connector::Options = crate::decode(pins)?; 
                let device = fujimod_emulator_exr2_serial::x_mount_connector::XMountLens::new(
                    transport,
                    configuration,
                    startup_ticks,
                    pins,
                    resources.pads(&gpio)?,
                    controllers(built, names)?,
                )?;
                let expected: std::collections::BTreeSet<_> =
                    device.windows().iter().map(|&(a, b)| (a, b - a)).collect();
                if expected != windows.iter().map(|w| (w.base, w.size)).collect() {
                    return Err(
                        "X-mount lens requires exactly its serial/DMA register windows".into(),
                    );
                }
                Box::new(device)
            }
            Model::SerialDma { options } => { let options: fujimod_emulator_exr2_serial::dma::audio::Options = crate::decode(options)?; 
                if windows.len() != 1
                    || windows[0].base != dma::audio::BASE
                    || windows[0].size != 0x40
                {
                    return Err("serial DMA requires its channel register window".into());
                }
                Box::new(dma::audio::SerialDma::new(options)?)
            }
            Model::ImageCombination { options, irq } => {
                if windows.len() != 1
                    || windows[0].base != fujimod_emulator_devices::exr2::isp::options::combination::BASE
                    || windows[0].size != 0x500
                {
                    return Err("image combination requires its register window".into());
                }
                Box::new(fujimod_emulator_exr2_isp::combination::Combination::new(options, route(irq)?)?)
            }
            Model::I2c {
                options,
                controllers: names,
                cable,
                bridge,
            } => { let options: fujimod_emulator_exr2_i2c::Options = crate::decode(options)?; 
                if windows.len() != 1
                    || windows[0].base != u64::from(options.base)
                    || windows[0].size != 0x40
                {
                    return Err("I2C requires its full physical channel window".into());
                }
                let cable = cable
                    .map(|name| Ok(wire!(resources, id, name, LogicLevel)))
                    .transpose()?;
                let bridge = bridge
                    .map(|name| Ok(wire!(resources, id, name, BridgeRegisters)))
                    .transpose()?;
                Box::new(i2c::I2cController::new(
                    options,
                    i2c::Wiring {
                        controllers: controllers(built, names)?,
                        cable,
                        bridge,
                    },
                )?)
            }
            Model::VectorGpu {
                options,
                controllers: names,
            } => { let options: fujimod_emulator_exr2_gpu::Options = crate::decode(options)?; 
                if windows.len() != 1
                    || windows[0].base != gpu::BASE
                    || windows[0].size != gpu::SIZE
                {
                    return Err(
                        "vector engine requires its complete physical register window".into(),
                    );
                }
                Box::new(gpu::VectorGpu::new(
                    options,
                    gpu::Wiring {
                        controllers: controllers(built, names)?,
                    },
                )?)
            }
            Model::Nand { options, medium } => { let options: fujimod_emulator_exr2_storage::nand::Options = crate::decode(options)?; Box::new(nand::NandController::new(
                options,
                resources
                    .inputs
                    .nands
                    .remove(&medium)
                    .ok_or_else(|| format!("missing NAND input {medium}"))?,
            )?) },
            Model::ExternalFrames {
                options,
                sensor_clock,
                images,
                statistics,
                activity,
                position,
                radiometric,
                phase_edges,
                frame_period,
            } => Box::new(fujimod_emulator_devices::scene::clocked::ClockedFrames::new(
                options,
                fujimod_emulator_devices::scene::clocked::Wiring {
                    sensor_clock: sensor_clock
                        .map(|name| {
                            resources
                                .inputs
                                .sensors
                                .get(&name)
                                .cloned()
                                .ok_or_else(|| format!("missing physical sensor clock {name}"))
                        })
                        .transpose()?,
                    frame_period: frame_period
                        .map(|name| Ok::<_, String>(wire!(resources, id, name, FramePeriod)))
                        .transpose()?,
                    phase_edges: phase_edges
                        .map(|name| Ok::<_, String>(wire!(resources, id, name, PhaseEdges)))
                        .transpose()?,
                    position: position
                        .map(|n| resources.optical_position(&n))
                        .transpose()?,
                    renderer: radiometric
                        .map(|r| {
                            fujimod_emulator_devices::imaging::radiometric::Renderer::new(
                                r.calibration,
                                resources
                                    .inputs
                                    .sensors
                                    .get(&r.frames)
                                    .cloned()
                                    .ok_or("missing radiometric scene")?,
                                wire!(resources, id, r.exposure, Exposure),
                                wire!(resources, id, r.aperture, Aperture),
                                wire!(resources, id, r.parameters, Parameters),
                                r.projection
                                    .map(|p| {
                                        let focus = wire!(resources, id, p.focus.clone(), Focus);
                                        fujimod_emulator_devices::imaging::optics::Projection::new(p, focus)
                                    })
                                    .transpose()?,
                            )
                        })
                        .transpose()?,
                    images: resources.image_input(&images, id)?,
                    statistics: resources
                        .inputs
                        .statistics
                        .get(&statistics)
                        .cloned()
                        .ok_or("missing external statistics input")?,
                    activity: activity
                        .map(|signal| {
                            Ok::<_, String>((
                                Box::new(resources.pads(&signal.gpio)?)
                                    as Box<dyn fujimod_emulator_devices::parts::pins::InputPins>,
                                signal.selection,
                            ))
                        })
                        .transpose()?,
                    start_tick,
                },
            )?),
            Model::DisplayTiming {
                options,
                clock,
                controllers,
            } => { let options: fujimod_emulator_exr2_display::timing::Options = crate::decode(options)?; 
                let clock = resources.clock(&clock)?;
                let controllers = self::controllers(built, controllers)?;
                Box::new(timing::DisplayTiming::new(
                    options,
                    timing::Wiring {
                        clock,
                        controllers,
                        start_tick,
                    },
                )?)
            }
            Model::Dram { options } => { let options: fujimod_emulator_exr2_system::dram::Options = crate::decode(options)?; Box::new(dram::Controller::new(options, dram::Wiring)?) },
            Model::DramPhy { options } => { let options: fujimod_emulator_exr2_system::dram::Options = crate::decode(options)?; Box::new(dram::Phy::new(options, dram::Wiring)?) },
            Model::Reset { options } => { let options: fujimod_emulator_exr2_system::reset::Options = crate::decode(options)?; Box::new(reset::Reset::new(options, reset::Wiring)?) },
            Model::Timers {
                options,
                controllers,
            } => { let options: fujimod_emulator_exr2_system::timer::Options = crate::decode(options)?; 
                let controllers = self::controllers(built, controllers)?;
                Box::new(timer::Timers::new(
                    options,
                    timer::Wiring {
                        controllers,
                        start_tick,
                    },
                )?)
            }
            Model::CoreRelease {
                options,
                core,
                request,
            } => { let options: fujimod_emulator_exr2_system::core_release::Options = crate::decode(options)?; Box::new(core_release::CoreRelease::new(
                options,
                core_release::Wiring {
                    core,
                    request: wire!(resources, id, request, SoftwarePulse),
                },
            )?) },
            Model::Block98000 { options, wiring } => { let options: fujimod_emulator_exr2_system::boot::Options = crate::decode(options)?; let wiring: fujimod_emulator_exr2_system::boot::Wiring = crate::decode(wiring)?; 
                Box::new(boot::Block98000::new(options, wiring)?)
            }
            Model::Clocks { options } => {
                let clock = clock::Clocks::new(options, ())?;
                resources.clocks.insert(name.into(), clock.output());
                Box::new(clock)
            }
            Model::InterruptController {
                options,
                core,
                base,
                routes,
                software,
            } => {
                if core >= 2 || built.core_interrupts.iter().any(|c| c.core == core) {
                    return Err("invalid or duplicate EXR II core interrupt controller".into());
                }
                let enables = routes
                    .map(|name| {
                        let routes = wire!(resources, id, name, IrqRoutes);
                        Ok(Box::new(isp::dma::RouteOutput::new(routes, core)?)
                            as Box<dyn irq::EnableOutput>)
                    })
                    .transpose()?;
                let inputs = irq::Inputs::default();
                resources.controllers.insert(name.into(), inputs.clone());
                let controller = irq::InterruptController::new(
                    options,
                    irq::Wiring {
                        base,
                        inputs: inputs.clone(),
                        enables,
                        software: software
                            .into_iter()
                            .map(|output| {
                                if !resources.software_sources.insert(output.pulse.clone()) {
                                    return Err(
                                        "software pulse requires one controller output".into()
                                    );
                                }
                                Ok(irq::SoftwareOutput {
                                    line: output.line,
                                    pulse: wire!(resources, id, output.pulse, SoftwarePulse),
                                })
                            })
                            .collect::<Result<_, String>>()?,
                    },
                )?;
                built.core_interrupts.push(CoreInterrupts {
                    core,
                    device: id,
                    inputs,
                });
                Box::new(controller)
            }
            Model::Adc { options } => { let options: fujimod_emulator_exr2_io::adc::controller::Options = crate::decode(options)?; Box::new(adc::controller::Converter::new(options)?) },
            Model::Gpio { options, irqs } => {
                for route in &irqs {
                    for &output in &route.outputs {
                        line(output)?;
                    }
                }
                let gpio = gpio::Gpio::new(options, gpio::Wiring { irqs })?;
                resources.gpio.insert(name.to_owned(), gpio.pads());
                Box::new(gpio)
            }
            Model::PulseUnit { options, irqs } => {
                for irq in irqs.into_iter().flatten() {
                    line(irq)?;
                }
                let pulse = pulse::PulseUnit::new(options, pulse::Wiring { irqs })?;
                resources.pulses.insert(name.to_owned(), pulse.pins());
                Box::new(pulse)
            }
            Model::SerialChannel {
                options,
                gpio,
                endpoint,
            } => { let options: fujimod_emulator_exr2_serial::spi::channel::Options = crate::decode(options)?; 
                let endpoint = Self::endpoint(endpoint, resources, id)?;
                Box::new(spi::channel::SerialChannel::new(
                    options,
                    spi::channel::Wiring {
                        pins: Box::new(resources.pads(&gpio)?),
                        endpoint,
                    },
                )?)
            }
            Model::SpiController { routes } => {
                let channels = routes
                    .into_iter()
                    .map(|route| {
                        let endpoint = Self::endpoint(route.endpoint, resources, id)?;
                        spi::channel::SerialChannel::new(
                            crate::decode(route.options)?,
                            spi::channel::Wiring {
                                pins: Box::new(resources.pads(&route.gpio)?),
                                endpoint,
                            },
                        )
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Box::new(spi::SpiController::new(channels)?)
            }
            Model::GpioContactResponse { options, gpio } => { let options: fujimod_emulator_parts_controls::gpio_contact_response::Options = crate::decode(options)?; Box::new(GpioContactResponse::new(
                options,
                fujimod_emulator_parts_controls::gpio_contact_response::Wiring {
                    pins: Box::new(resources.pads(&gpio)?),
                },
            )?) },
            Model::Encoder {
                options,
                gpio,
                counter,
            } => { let options: fujimod_emulator_parts_controls::encoder::Options = crate::decode(options)?; 
                let encoder = Encoder::new(
                    options,
                    fujimod_emulator_parts_controls::encoder::Wiring {
                        gpio: Box::new(resources.pads(&gpio)?),
                        counter: counter.map(|n| resources.pulses(&n)).transpose()?,
                    },
                )?;
                resources.encoders.insert(name.to_owned(), encoder.input());
                Box::new(encoder)
            }
            Model::I2cMotorBridge {
                options,
                registers,
                pulses,
                gpio,
                clock_irq,
            } => { let options: fujimod_emulator_parts_lens::lens::bridge::Options = crate::decode(options)?; Box::new(Bridge::new(
                options,
                fujimod_emulator_parts_lens::lens::bridge::Wiring {
                    registers: wire!(resources, id, registers, BridgeRegisters),
                    pulses: resources.pulses(&pulses)?,
                    gpio: resources.pads(&gpio)?,
                    clock_irq: line(clock_irq)?,
                },
            )?) },
            Model::Block86000 { options } => Box::new(pulse::block86000::Block86000::new(options)),
            Model::Companion { options } => { let options: fujimod_emulator_exr2_companion::companion::Options = crate::decode(options)?; Box::new(companion::ProgramUpload::new(options)?) },
            Model::Fe80Processing {
                transport,
                options,
                frames,
                routes,
            } => { let transport: fujimod_emulator_exr2_companion::companion::Options = crate::decode(transport)?; let options: fujimod_emulator_exr2_companion::companion_processing::Options = crate::decode(options)?; Box::new(fujimod_emulator_exr2_companion::companion_processing::Processing::new(
                transport,
                options,
                fujimod_emulator_exr2_companion::companion_processing::Wiring {
                    frames: resources
                        .inputs
                        .images
                        .get(&frames)
                        .cloned()
                        .ok_or_else(|| format!("missing FE80 frame clock {frames}"))?,
                    routes: wire!(resources, id, routes, IrqRoutes),
                },
            )?) },
            Model::Formatter { options } => { let options: fujimod_emulator_exr2_display::formatter::Options = crate::decode(options)?; Box::new(formatter::Formatter::new(options)?) },
            Model::HdmiSink { options } => { let options: fujimod_emulator_devices::parts::hdmi_sink::Options = crate::decode(options)?; 
                let input = fujimod_emulator_devices::parts::hdmi_sink::Input::new(options)?;
                resources.hdmi_sinks.insert(name.to_owned(), input.sink());
                Box::new(input)
            }
            Model::Hdmi {
                options,
                sink,
                hot_plug,
            } => { let options: fujimod_emulator_exr2_io::hdmi::transmitter::Options = crate::decode(options)?; Box::new(hdmi::Transmitter::new(
                options,
                resources.hdmi_sink(&sink)?,
                hdmi::transmitter::Wiring {
                    hot_plug: line(hot_plug)?,
                },
            )?) },
            Model::SerialAudio { options, sinks } => { let options: fujimod_emulator_exr2_audio::Options = crate::decode(options)?; let sinks: [fujimod_emulator_parts_io::audio_sink::Options; 2] = crate::decode(sinks)?; 
                Box::new(audio::SerialAudio::new(options, sinks.map(AudioSink::new)))
            }
            Model::SdHost {
                options,
                card,
                pins,
                present,
                protected,
                irq,
            } => { let options: fujimod_emulator_exr2_storage::sdhc::Options = crate::decode(options)?; 
                let card = match card {
                    Card::Blank { options } => { let options: fujimod_emulator_devices::parts::sd_card::Options = crate::decode(options)?; sd_card::Card::new(options)? },
                    Card::Formatted { options, files } => { let options: fujimod_emulator_storage::fat::Options = crate::decode(options)?; fat::format(
                        &options,
                        resources
                            .inputs
                            .files
                            .get(&files)
                            .ok_or_else(|| format!("missing card files input {files}"))?,
                    )? },
                    Card::Retained { input } => resources
                        .inputs
                        .cards
                        .remove(&input)
                        .ok_or_else(|| format!("missing retained medium {input}"))?,
                };
                if let Some(pins) = pins {
                    pins.detect.validate()?;
                    pins.protect.validate()?;
                    if pins.detect == pins.protect {
                        return Err("SD detect and protect must use distinct pads".into());
                    }
                    let pads = resources.pads(&pins.gpio)?;
                    if pads.input_level(pins.detect)? != (present == pins.present_high)
                        || pads.input_level(pins.protect)? != (protected == pins.protected_high)
                    {
                        return Err(
                            "SD contact reset levels differ from the connected GPIO pads".into(),
                        );
                    }
                    resources.card_pins.insert(
                        name.to_owned(),
                        fujimod_emulator_exr2_plan::CardPins {
                            gpio: *built
                                .node_ids
                                .get(&pins.gpio)
                                .ok_or("SD GPIO has not been constructed")?,
                            pads,
                            detect: pins.detect,
                            present_high: pins.present_high,
                            protect: pins.protect,
                            protected_high: pins.protected_high,
                        },
                    );
                }
                let contacts = fujimod_emulator_devices::parts::card_contacts::Contacts::new(present, protected);
                let host = sdhc::SdHost::new(
                    options,
                    sdhc::Wiring {
                        card,
                        contacts: contacts.clone(),
                        irq: line(irq)?,
                    },
                )?;
                built.contacts.insert(name.to_owned(), contacts);
                Box::new(host)
            }
            Model::SdioHost {
                options,
                module,
                irq,
            } => { let options: fujimod_emulator_exr2_storage::sdio::Options = crate::decode(options)?; let module: fujimod_emulator_parts_io::wifi_module::Options = crate::decode(module)?; Box::new(sdio::SdioHost::new(
                options,
                sdio::Wiring {
                    module: WifiModule::new(module)?,
                    irq: line(irq)?,
                },
            )?) },
            Model::UsbEndpoints {
                options,
                irq,
                dma_irq,
                ram_bytes,
            } => { let options: fujimod_emulator_parts_io::usb_endpoint::EndpointOptions = crate::decode(options)?; Box::new(usb_endpoint::Endpoint::new(
                options,
                usb_endpoint::Wiring {
                    irq: line(irq)?,
                    dma_irq: line(dma_irq)?,
                    ram_bytes,
                },
            )?) },
            Model::ImageInput {
                options,
                phase_irqs,
                raw_irq,
                readback_irq,
                shutter_output,
                sensor,
                parameters,
                memory_outputs,
                statistics_grid,
                frame_period,
                edges,
            } => {
                let sensor = sensor
                    .map(|s| {
                        let mut sensor = Sensor::new(
                            crate::decode(s.options)?,
                            resources
                                .inputs
                                .sensors
                                .get(&s.frames)
                                .cloned()
                                .ok_or_else(|| format!("missing sensor frames {}", s.frames))?,
                        )?;
                        if let Some(fujimod_emulator_exr2_plan::SensorControls::Xtrans16 {
                            options,
                            calibration,
                            exposure,
                            aperture,
                            projection,
                            frame_clock,
                        }) = s.controls
                        {
                            let exposure = wire!(resources, id, exposure, Exposure);
                            let aperture = wire!(resources, id, aperture, Aperture);
                            let mut controls = fujimod_emulator_parts_sensor::xtrans16::Sensor::new(
                                crate::decode(options)?,
                                exposure.clone(),
                                aperture.clone(),
                            )?;
                            if let Some(clock) = frame_clock {
                                controls.attach_frame_clock(
                                    wire!(resources, id, clock.output, FramePeriod),
                                    clock.ticks_per_second,
                                )?;
                            }
                            sensor.attach_controls(
                                Box::new(controls),
                                calibration,
                                exposure,
                                aperture,
                            )?;
                            if let Some(p) = projection {
                                let focus = wire!(resources, id, p.focus.clone(), Focus);
                                sensor.attach_projection(fujimod_emulator_devices::imaging::optics::Projection::new(
                                    p, focus,
                                )?);
                            }
                        }
                        Ok::<_, String>(sensor)
                    })
                    .transpose()?;
                let parameters = parameters
                    .map(|name| Ok(wire!(resources, id, name, Parameters)))
                    .transpose()?;
                let memory_outputs = memory_outputs
                    .map(|name| Ok(wire!(resources, id, name, MemoryOutputs)))
                    .transpose()?;
                Box::new(fujimod_emulator_exr2_isp::ImageInput::new(
                    options,
                    fujimod_emulator_exr2_isp::input::Wiring {
                        frame_period: frame_period
                            .map(|name| Ok(wire!(resources, id, name, FramePeriod)))
                            .transpose()?,
                        statistics_grid: statistics_grid
                            .map(|name| Ok(wire!(resources, id, name, ImageGrid)))
                            .transpose()?,
                        phase_irqs: [route(phase_irqs[0])?, route(phase_irqs[1])?],
                        raw_irq: route(raw_irq)?,
                        readback_irq: route(readback_irq)?,
                        shutter_output,
                        sensor,
                        parameters,
                        memory_outputs,
                        edges: wire!(resources, id, edges, PhaseEdges),
                        start_tick,
                    },
                )?)
            }
            Model::OutputChannel {
                options,
                base,
                irq,
                channel,
                channels,
                routes,
                frames,
                memory_outputs,
            } => {
                let memory_outputs = memory_outputs
                    .map(|name| Ok(wire!(resources, id, name, MemoryOutputs)))
                    .transpose()?;
                Box::new(fujimod_emulator_exr2_isp::OutputChannel::new(
                    options,
                    fujimod_emulator_exr2_isp::output::Wiring {
                        base,
                        irq,
                        channel,
                        channels: wire!(resources, id, channels, OutputChannels),
                        routes: wire!(resources, id, routes, IrqRoutes),
                        frames: resources
                            .inputs
                            .images
                            .get(&frames)
                            .cloned()
                            .ok_or_else(|| format!("missing image frames {frames}"))?,
                        memory_outputs,
                    },
                )?)
            }
            Model::Statistics {
                options,
                routes,
                frames,
                images,
                grid,
            } => Box::new(fujimod_emulator_exr2_isp::Block18000::new(
                options,
                fujimod_emulator_exr2_isp::producer::Wiring {
                    grid: grid
                        .map(|name| Ok(wire!(resources, id, name, ImageGrid)))
                        .transpose()?,
                    routes: wire!(resources, id, routes, IrqRoutes),
                    images: resources
                        .inputs
                        .images
                        .get(&images)
                        .cloned()
                        .ok_or_else(|| format!("missing histogram image frames {images}"))?,
                    frames: resources
                        .inputs
                        .statistics
                        .get(&frames)
                        .cloned()
                        .ok_or_else(|| format!("missing statistics frames {frames}"))?,
                },
            )?),
            Model::PhaseDetection { options, routes } => Box::new(fujimod_emulator_exr2_isp::PhaseDetection::new(
                options,
                fujimod_emulator_exr2_isp::phase_detection::Wiring {
                    routes: wire!(resources, id, routes, IrqRoutes),
                },
            )?),
            Model::WindowAnalysis { options, routes } => Box::new(fujimod_emulator_exr2_isp::WindowAnalysis::new(
                options,
                fujimod_emulator_exr2_isp::window_analysis::Wiring {
                    routes: wire!(resources, id, routes, IrqRoutes),
                },
            )?),
            Model::Parameters { options, bank } => Box::new(isp::ParameterBank::new(
                options,
                wire!(resources, id, bank, Parameters),
            )?),
            Model::RawConversion { options } => Box::new(fujimod_emulator_exr2_isp::RawConversion::new(options, ())?),
            Model::Prefilter { options, irq } => {
                Box::new(fujimod_emulator_exr2_isp::Prefilter::new(options, route(irq)?)?)
            }
            Model::RawStatistics { options, irq } => {
                Box::new(fujimod_emulator_exr2_isp::RawStatistics::new(options, route(irq)?)?)
            }
            Model::StillProcessor { options, irq } => {
                Box::new(fujimod_emulator_exr2_isp::StillProcessor::new(options, route(irq)?)?)
            }
            Model::Scaler { options, irq } => Box::new(fujimod_emulator_exr2_isp::Scaler::new(options, route(irq)?)?),
            Model::Warp {
                options,
                irq,
                stream,
            } => Box::new(fujimod_emulator_exr2_isp::Warp::new(
                options,
                (route(irq)?, wire!(resources, id, stream, Stream)),
            )?),
            Model::VideoEncoder { options, irq } => {
                Box::new(fujimod_emulator_exr2_isp::VideoEncoder::new(options, route(irq)?)?)
            }
            Model::JpegEncoder {
                options,
                irq,
                stream,
            } => { let options: fujimod_emulator_exr2_codecs::jpeg::encoder::Options = crate::decode(options)?; Box::new(jpeg::Encoder::new(
                options,
                (route(irq)?, wire!(resources, id, stream, Stream)),
            )?) },
            Model::JpegDecoder { options, irq } => { let options: fujimod_emulator_exr2_codecs::jpeg::decoder::Options = crate::decode(options)?; 
                Box::new(jpeg::Decoder::new(options, route(irq)?)?)
            }
            Model::Block11000 { options } => Box::new(fujimod_emulator_exr2_isp::Block11000::new(options)?),
            Model::FaceDetector { options } => Box::new(fujimod_emulator_exr2_isp::FaceDetector::new(options)),
            Model::FaceResults { profile, routes } => {
                Box::new(fujimod_emulator_exr2_isp::face_results::FaceResults::new(
                    profile,
                    wire!(resources, id, routes, IrqRoutes),
                ))
            }
            Model::Adapter { adapter: spec } => {
                for name in &spec.wires {
                    resources.wire(name, id)?;
                }
                adapter.construct(name, &spec, windows, resources, &built.node_ids)?
            }
        })
    }
}

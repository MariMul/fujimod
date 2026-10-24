//! Constructors of the EXR II devices the interface crates still hold.
use super::{Builder, Built, Constructors, decode, line};
use crate::{CoreInterrupts, Model};
use fujimod_emulator_devices::exr2::{clock, gpio, irq, pulse};
use fujimod_emulator_imaging::exr2_isp as isp;

/// Add the constructors of the devices the interface crates hold.
pub fn register_interface_models(constructors: &mut Constructors) {
    constructors.add("clocks", clocks);
    constructors.add("exr2-interrupt-controller", interrupt_controller);
    constructors.add("gpio", gpio);
    constructors.add("exr2-pulse", pulse_unit);
    constructors.add("block86000", block86000);
    constructors.add("hdmi-sink", hdmi_sink);
    constructors.add("external-frames", external_frames);
    constructors.add("parameters", parameters);
}

fn clocks(builder: Builder<'_>, model: Model) -> Result<Built, String> {
    let Builder {
        name, resources, ..
    } = builder;
    let Model::Clocks { options } = model else {
        return Err("constructor called for another model kind".into());
    };
    Ok(Built::Device({
        let clock = clock::Clocks::new(options, ())?;
        resources.clocks.insert(name.into(), clock.output());
        Box::new(clock)
    }))
}

fn interrupt_controller(builder: Builder<'_>, model: Model) -> Result<Built, String> {
    let Builder {
        name,
        id,
        resources,
        built,
        ..
    } = builder;
    let Model::InterruptController {
        options,
        core,
        base,
        routes,
        software,
    } = model
    else {
        return Err("constructor called for another model kind".into());
    };
    Ok(Built::Device({
        if core >= 2 || built.core_interrupts.iter().any(|c| c.core == core) {
            return Err("invalid or duplicate EXR II core interrupt controller".into());
        }
        let enables = routes
            .map(|name| {
                let routes = crate::wire!(resources, id, name, IrqRoutes);
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
                            return Err("software pulse requires one controller output".into());
                        }
                        Ok(irq::SoftwareOutput {
                            line: output.line,
                            pulse: crate::wire!(resources, id, output.pulse, SoftwarePulse),
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
    }))
}

fn gpio(builder: Builder<'_>, model: Model) -> Result<Built, String> {
    let Builder {
        name, resources, ..
    } = builder;
    let Model::Gpio { options, irqs } = model else {
        return Err("constructor called for another model kind".into());
    };
    Ok(Built::Device({
        for route in &irqs {
            for &output in &route.outputs {
                line(output)?;
            }
        }
        let gpio = gpio::Gpio::new(options, gpio::Wiring { irqs })?;
        resources.gpio.insert(name.to_owned(), gpio.pads());
        Box::new(gpio)
    }))
}

fn pulse_unit(builder: Builder<'_>, model: Model) -> Result<Built, String> {
    let Builder {
        name, resources, ..
    } = builder;
    let Model::PulseUnit { options, irqs } = model else {
        return Err("constructor called for another model kind".into());
    };
    Ok(Built::Device({
        for irq in irqs.into_iter().flatten() {
            line(irq)?;
        }
        let pulse = pulse::PulseUnit::new(options, pulse::Wiring { irqs })?;
        resources.pulses.insert(name.to_owned(), pulse.pins());
        Box::new(pulse)
    }))
}

fn block86000(_builder: Builder<'_>, model: Model) -> Result<Built, String> {
    let Model::Block86000 { options } = model else {
        return Err("constructor called for another model kind".into());
    };
    Ok(Built::Device(Box::new(pulse::block86000::Block86000::new(
        options,
    ))))
}

fn hdmi_sink(builder: Builder<'_>, model: Model) -> Result<Built, String> {
    let Builder {
        name, resources, ..
    } = builder;
    let Model::HdmiSink { options } = model else {
        return Err("constructor called for another model kind".into());
    };
    Ok(Built::Device({
        let options: fujimod_emulator_devices::parts::hdmi_sink::Options = decode(options)?;
        let input = fujimod_emulator_devices::parts::hdmi_sink::Input::new(options)?;
        resources.hdmi_sinks.insert(name.to_owned(), input.sink());
        Box::new(input)
    }))
}

fn external_frames(builder: Builder<'_>, model: Model) -> Result<Built, String> {
    let Builder {
        id,
        resources,
        start_tick,
        ..
    } = builder;
    let Model::ExternalFrames {
        options,
        sensor_clock,
        images,
        statistics,
        activity,
        position,
        radiometric,
        phase_edges,
        frame_period,
    } = model
    else {
        return Err("constructor called for another model kind".into());
    };
    Ok(Built::Device(Box::new(
        fujimod_emulator_imaging::scene::clocked::ClockedFrames::new(
            options,
            fujimod_emulator_imaging::scene::clocked::Wiring {
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
                    .map(|name| Ok::<_, String>(crate::wire!(resources, id, name, FramePeriod)))
                    .transpose()?,
                phase_edges: phase_edges
                    .map(|name| Ok::<_, String>(crate::wire!(resources, id, name, PhaseEdges)))
                    .transpose()?,
                position: position
                    .map(|n| resources.optical_position(&n))
                    .transpose()?,
                renderer: radiometric
                    .map(|r| {
                        fujimod_emulator_imaging::radiometric::Renderer::new(
                            r.calibration,
                            resources
                                .inputs
                                .sensors
                                .get(&r.frames)
                                .cloned()
                                .ok_or("missing radiometric scene")?,
                            crate::wire!(resources, id, r.exposure, Exposure),
                            crate::wire!(resources, id, r.aperture, Aperture),
                            crate::wire!(resources, id, r.parameters, Parameters),
                            r.projection
                                .map(|p| {
                                    let focus = crate::wire!(resources, id, p.focus.clone(), Focus);
                                    fujimod_emulator_imaging::optics::Projection::new(p, focus)
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
        )?,
    )))
}

fn parameters(builder: Builder<'_>, model: Model) -> Result<Built, String> {
    let Builder { id, resources, .. } = builder;
    let Model::Parameters { options, bank } = model else {
        return Err("constructor called for another model kind".into());
    };
    Ok(Built::Device(Box::new(isp::ParameterBank::new(
        options,
        crate::wire!(resources, id, bank, Parameters),
    )?)))
}

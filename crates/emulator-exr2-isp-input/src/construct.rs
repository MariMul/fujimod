//! EXR II board constructors for this crate's models.
use fujimod_emulator_exr2_plan::{Model, construction::{Builder, Built, Constructors, decode, route}};
use fujimod_emulator_exr2_resources::wire;
use fujimod_emulator_parts_sensor::sensor::Sensor;

/// Add this crate's EXR II constructors.
pub fn register(constructors: &mut Constructors) {
    constructors.add("image-input", image_input);
    constructors.add("raw-conversion", raw_conversion);
    constructors.add("prefilter", prefilter);
    constructors.add("raw-statistics", raw_statistics);
    constructors.add("block11000", block11000);
    constructors.add("external-frames", external_frames);
    constructors.add("parameters", parameters);
    fujimod_emulator_imaging::scene::frames::install_sensor_decoders(
        fujimod_emulator_image_processing::sensor_decoders(),
    );
}

fn image_input(builder: Builder<'_>, model: Model) -> Result<Built, String> {
    let Builder {
        id,
        resources,
        start_tick,
        ..
    } = builder;
    let Model::ImageInput {
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
    } = model
    else {
        return Err("constructor called for another model kind".into());
    };
    Ok(Built::Device({
        let sensor = sensor
            .map(|s| {
                let mut sensor = Sensor::new(
                    decode(s.options)?,
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
                        decode(options)?,
                        exposure.clone(),
                        aperture.clone(),
                    )?;
                    if let Some(clock) = frame_clock {
                        controls.attach_frame_clock(
                            wire!(resources, id, clock.output, FramePeriod),
                            clock.ticks_per_second,
                        )?;
                    }
                    sensor.attach_controls(Box::new(controls), calibration, exposure, aperture)?;
                    if let Some(p) = projection {
                        let focus = wire!(resources, id, p.focus.clone(), Focus);
                        sensor.attach_projection(
                            fujimod_emulator_image_formation::optics::Projection::new(p, focus)?,
                        );
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
        Box::new(crate::input::ImageInput::new(
            options,
            crate::input::Wiring {
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
    }))
}

fn raw_conversion(_builder: Builder<'_>, model: Model) -> Result<Built, String> {
    let Model::RawConversion { options } = model else {
        return Err("constructor called for another model kind".into());
    };
    Ok(Built::Device(Box::new(crate::raw::RawConversion::new(
        options,
        (),
    )?)))
}

fn prefilter(_builder: Builder<'_>, model: Model) -> Result<Built, String> {
    let Model::Prefilter { options, irq } = model else {
        return Err("constructor called for another model kind".into());
    };
    Ok(Built::Device({
        Box::new(crate::raw::Prefilter::new(options, route(irq)?)?)
    }))
}

fn raw_statistics(_builder: Builder<'_>, model: Model) -> Result<Built, String> {
    let Model::RawStatistics { options, irq } = model else {
        return Err("constructor called for another model kind".into());
    };
    Ok(Built::Device({
        Box::new(crate::raw::RawStatistics::new(options, route(irq)?)?)
    }))
}

fn block11000(_builder: Builder<'_>, model: Model) -> Result<Built, String> {
    let Model::Block11000 { options } = model else {
        return Err("constructor called for another model kind".into());
    };
    Ok(Built::Device(Box::new(crate::block11000::Block11000::new(
        options,
    )?)))
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
        fujimod_emulator_image_processing::scene::clocked::ClockedFrames::new(
            options,
            fujimod_emulator_image_processing::scene::clocked::Wiring {
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
                        fujimod_emulator_image_formation::radiometric::Renderer::new(
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
                                    fujimod_emulator_image_formation::optics::Projection::new(p, focus)
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
    Ok(Built::Device(Box::new(fujimod_emulator_image_processing::parameters::ParameterBank::new(
        options,
        wire!(resources, id, bank, Parameters),
    )?)))
}

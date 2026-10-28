//! Optional host render path for one DMP command list.

use super::{
    Reader, Snapshots,
    draw_state::{DrawState, PrimitiveMode},
    gpu_surfaces::{GpuSurface, SurfaceCache, SurfaceExtent, SurfaceFormat, SurfaceKey},
    raster::{self, RasterVertex},
    shader::Vector,
};
use std::collections::{BTreeMap, HashMap};
use wgpu::util::DeviceExt;

const REGISTER_WORDS: usize = 0x500;
const FOG_OFFSET: usize = REGISTER_WORDS;
const FOG_WORDS: usize = 128;
const INPUT_OFFSET: usize = FOG_OFFSET + FOG_WORDS;
const CASE_WORDS: usize = INPUT_OFFSET + 16;

#[derive(Clone, Debug)]
pub(super) struct Primitive {
    state: DrawState,
    vertices: [[Vector; 16]; 3],
    registers: Vec<(u16, u32)>,
}

impl Primitive {
    pub(super) fn new(
        state: DrawState,
        vertices: [[Vector; 16]; 3],
        registers: &BTreeMap<u16, u32>,
    ) -> Self {
        Self {
            state,
            vertices,
            registers: registers
                .iter()
                .map(|(&index, &value)| (index, value))
                .collect(),
        }
    }
}

pub(super) struct RenderedList {
    pub snapshots: Snapshots,
    pub written_ranges: Vec<(u64, usize)>,
}

pub(super) fn render(
    primitives: &[Primitive],
    recorded_reads: &[(u64, Vec<u8>)],
    expected_snapshots: &Snapshots,
    read: &mut Reader<'_>,
) -> Result<RenderedList, String> {
    let mut geometry = Vec::new();
    for primitive in primitives {
        if !matches!(
            primitive.state.raster.primitive_mode,
            PrimitiveMode::Triangles | PrimitiveMode::TriangleStrip | PrimitiveMode::TriangleFan
        ) {
            return Err("DMP host GPU draw received a non-triangle primitive".into());
        }
        let triangles = raster::clipped_triangles(&primitive.state, primitive.vertices)?;
        if !triangles.is_empty() {
            geometry.push((primitive, triangles));
        }
    }
    if geometry.is_empty() {
        return Ok(RenderedList {
            snapshots: Vec::new(),
            written_ranges: Vec::new(),
        });
    }

    let mut cache = SurfaceCache::new()?;
    let mut surfaces = BTreeMap::<u64, SurfaceKey>::new();
    for (primitive, _) in &geometry {
        let state = &primitive.state;
        let (width, height) = state.targets.extent.dimensions()?;
        let extent = SurfaceExtent {
            width: width as u32,
            height: height as u32,
        };
        let color_key = SurfaceKey {
            guest_address: state.targets.color_address,
            format: SurfaceFormat::from_color_code(state.targets.color_format.code())?,
            extent,
        };
        upload_surface(
            &mut cache,
            &mut surfaces,
            color_key,
            recorded_reads,
            expected_snapshots,
            read,
        )?;

        let depth_key = SurfaceKey {
            guest_address: state.targets.depth_address,
            format: SurfaceFormat::from_depth_code(state.targets.depth_format.code())?,
            extent,
        };
        upload_surface(
            &mut cache,
            &mut surfaces,
            depth_key,
            recorded_reads,
            expected_snapshots,
            read,
        )?;
    }

    let shader_source = format!(
        "{}\n{}",
        include_str!("gpu_fragment.wgsl"),
        include_str!("gpu_draw.wgsl")
    );
    let shader = cache
        .device()
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DMP WGSL render stage"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });
    let mut pipelines = HashMap::<DrawState, wgpu::RenderPipeline>::new();

    for (primitive, triangles) in &geometry {
        for triangle in triangles {
            render_triangle(
                primitive,
                triangle,
                &shader,
                &mut pipelines,
                &mut cache,
                recorded_reads,
                read,
            )?;
        }
    }

    let mut snapshots = Vec::new();
    let mut written_ranges = Vec::new();
    for (&address, &key) in &surfaces {
        let readback = cache.readback(key)?;
        if expected_snapshots
            .iter()
            .any(|(expected, _)| *expected == address)
            || !readback.written_ranges.is_empty()
        {
            snapshots.push((address, readback.bytes));
            written_ranges.extend(readback.written_ranges);
        }
    }
    written_ranges.sort_unstable_by_key(|(address, _)| *address);
    Ok(RenderedList {
        snapshots,
        written_ranges,
    })
}

pub(super) fn snapshot_mismatch(actual: &Snapshots, expected: &Snapshots) -> String {
    let count = actual.len().min(expected.len());
    for index in 0..count {
        let (actual_address, actual_bytes) = &actual[index];
        let (expected_address, expected_bytes) = &expected[index];
        if actual_address != expected_address || actual_bytes != expected_bytes {
            let byte = actual_bytes
                .iter()
                .zip(expected_bytes)
                .position(|(actual, expected)| actual != expected);
            return format!(
                "DMP host GPU snapshot differs from CPU at GPU {actual_address:#x}, CPU {expected_address:#x}, byte {byte:?} (lengths {} and {})",
                actual_bytes.len(),
                expected_bytes.len(),
            );
        }
    }
    format!(
        "DMP host GPU returned {} snapshots, CPU returned {}",
        actual.len(),
        expected.len()
    )
}

fn upload_surface(
    cache: &mut SurfaceCache,
    surfaces: &mut BTreeMap<u64, SurfaceKey>,
    key: SurfaceKey,
    recorded_reads: &[(u64, Vec<u8>)],
    expected_snapshots: &Snapshots,
    read: &mut Reader<'_>,
) -> Result<(), String> {
    if let Some(existing) = surfaces.get(&key.guest_address) {
        if *existing != key {
            return Err(format!(
                "DMP host GPU target {:#x} changes format or extent within one list",
                key.guest_address
            ));
        }
        return Ok(());
    }
    let byte_len = (key.extent.width as usize)
        .checked_mul(key.extent.height as usize)
        .and_then(|pixels| pixels.checked_mul(key.format.bytes_per_pixel() as usize))
        .ok_or("DMP host GPU surface byte length overflow")?;
    let bytes = if let Some(bytes) = recorded_range(recorded_reads, key.guest_address, byte_len) {
        bytes
    } else if expected_snapshots
        .iter()
        .any(|(address, _)| *address == key.guest_address)
    {
        read(key.guest_address, byte_len)?
    } else {
        vec![0; byte_len]
    };
    if bytes.len() != byte_len {
        return Err(format!(
            "DMP host GPU target {:#x} supplied {} bytes, expected {byte_len}",
            key.guest_address,
            bytes.len()
        ));
    }
    cache.get_or_upload(key, &bytes)?;
    surfaces.insert(key.guest_address, key);
    Ok(())
}

fn recorded_range(reads: &[(u64, Vec<u8>)], address: u64, length: usize) -> Option<Vec<u8>> {
    let end = address.checked_add(length as u64)?;
    reads.iter().find_map(|(base, bytes)| {
        let read_end = base.checked_add(bytes.len() as u64)?;
        if address < *base || end > read_end {
            return None;
        }
        let start = usize::try_from(address - base).ok()?;
        Some(bytes[start..start + length].to_vec())
    })
}

fn render_triangle(
    primitive: &Primitive,
    triangle: &[RasterVertex; 3],
    shader: &wgpu::ShaderModule,
    pipelines: &mut HashMap<DrawState, wgpu::RenderPipeline>,
    cache: &mut SurfaceCache,
    recorded_reads: &[(u64, Vec<u8>)],
    read: &mut Reader<'_>,
) -> Result<(), String> {
    let state = &primitive.state;
    let extent = SurfaceExtent {
        width: state.targets.extent.width as u32,
        height: state.targets.extent.height as u32,
    };
    let color_key = SurfaceKey {
        guest_address: state.targets.color_address,
        format: SurfaceFormat::from_color_code(state.targets.color_format.code())?,
        extent,
    };
    let depth_key = SurfaceKey {
        guest_address: state.targets.depth_address,
        format: SurfaceFormat::from_depth_code(state.targets.depth_format.code())?,
        extent,
    };
    if color_key.guest_address == depth_key.guest_address {
        return Err("DMP host GPU color and depth targets alias".into());
    }
    let color_surface = cache
        .get(color_key)
        .ok_or("DMP color surface is not resident")?;
    let depth_surface = cache
        .get(depth_key)
        .ok_or("DMP depth surface is not resident")?;

    let device = cache.device();
    let pipeline = pipelines
        .entry(state.clone())
        .or_insert_with(|| create_pipeline(device, shader));
    let (parameters, guest_bytes) = draw_parameters(primitive, recorded_reads, read)?;
    let parameter_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("DMP draw parameters"),
        contents: &encode_words(&parameters),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let guest_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("DMP draw textures"),
        contents: &guest_bytes,
        usage: wgpu::BufferUsages::STORAGE,
    });
    let vertex_data = encode_vertices(triangle, extent)?;
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("DMP clipped triangle vertices"),
        contents: &vertex_data,
        usage: wgpu::BufferUsages::VERTEX,
    });
    let color_input = copy_surface_texture(device, cache.queue(), color_surface, extent);
    let depth_input = copy_surface_texture(device, cache.queue(), depth_surface, extent);
    let color_input_view = color_input.create_view(&Default::default());
    let depth_input_view = depth_input.create_view(&Default::default());
    let color_view = color_surface.texture().create_view(&Default::default());
    let depth_view = depth_surface.texture().create_view(&Default::default());
    let color_mask_view = color_surface
        .write_mask_texture()
        .create_view(&Default::default());
    let depth_mask_view = depth_surface
        .write_mask_texture()
        .create_view(&Default::default());
    let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("DMP render bindings"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: parameter_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: guest_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&color_input_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&depth_input_view),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("DMP triangle render"),
    });
    {
        let attachments = [
            Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &depth_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &color_mask_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &depth_mask_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            }),
        ];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("DMP clipped triangle"),
            color_attachments: &attachments,
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bindings, &[]);
        pass.set_viewport(
            0.0,
            0.0,
            extent.width as f32,
            extent.height as f32,
            0.0,
            1.0,
        );
        pass.set_scissor_rect(0, 0, extent.width, extent.height);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..3, 0..1);
    }
    cache.queue().submit([encoder.finish()]);
    cache.accumulate_draw_mask(color_key)?;
    cache.accumulate_draw_mask(depth_key)?;
    Ok(())
}

fn create_pipeline(device: &wgpu::Device, shader: &wgpu::ShaderModule) -> wgpu::RenderPipeline {
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x4,
        1 => Float32x4,
        2 => Float32x2,
        3 => Float32x2
    ];
    let buffers = [wgpu::VertexBufferLayout {
        array_stride: 48,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBUTES,
    }];
    let targets = [
        Some(color_target()),
        Some(color_target()),
        Some(color_target()),
        Some(color_target()),
    ];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("DMP fixed-function draw state"),
        layout: None,
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("draw_vertex"),
            compilation_options: Default::default(),
            buffers: &[Some(buffers[0].clone())],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Cw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("draw_fragment"),
            compilation_options: Default::default(),
            targets: &targets,
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn color_target() -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format: wgpu::TextureFormat::R32Uint,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    }
}

fn copy_surface_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface: &GpuSurface,
    extent: SurfaceExtent,
) -> wgpu::Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("DMP draw destination snapshot"),
        size: wgpu::Extent3d {
            width: extent.width,
            height: extent.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Uint,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("DMP destination snapshot copy"),
    });
    encoder.copy_texture_to_texture(
        surface.texture().as_image_copy(),
        texture.as_image_copy(),
        wgpu::Extent3d {
            width: extent.width,
            height: extent.height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    texture
}

fn draw_parameters(
    primitive: &Primitive,
    recorded_reads: &[(u64, Vec<u8>)],
    read: &mut Reader<'_>,
) -> Result<(Vec<u32>, Vec<u8>), String> {
    let mut registers = vec![0u32; REGISTER_WORDS];
    for &(index, value) in &primitive.registers {
        registers[index as usize] = value;
    }
    let guest = texture_bytes(&primitive.state, &mut registers, recorded_reads, read)?;
    let mut parameters = vec![0u32; CASE_WORDS];
    parameters[..REGISTER_WORDS].copy_from_slice(&registers);
    if primitive.state.fog.table.len() != FOG_WORDS {
        return Err("DMP draw fog table extent is invalid".into());
    }
    parameters[FOG_OFFSET..INPUT_OFFSET].copy_from_slice(&primitive.state.fog.table);
    Ok((parameters, guest))
}

fn texture_bytes(
    state: &DrawState,
    registers: &mut [u32],
    recorded_reads: &[(u64, Vec<u8>)],
    read: &mut Reader<'_>,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    for (unit, texture) in state.textures.iter().enumerate() {
        if !texture.enabled {
            continue;
        }
        let bytes_per_pixel = texture.format.bytes_per_pixel()?;
        if texture.width == 0
            || texture.height == 0
            || !texture.width.is_multiple_of(8)
            || !texture.height.is_multiple_of(8)
        {
            return Err("DMP draw texture extent is invalid".into());
        }
        let (base_register, mip_register) = if unit == 0 {
            (0x285, 0x289)
        } else {
            (0x295, 0x299)
        };
        let base_offset = align_four(bytes.len());
        bytes.resize(base_offset, 0);
        registers[base_register] = u32::try_from(base_offset)
            .map_err(|_| "DMP draw texture buffer address exceeds u32")?;
        append_mip_level(
            &mut bytes,
            texture.base_address,
            texture.width,
            texture.height,
            bytes_per_pixel,
            recorded_reads,
            read,
        )?;

        let mip_offset = align_four(bytes.len());
        bytes.resize(mip_offset, 0);
        registers[mip_register] =
            u32::try_from(mip_offset).map_err(|_| "DMP draw mip buffer address exceeds u32")?;
        let mut maximum_level = 0u8;
        for level in 1..=texture.max_level {
            let width = texture.width >> level;
            let height = texture.height >> level;
            if width == 0 || height == 0 || !width.is_multiple_of(8) || !height.is_multiple_of(8) {
                break;
            }
            let mut address_offset = 0usize;
            for previous in 1..level {
                address_offset +=
                    (texture.width >> previous) * (texture.height >> previous) * bytes_per_pixel;
            }
            append_mip_level(
                &mut bytes,
                texture.mip_address + address_offset as u64,
                width,
                height,
                bytes_per_pixel,
                recorded_reads,
                read,
            )?;
            maximum_level = level;
        }
        let levels_register = if unit == 0 { 0x284 } else { 0x294 };
        registers[levels_register] =
            (registers[levels_register] & !0x000f_0000) | (u32::from(maximum_level) << 16);
    }
    if bytes.is_empty() {
        bytes.extend_from_slice(&[0; 4]);
    } else {
        bytes.resize(align_four(bytes.len()), 0);
    }
    Ok(bytes)
}

fn append_mip_level(
    bytes: &mut Vec<u8>,
    address: u64,
    width: usize,
    height: usize,
    bytes_per_pixel: usize,
    recorded_reads: &[(u64, Vec<u8>)],
    read: &mut Reader<'_>,
) -> Result<(), String> {
    let size = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(bytes_per_pixel))
        .ok_or("DMP draw texture size overflow")?;
    let level = recorded_range(recorded_reads, address, size)
        .map(Ok)
        .unwrap_or_else(|| read(address, size))?;
    if level.len() != size {
        return Err(format!(
            "DMP draw texture at {address:#x} has {} bytes, expected {size}",
            level.len()
        ));
    }
    bytes.extend_from_slice(&level);
    Ok(())
}

fn align_four(length: usize) -> usize {
    (length + 3) & !3
}

fn encode_vertices(triangle: &[RasterVertex; 3], extent: SurfaceExtent) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(3 * 48);
    for vertex in triangle {
        let reciprocal_w = vertex.position[3];
        if !reciprocal_w.is_finite() || reciprocal_w == 0.0 {
            return Err("DMP clipped vertex has invalid reciprocal W".into());
        }
        let w = 1.0 / reciprocal_w;
        let ndc_x = vertex.position[0] * (2.0 / extent.width as f32) - 1.0;
        let ndc_y = 1.0 - vertex.position[1] * (2.0 / extent.height as f32);
        let clip = [ndc_x * w, ndc_y * w, vertex.position[2] * w, w];
        for value in clip
            .into_iter()
            .chain(vertex.color)
            .chain(vertex.uv[0])
            .chain(vertex.uv[1])
        {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(bytes)
}

fn encode_words(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

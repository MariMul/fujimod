fn process_fragment(case_index: u32) {
    let x = input_word(case_index, 0u);
    let y = input_word(case_index, 1u);
    let z = bitcast<f32>(input_word(case_index, 2u));
    let coverage = bitcast<f32>(input_word(case_index, 3u));
    let primary = vec4<f32>(
        bitcast<f32>(input_word(case_index, 4u)),
        bitcast<f32>(input_word(case_index, 5u)),
        bitcast<f32>(input_word(case_index, 6u)),
        bitcast<f32>(input_word(case_index, 7u)),
    );
    let uv0 = vec2<f32>(bitcast<f32>(input_word(case_index, 8u)), bitcast<f32>(input_word(case_index, 9u)));
    let uv1 = vec2<f32>(bitcast<f32>(input_word(case_index, 10u)), bitcast<f32>(input_word(case_index, 11u)));
    let lod0 = bitcast<f32>(input_word(case_index, 12u));
    let lod1 = bitcast<f32>(input_word(case_index, 13u));

    let dimensions = reg(case_index, 0x31eu);
    let width = dimensions & 8191u;
    let height = (dimensions >> 16u) & 8191u;
    let origin = reg(case_index, 0x266u);
    let end = reg(case_index, 0x267u);
    if x < (origin & 65535u) || y < (origin >> 16u)
        || x > (end & 65535u) || y > (end >> 16u) {
        return;
    }

    var texture0 = vec4<f32>(0.0);
    var texture1 = vec4<f32>(0.0);
    let texture_enable = reg(case_index, 0x280u);
    if (texture_enable & 1u) != 0u {
        let format = reg(case_index, 0x28eu) & 15u;
        if format > 5u && format != 7u && format != 8u {
            status.words[case_index] = 0x10000u | format;
            return;
        }
        let uv = vec2<f32>(bitcast<f32>(input_word(case_index, 8u)), bitcast<f32>(input_word(case_index, 9u)));
        texture0 = sample_texture(case_index, 0u, uv, lod0);
    }
    if (texture_enable & 2u) != 0u {
        let format = reg(case_index, 0x296u) & 15u;
        if format > 5u && format != 7u && format != 8u {
            status.words[case_index] = 0x10000u | format;
            return;
        }
        let uv = vec2<f32>(bitcast<f32>(input_word(case_index, 10u)), bitcast<f32>(input_word(case_index, 11u)));
        texture1 = sample_texture(case_index, 1u, uv, lod1);
    }
    var color = combine(case_index, primary, texture0, texture1);
    color.w *= floor(coverage * 255.0) / 255.0;

    let alpha = reg(case_index, 0x304u);
    if (alpha & 1u) != 0u {
        let value = u32(clamp(color.w, 0.0, 1.0) * 255.0);
        if !comparison(alpha >> 4u, value, (alpha >> 8u) & 255u) {
            return;
        }
    }

    let fog = reg(case_index, 0x2e0u);
    if (fog & 1u) != 0u {
        var factor = 0.0;
        if (fog & 4u) != 0u {
            let position = min(clamp(z, 0.0, 1.0) * 128.0, 127.9999);
            let word = param(case_index, FOG_OFFSET + u32(position));
            let slope = signed13(word);
            factor = clamp(
                f32((word >> 13u) & 2047u) / 2048.0
                    + f32(slope) / 2048.0 * fract(position),
                0.0,
                1.0,
            );
        } else {
            let scale = f32(signed16(reg(case_index, 0x2e2u))) / 2048.0;
            let bias = f32(signed16(reg(case_index, 0x2e3u))) / 2048.0;
            factor = clamp(z * scale + bias, 0.0, 1.0);
        }
        let fog_color = reg(case_index, 0x2e1u);
        for (var channel = 0u; channel < 3u; channel += 1u) {
            let fog_component = f32((fog_color >> (channel * 8u)) & 255u) / 255.0;
            color[channel] = color[channel] * factor + fog_component * (1.0 - factor);
        }
    }

    let pixel = morton_index(x, y, width);
    let depth_control = reg(case_index, 0x307u);
    let stencil = reg(case_index, 0x305u);
    let operations = reg(case_index, 0x306u);
    if (stencil & 1u) != 0u || (depth_control & 1u) != 0u {
        let format = reg(case_index, 0x316u);
        var bytes_per_pixel = 0u;
        var maximum = 0x00ffffffu;
        if format == 0u {
            bytes_per_pixel = 2u;
            maximum = 65535u;
        } else if format == 2u {
            bytes_per_pixel = 3u;
        } else {
            bytes_per_pixel = 4u;
        }
        let depth_address = reg(case_index, 0x31cu) + pixel * bytes_per_pixel;
        var old = memory_u24(depth_address);
        if bytes_per_pixel == 2u {
            old = memory_u16(depth_address);
        } else if bytes_per_pixel == 4u {
            old = memory_u32(depth_address);
        }
        let old_stencil = (old >> 24u) & 255u;
        let reference = (stencil >> 16u) & 255u;
        let value_mask = (stencil >> 24u) & 255u;
        let stencil_pass = (stencil & 1u) == 0u || comparison(
            stencil >> 4u,
            reference & value_mask,
            old_stencil & value_mask,
        );
        let depth = u32(clamp(z, 0.0, 1.0) * f32(maximum));
        let depth_pass = (depth_control & 1u) == 0u
            || comparison(depth_control >> 4u, depth, old & maximum);
        if (stencil & 1u) != 0u && bytes_per_pixel == 4u {
            var operation = (operations >> 8u) & 7u;
            if !stencil_pass {
                operation = operations & 7u;
            } else if !depth_pass {
                operation = (operations >> 4u) & 7u;
            }
            let write_mask = (stencil >> 8u) & 255u;
            let updated = (old_stencil & ~write_mask)
                | (stencil_result(operation, old_stencil, reference) & write_mask);
            write_memory_byte(depth_address + 3u, updated);
        }
        if !stencil_pass || !depth_pass {
            return;
        }
        if (depth_control & 0x1001u) == 0x1001u {
            if bytes_per_pixel == 2u {
                write_u16(depth_address, depth);
            } else {
                write_u24(depth_address, depth);
            }
        }
    }

    if ((depth_control >> 8u) & 15u) == 0u {
        return;
    }
    let format = (reg(case_index, 0x317u) >> 16u) & 15u;
    var bytes_per_pixel = 2u;
    if format == 0u {
        bytes_per_pixel = 4u;
    }
    let color_address = reg(case_index, 0x31du) + pixel * bytes_per_pixel;
    var target_format = 4u;
    if format == 0u {
        target_format = 0u;
    } else if format == 1u {
        target_format = 3u;
    } else if format == 2u {
        target_format = 2u;
    }
    let destination = unpack_texel(color_address, target_format);
    let control = reg(case_index, 0x301u);
    let constant_word = reg(case_index, 0x303u);
    let constant = vec4<f32>(
        f32(constant_word & 255u),
        f32((constant_word >> 8u) & 255u),
        f32((constant_word >> 16u) & 255u),
        f32(constant_word >> 24u),
    ) / 255.0;
    if (reg(case_index, 0x300u) & 0x100u) != 0u {
        for (var channel = 0u; channel < 4u; channel += 1u) {
            let shift = select(16u, 24u, channel == 3u);
            let source_factor = (control >> shift) & 15u;
            let destination_factor = (control >> (shift + 4u)) & 15u;
            let equation = select(control & 255u, (control >> 8u) & 255u, channel == 3u);
            if source_factor > 14u || destination_factor > 14u {
                status.words[case_index] = 1u;
                return;
            }
            if equation > 4u {
                status.words[case_index] = 2u;
                return;
            }
        }
        let source = color;
        for (var channel = 0u; channel < 4u; channel += 1u) {
            let shift = select(16u, 24u, channel == 3u);
            let a = source[channel]
                * blend_factor((control >> shift) & 15u, channel, source, destination, constant);
            let b = destination[channel]
                * blend_factor((control >> (shift + 4u)) & 15u, channel, source, destination, constant);
            let equation = select(control & 255u, (control >> 8u) & 255u, channel == 3u);
            if equation == 0u {
                color[channel] = a + b;
            } else if equation == 1u {
                color[channel] = a - b;
            } else if equation == 2u {
                color[channel] = b - a;
            } else if equation == 3u {
                color[channel] = min(color[channel], b);
            } else {
                color[channel] = max(color[channel], b);
            }
        }
    } else if reg(case_index, 0x302u) != 3u {
        status.words[case_index] = 3u;
        return;
    }

    let mask = (depth_control >> 8u) & 15u;
    for (var channel = 0u; channel < 4u; channel += 1u) {
        if (mask & (1u << channel)) == 0u {
            color[channel] = destination[channel];
        }
    }
    if format == 0u {
        for (var channel = 0u; channel < 4u; channel += 1u) {
            if (mask & (1u << channel)) != 0u {
                let value = u32(floor(clamp(color[channel], 0.0, 1.0) * 255.0 + 0.5));
                write_memory_byte(color_address + 3u - channel, value);
            }
        }
    } else {
        let packed = clamp(color, vec4<f32>(0.0), vec4<f32>(1.0));
        var word = 0u;
        if format == 1u {
            word = (u32(packed.x * 31.0) << 11u)
                | (u32(packed.y * 63.0) << 5u)
                | u32(packed.z * 31.0);
        } else if format == 2u {
            word = (u32(packed.x * 31.0) << 11u)
                | (u32(packed.y * 31.0) << 6u)
                | (u32(packed.z * 31.0) << 1u)
                | u32(packed.w);
        } else {
            word = (u32(packed.x * 15.0) << 12u)
                | (u32(packed.y * 15.0) << 8u)
                | (u32(packed.z * 15.0) << 4u)
                | u32(packed.w * 15.0);
        }
        var bits = 0u;
        for (var channel = 0u; channel < 4u; channel += 1u) {
            if (mask & (1u << channel)) != 0u {
                if format == 1u {
                    if channel == 0u { bits |= 0xf800u; }
                    if channel == 1u { bits |= 0x07e0u; }
                    if channel == 2u { bits |= 0x001fu; }
                } else if format == 2u {
                    if channel == 0u { bits |= 0xf800u; }
                    if channel == 1u { bits |= 0x07c0u; }
                    if channel == 2u { bits |= 0x003eu; }
                    if channel == 3u { bits |= 0x0001u; }
                } else {
                    if channel == 0u { bits |= 0xf000u; }
                    if channel == 1u { bits |= 0x0f00u; }
                    if channel == 2u { bits |= 0x00f0u; }
                    if channel == 3u { bits |= 0x000fu; }
                }
            }
        }
        let previous = memory_u16(color_address);
        write_u16(color_address, (previous & ~bits) | (word & bits));
    }
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let case_count = arrayLength(&params.words) / CASE_WORDS;
    if id.x >= case_count {
        return;
    }
    process_fragment(id.x);
}

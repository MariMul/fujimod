struct DrawFragmentInput {
    @builtin(position) position: vec4<f32>,
    @location(0) primary: vec4<f32>,
    @location(1) texcoords01: vec4<f32>,
    @location(2) texcoord2: vec2<f32>,
    @location(3) @interpolate(linear) depth: f32,
    @location(6) @interpolate(flat) valid: u32,
};

fn lod_from_derivatives(uv: vec2<f32>, dimensions: vec2<f32>) -> f32 {
    let dx = dpdx(uv) * dimensions;
    let dy = dpdy(uv) * dimensions;
    let rho = max(dot(dx, dx), dot(dy, dy));
    if rho > 0.0 {
        return 0.5 * log2(rho);
    }
    return -128.0;
}

fn fragment_color(input: DrawFragmentInput) -> vec4<f32> {
    if input.valid == 0u {
        discard;
    }
    let x = u32(input.position.x);
    let y = u32(input.position.y);
    let z = input.depth;
    let origin = reg(0u, 0x266u);
    let end = reg(0u, 0x267u);
    if x < (origin & 65535u) || y < (origin >> 16u)
        || x > (end & 65535u) || y > (end >> 16u) {
        discard;
    }

    var texture0 = vec4<f32>(0.0);
    var texture1 = vec4<f32>(0.0);
    let texture_enable = reg(0u, 0x280u);
    if (texture_enable & 1u) != 0u {
        let tex_dimensions = reg(0u, 0x282u);
        let size = vec2<f32>(f32(tex_dimensions >> 16u), f32(tex_dimensions & 65535u));
        texture0 = sample_texture(
            0u,
            0u,
            input.texcoords01.xy,
            lod_from_derivatives(input.texcoords01.xy, size),
        );
    }
    if (texture_enable & 2u) != 0u {
        let tex_dimensions = reg(0u, 0x292u);
        let size = vec2<f32>(f32(tex_dimensions >> 16u), f32(tex_dimensions & 65535u));
        texture1 = sample_texture(
            0u,
            1u,
            input.texcoords01.zw,
            lod_from_derivatives(input.texcoords01.zw, size),
        );
    }

    var color = combine(0u, input.primary, texture0, texture1);
    let alpha = reg(0u, 0x304u);
    if (alpha & 1u) != 0u {
        let value = u32(clamp(color.w, 0.0, 1.0) * 255.0);
        if !comparison(alpha >> 4u, value, (alpha >> 8u) & 255u) {
            discard;
        }
    }

    let fog = reg(0u, 0x2e0u);
    if (fog & 1u) != 0u {
        var factor = 0.0;
        if (fog & 4u) != 0u {
            let position = min(clamp(z, 0.0, 1.0) * 128.0, 127.9999);
            let word = param(0u, FOG_OFFSET + u32(position));
            factor = clamp(
                f32((word >> 13u) & 2047u) / 2048.0
                    + f32(signed13(word)) / 2048.0 * fract(position),
                0.0,
                1.0,
            );
        } else {
            let scale = f32(signed16(reg(0u, 0x2e2u))) / 2048.0;
            let bias = f32(signed16(reg(0u, 0x2e3u))) / 2048.0;
            factor = clamp(z * scale + bias, 0.0, 1.0);
        }
        let fog_color = reg(0u, 0x2e1u);
        for (var channel = 0u; channel < 3u; channel += 1u) {
            let component = f32((fog_color >> (channel * 8u)) & 255u) / 255.0;
            color[channel] = color[channel] * factor + component * (1.0 - factor);
        }
    }

    return color;
}


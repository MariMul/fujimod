@group(0) @binding(8) var<storage, read_write> color_writes: array<atomic<u32>>;

// This entry is selected only for a full RGBA8 write, no fog/depth/stencil,
// and byte-quantized source-over or copy. The render attachment blends in draw
// order and quantizes to RGBA8 after each primitive.
@fragment
fn native_fragment(input: DrawFragmentInput) -> @location(0) vec4<f32> {
    let color = fragment_color(input);
    let width = reg(0u, 0x31eu) & 8191u;
    let at = u32(input.position.y) * width + u32(input.position.x);
    atomicOr(&color_writes[at], 15u);
    return color;
}

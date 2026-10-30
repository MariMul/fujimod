@group(0) @binding(0) var<uniform> extent: vec4<u32>;
@group(0) @binding(1) var packed_in: texture_2d<u32>;
@group(0) @binding(2) var color_out: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var color_in: texture_2d<f32>;
@group(0) @binding(4) var packed_out: texture_storage_2d<r32uint, write>;

@compute @workgroup_size(8, 8)
fn import_rgba8(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= extent.xy) { return; }
    let p = textureLoad(packed_in, vec2<i32>(id.xy), 0).x;
    let color = vec4<f32>(f32(p >> 24u), f32((p >> 16u) & 255u), f32((p >> 8u) & 255u), f32(p & 255u)) / 255.;
    textureStore(color_out, vec2<i32>(id.xy), color);
}

@compute @workgroup_size(8, 8)
fn export_rgba8(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= extent.xy) { return; }
    let color = textureLoad(color_in, vec2<i32>(id.xy), 0);
    let p = vec4<u32>(floor(clamp(color, vec4<f32>(0.), vec4<f32>(1.)) * 255. + 0.5));
    textureStore(packed_out, vec2<i32>(id.xy), vec4<u32>((p.x << 24u) | (p.y << 16u) | (p.z << 8u) | p.w, 0u, 0u, 0u));
}

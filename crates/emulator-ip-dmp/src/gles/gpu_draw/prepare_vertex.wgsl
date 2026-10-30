// Host pipeline preparation only. No draw using this vertex stage is submitted.
struct Uniforms { data: array<vec4<f32>, 141> };
@group(0) @binding(6) var<uniform> uniforms: Uniforms;
@group(0) @binding(7) var<storage, read> attributes: array<u32>;
struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) primary: vec4<f32>,
    @location(1) texcoords01: vec4<f32>,
    @location(2) texcoord2: vec2<f32>,
    @location(3) @interpolate(linear) depth: f32,
    @location(6) @interpolate(flat) valid: u32,
};
@vertex fn main() -> Vertex {
    var output: Vertex;
    output.position = vec4<f32>(uniforms.data[0].xy + vec2<f32>(f32(attributes[0] & 1u)), 0., 1.);
    output.primary = vec4<f32>(1.);
    output.texcoords01 = vec4<f32>(0.);
    output.texcoord2 = vec2<f32>(0.);
    output.depth = 0.;
    output.valid = 1u;
    return output;
}

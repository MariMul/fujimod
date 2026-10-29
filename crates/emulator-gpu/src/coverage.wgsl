struct CoverageJob {
    bounds: vec4<u32>,
    parameters: vec4<u32>,
    stroke_radius: i32,
    coverage_offset: u32,
    padding: vec2<u32>,
}

@group(0) @binding(0) var<storage, read> jobs: array<CoverageJob>;
@group(0) @binding(1) var<storage, read> edges: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> coverage: array<u32>;

fn edge_in_grid(index: u32) -> vec4<i32> {
    return vec4<i32>(round(edges[index] * 8.0));
}

fn multiply_u32(a: u32, b: u32) -> vec2<u32> {
    let a_high = a >> 16u;
    let a_low = a & 0xffffu;
    let b_high = b >> 16u;
    let b_low = b & 0xffffu;
    let first = a_low * b_low;
    let middle = a_high * b_low + (first >> 16u);
    let middle_low = middle & 0xffffu;
    let middle_high = middle >> 16u;
    let upper_middle = a_low * b_high + middle_low;
    let high = a_high * b_high + middle_high + (upper_middle >> 16u);
    let low = (upper_middle << 16u) | (first & 0xffffu);
    return vec2<u32>(low, high);
}

fn less_equal_u64(a: vec2<u32>, b: vec2<u32>) -> bool {
    return a.y < b.y || (a.y == b.y && a.x <= b.x);
}

fn add_u64(a: vec2<u32>, b: vec2<u32>) -> vec2<u32> {
    let low = a.x + b.x;
    let carry = select(0u, 1u, low < a.x);
    return vec2<u32>(low, a.y + b.y + carry);
}

fn square_u64(value: i32) -> vec2<u32> {
    let magnitude = u32(abs(value));
    return multiply_u32(magnitude, magnitude);
}

fn stroke_covers(point: vec2<i32>, edge: vec4<i32>, radius: i32) -> bool {
    if radius < 0 {
        return false;
    }
    let a = edge.xy;
    let b = edge.zw;
    let direction = b - a;
    let offset = point - a;
    let length_squared = direction.x * direction.x + direction.y * direction.y;
    let radius_squared = radius * radius;
    if length_squared == 0 {
        let distance_squared = add_u64(square_u64(offset.x), square_u64(offset.y));
        return less_equal_u64(distance_squared, square_u64(radius));
    }

    let projection = offset.x * direction.x + offset.y * direction.y;
    if projection <= 0 {
        let distance_squared = add_u64(square_u64(offset.x), square_u64(offset.y));
        return less_equal_u64(distance_squared, square_u64(radius));
    }
    if projection >= length_squared {
        let end_offset = point - b;
        let distance_squared = add_u64(square_u64(end_offset.x), square_u64(end_offset.y));
        return less_equal_u64(distance_squared, square_u64(radius));
    }

    let cross = direction.x * offset.y - direction.y * offset.x;
    return less_equal_u64(
        square_u64(cross),
        multiply_u32(u32(radius_squared), u32(length_squared)),
    );
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let job = jobs[id.z];
    if id.x >= job.bounds.z || id.y >= job.bounds.w {
        return;
    }

    let x = job.bounds.x + id.x;
    let y = job.bounds.y + id.y;
    let samples = job.parameters.x;
    let edge_offset = job.parameters.y;
    let edge_count = job.parameters.z;
    let stroke_enabled = (job.parameters.w & 1u) != 0u;
    let nonzero = (job.parameters.w & 2u) != 0u;
    let radius = job.stroke_radius;
    var pixel_coverage = 0u;

    for (var sy = 0u; sy < samples; sy += 1u) {
        let py = i32(y * 8u + ((sy * 2u + 1u) * 4u) / samples);
        for (var sx = 0u; sx < samples; sx += 1u) {
            let px = i32(x * 8u + ((sx * 2u + 1u) * 4u) / samples);
            let point = vec2<i32>(px, py);
            var covered = false;

            if stroke_enabled {
                for (var edge_index = 0u; edge_index < edge_count; edge_index += 1u) {
                    if stroke_covers(point, edge_in_grid(edge_offset + edge_index), radius) {
                        covered = true;
                        break;
                    }
                }
            } else {
                var winding = 0i;
                for (var edge_index = 0u; edge_index < edge_count; edge_index += 1u) {
                    let edge = edge_in_grid(edge_offset + edge_index);
                    let a = edge.xy;
                    let b = edge.zw;
                    let upward = b.y > a.y;
                    let crosses_row = (a.y <= py && b.y > py) || (b.y <= py && a.y > py);
                    if crosses_row {
                        let relative = (b.x - a.x) * (py - a.y)
                            - (b.y - a.y) * (px - a.x);
                        if (upward && relative > 0) || (!upward && relative < 0) {
                            winding += select(-1, 1, upward);
                        }
                    }
                }
                covered = select((winding & 1) != 0, winding != 0, nonzero);
            }

            pixel_coverage += select(0u, 1u, covered);
        }
    }

    coverage[job.coverage_offset + id.y * job.bounds.z + id.x] = pixel_coverage;
}

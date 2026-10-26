@group(0) @binding(0) var<storage, read> p: array<i32>;
@group(0) @binding(1) var<storage, read_write> coverage: array<u32>;
fn edge(a: vec2<i32>, b: vec2<i32>, c: vec2<i32>) -> i32 {
    return (b.x-a.x)*(c.y-a.y)-(b.y-a.y)*(c.x-a.x);
}
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let n = i32(id.x);
    if n >= p[8]*p[9] { return; }
    let c = vec2<i32>((p[6]+n%p[8])*16+8, (p[7]+n/p[8])*16+8);
    let a = vec2<i32>(p[0],p[1]);
    let b = vec2<i32>(p[2],p[3]);
    let d = vec2<i32>(p[4],p[5]);
    let e = vec3<i32>(edge(b,d,c),edge(d,a,c),edge(a,b,c));
    let include = vec3<bool>(p[10]!=0,p[11]!=0,p[12]!=0);
    coverage[n] = select(0u,1u,all((e>vec3<i32>(0)) | ((e==vec3<i32>(0)) & include)));
}

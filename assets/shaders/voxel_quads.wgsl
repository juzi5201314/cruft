#import bevy_render::view::View

struct ChunkUniform {
    origin: vec3<f32>,
    quad_base: u32,
}

struct Quad {
    low: u32,
    high: u32,
}

@group(0) @binding(0) var<uniform> view: View;

@group(1) @binding(0) var<uniform> chunk: ChunkUniform;
@group(1) @binding(1) var<storage, read> quads: array<u32>;
@group(1) @binding(2) var array_texture: texture_2d_array<f32>;
@group(1) @binding(3) var array_sampler: sampler;

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) layer: i32,
}

fn decode_u32(pair_index: u32) -> Quad {
    let low = quads[pair_index * 2u];
    let high = quads[pair_index * 2u + 1u];
    return Quad(low, high);
}

fn quad_corner(vid: u32) -> vec2<f32> {
    // two triangles: (0,0)-(1,0)-(0,1) and (0,1)-(1,0)-(1,1)
    switch (vid) {
        case 0u: { return vec2<f32>(0.0, 0.0); }
        case 1u: { return vec2<f32>(1.0, 0.0); }
        case 2u: { return vec2<f32>(0.0, 1.0); }
        case 3u: { return vec2<f32>(0.0, 1.0); }
        case 4u: { return vec2<f32>(1.0, 0.0); }
        default: { return vec2<f32>(1.0, 1.0); }
    }
}

@vertex
fn vertex(@builtin(vertex_index) vid: u32, @builtin(instance_index) instance_index: u32) -> VsOut {
    let q = decode_u32(chunk.quad_base + instance_index);

    let x = f32(q.low & 0x3Fu);
    let y = f32((q.low >> 6u) & 0x3Fu);
    let z = f32((q.low >> 12u) & 0x3Fu);
    let w = f32(((q.low >> 18u) & 0x1Fu) + 1u);
    let h = f32(((q.low >> 23u) & 0x1Fu) + 1u);
    let face = (q.low >> 28u) & 0x7u;

    let material_key = (q.high & 0xFFu);

    var normal = vec3<f32>(0.0, 1.0, 0.0);
    var u_axis = vec3<f32>(1.0, 0.0, 0.0);
    var v_axis = vec3<f32>(0.0, 0.0, 1.0);

    // w/h 轴向：按 `docs/voxel/meshing.md` 写死约定。
    switch (face) {
        // +X / -X：w 沿 +Z，h 沿 +Y
        case 0u: { normal = vec3<f32>( 1.0, 0.0, 0.0); u_axis = vec3<f32>(0.0, 0.0, 1.0); v_axis = vec3<f32>(0.0, 1.0, 0.0); }
        case 1u: { normal = vec3<f32>(-1.0, 0.0, 0.0); u_axis = vec3<f32>(0.0, 0.0, 1.0); v_axis = vec3<f32>(0.0, 1.0, 0.0); }
        // +Y / -Y：w 沿 +X，h 沿 +Z
        case 2u: { normal = vec3<f32>(0.0,  1.0, 0.0); u_axis = vec3<f32>(1.0, 0.0, 0.0); v_axis = vec3<f32>(0.0, 0.0, 1.0); }
        case 3u: { normal = vec3<f32>(0.0, -1.0, 0.0); u_axis = vec3<f32>(1.0, 0.0, 0.0); v_axis = vec3<f32>(0.0, 0.0, 1.0); }
        // +Z / -Z：w 沿 +X，h 沿 +Y
        case 4u: { normal = vec3<f32>(0.0, 0.0,  1.0); u_axis = vec3<f32>(1.0, 0.0, 0.0); v_axis = vec3<f32>(0.0, 1.0, 0.0); }
        default: { normal = vec3<f32>(0.0, 0.0, -1.0); u_axis = vec3<f32>(1.0, 0.0, 0.0); v_axis = vec3<f32>(0.0, 1.0, 0.0); }
    }

    let corner = quad_corner(vid);
    let local = vec3<f32>(x, y, z) + u_axis * (corner.x * w) + v_axis * (corner.y * h);
    let world = chunk.origin + local;
    let clip = view.clip_from_world * vec4<f32>(world, 1.0);

    var out: VsOut;
    out.clip_position = clip;
    out.uv = corner;
    out.layer = i32(material_key);
    return out;
}

@fragment
fn fragment(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(array_texture, array_sampler, in.uv, in.layer);
}

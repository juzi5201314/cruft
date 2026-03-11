#import bevy_render::view::View

const CHUNK_META_STRIDE: u32 = 12u;

struct Quad {
    low: u32,
    high: u32,
}

const MATERIAL_TABLE_STRIDE: u32 = 32u;
const FACE_CHANNEL_STRIDE: u32 = 5u;

@group(0) @binding(0) var<uniform> view: View;

// chunk_meta 固定 stride=12*u32：
// [origin.xyz(i32), opaque_offset(u32), min.xyz(i32), opaque_len(u32), max.xyz(i32), reserved]
@group(1) @binding(0) var<storage, read> chunk_meta: array<u32>;
@group(1) @binding(1) var<storage, read> quads: array<u32>;
@group(1) @binding(2) var<storage, read> material_table: array<u32>;
@group(1) @binding(3) var albedo_texture: texture_2d_array<f32>;
@group(1) @binding(4) var normal_texture: texture_2d_array<f32>;
@group(1) @binding(5) var orm_texture: texture_2d_array<f32>;
@group(1) @binding(6) var emissive_texture: texture_2d_array<f32>;
@group(1) @binding(7) var height_texture: texture_2d_array<f32>;
@group(1) @binding(8) var array_sampler: sampler;

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) face: u32,
    @location(2) @interpolate(flat) material_id: u32,
}

fn chunk_meta_base(chunk_index: u32) -> u32 {
    return chunk_index * CHUNK_META_STRIDE;
}

fn chunk_origin(chunk_index: u32) -> vec3<f32> {
    let base = chunk_meta_base(chunk_index);
    return vec3<f32>(
        f32(bitcast<i32>(chunk_meta[base + 0u])),
        f32(bitcast<i32>(chunk_meta[base + 1u])),
        f32(bitcast<i32>(chunk_meta[base + 2u])),
    );
}

fn chunk_quad_base(chunk_index: u32) -> u32 {
    let base = chunk_meta_base(chunk_index);
    return chunk_meta[base + 3u];
}

fn decode_u32(pair_index: u32) -> Quad {
    let low = quads[pair_index * 2u];
    let high = quads[pair_index * 2u + 1u];
    return Quad(low, high);
}

fn face_offset(face: u32) -> u32 {
    switch (face) {
        case 0u: { return 22u; } // east
        case 1u: { return 27u; } // west
        case 2u: { return 2u; }  // top
        case 3u: { return 7u; }  // bottom
        case 4u: { return 17u; } // south
        default: { return 12u; } // north
    }
}

fn material_base(material_id: u32) -> u32 {
    return material_id * MATERIAL_TABLE_STRIDE;
}

fn material_face_layer(material_id: u32, face: u32, channel_offset: u32) -> i32 {
    let base = material_base(material_id) + face_offset(face) + channel_offset;
    if (base >= arrayLength(&material_table)) {
        return 0;
    }
    return i32(material_table[base]);
}

fn material_alpha_mode(material_id: u32) -> u32 {
    let base = material_base(material_id);
    if (base >= arrayLength(&material_table)) {
        return 0u;
    }
    return material_table[base];
}

fn material_cutout_threshold(material_id: u32) -> f32 {
    let base = material_base(material_id);
    if (base + 1u >= arrayLength(&material_table)) {
        return 0.5;
    }
    return bitcast<f32>(material_table[base + 1u]);
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
fn vertex(
    @builtin(vertex_index) global_vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VsOut {
    // draw 命令把 first_vertex 设为 chunk_index*6，因此可反推 chunk 索引。
    let chunk_index = global_vertex_index / 6u;
    let vid = global_vertex_index % 6u;

    let q = decode_u32(chunk_quad_base(chunk_index) + instance_index);

    let x = f32(q.low & 0x3Fu);
    let y = f32((q.low >> 6u) & 0x3Fu);
    let z = f32((q.low >> 12u) & 0x3Fu);
    let w = f32(((q.low >> 18u) & 0x1Fu) + 1u);
    let h = f32(((q.low >> 23u) & 0x1Fu) + 1u);
    let face = (q.low >> 28u) & 0x7u;
    let material_id = (q.high & 0xFFFFu);

    var u_axis = vec3<f32>(1.0, 0.0, 0.0);
    var v_axis = vec3<f32>(0.0, 0.0, 1.0);

    // w/h 轴向：按 `docs/voxel/meshing.md` 写死约定。
    switch (face) {
        // +X / -X：w 沿 +Z，h 沿 +Y
        case 0u: { u_axis = vec3<f32>(0.0, 0.0, 1.0); v_axis = vec3<f32>(0.0, 1.0, 0.0); }
        case 1u: { u_axis = vec3<f32>(0.0, 0.0, 1.0); v_axis = vec3<f32>(0.0, 1.0, 0.0); }
        // +Y / -Y：w 沿 +X，h 沿 +Z
        case 2u: { u_axis = vec3<f32>(1.0, 0.0, 0.0); v_axis = vec3<f32>(0.0, 0.0, 1.0); }
        case 3u: { u_axis = vec3<f32>(1.0, 0.0, 0.0); v_axis = vec3<f32>(0.0, 0.0, 1.0); }
        // +Z / -Z：w 沿 +X，h 沿 +Y
        case 4u: { u_axis = vec3<f32>(1.0, 0.0, 0.0); v_axis = vec3<f32>(0.0, 1.0, 0.0); }
        default: { u_axis = vec3<f32>(1.0, 0.0, 0.0); v_axis = vec3<f32>(0.0, 1.0, 0.0); }
    }

    // 约定：primitive front face 为 CCW。
    // 当前 w/h 轴向按 docs/voxel/meshing.md 固定，因此这里用“翻转角点 u”来修正部分面的绕序，
    // 确保开启背面剔除后所有面都能从外侧可见。
    let invert_winding = (face == 0u) || (face == 2u) || (face == 5u);
    var corner = quad_corner(vid);
    if (invert_winding) {
        corner.x = 1.0 - corner.x;
    }

    let local = vec3<f32>(x, y, z) + u_axis * (corner.x * w) + v_axis * (corner.y * h);
    let world = chunk_origin(chunk_index) + local;
    let clip = view.clip_from_world * vec4<f32>(world, 1.0);

    var out: VsOut;
    out.clip_position = clip;
    out.uv = corner;
    out.face = face;
    out.material_id = material_id;
    return out;
}

@fragment
fn fragment(in: VsOut) -> @location(0) vec4<f32> {
    let albedo_layer = material_face_layer(in.material_id, in.face, 0u);
    let normal_layer = material_face_layer(in.material_id, in.face, 1u);
    let orm_layer = material_face_layer(in.material_id, in.face, 2u);
    let emissive_layer = material_face_layer(in.material_id, in.face, 3u);
    let height_layer = material_face_layer(in.material_id, in.face, 4u);
    let color = textureSample(albedo_texture, array_sampler, in.uv, albedo_layer);
    let orm = textureSample(orm_texture, array_sampler, in.uv, orm_layer);
    let _normal = textureSample(normal_texture, array_sampler, in.uv, normal_layer);
    let emissive = textureSample(emissive_texture, array_sampler, in.uv, emissive_layer);
    let _height = textureSample(height_texture, array_sampler, in.uv, height_layer);
    if (material_alpha_mode(in.material_id) == 1u && orm.a < material_cutout_threshold(in.material_id)) {
        discard;
    }
    return vec4<f32>(color.rgb + emissive.rgb * 0.0, color.a);
}

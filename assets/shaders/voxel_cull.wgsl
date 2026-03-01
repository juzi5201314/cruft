const CHUNK_META_STRIDE: u32 = 12u;
const WORKGROUP_SIZE: u32 = 64u;

struct CullingUniform {
    clip_from_world: mat4x4<f32>,
    chunk_count: u32,
    hzb_mip_count: u32,
    hzb_enabled: u32,
    _pad0: u32,
    hzb_size: vec2<u32>,
    _pad1: vec2<u32>,
}

@group(0) @binding(0) var<uniform> culling: CullingUniform;

// chunk_meta 固定 stride=12*u32：
// [origin.xyz(i32), opaque_offset(u32), min.xyz(i32), opaque_len(u32), max.xyz(i32), reserved]
@group(0) @binding(1) var<storage, read> chunk_meta: array<u32>;
@group(0) @binding(2) var<storage, read_write> indirect: array<u32>;
@group(0) @binding(3) var depth_pyramid: texture_2d<f32>;

struct ProjectedAabb {
    valid: u32,
    uv_min: vec2<f32>,
    uv_max: vec2<f32>,
    max_depth_ndc: f32,
}

fn chunk_meta_base(chunk_index: u32) -> u32 {
    return chunk_index * CHUNK_META_STRIDE;
}

fn chunk_min(chunk_index: u32) -> vec3<f32> {
    let base = chunk_meta_base(chunk_index);
    return vec3<f32>(
        f32(bitcast<i32>(chunk_meta[base + 4u])),
        f32(bitcast<i32>(chunk_meta[base + 5u])),
        f32(bitcast<i32>(chunk_meta[base + 6u])),
    );
}

fn chunk_max(chunk_index: u32) -> vec3<f32> {
    let base = chunk_meta_base(chunk_index);
    return vec3<f32>(
        f32(bitcast<i32>(chunk_meta[base + 8u])),
        f32(bitcast<i32>(chunk_meta[base + 9u])),
        f32(bitcast<i32>(chunk_meta[base + 10u])),
    );
}

fn chunk_opaque_len(chunk_index: u32) -> u32 {
    let base = chunk_meta_base(chunk_index);
    return chunk_meta[base + 7u];
}

fn ndc_to_uv(ndc: vec2<f32>) -> vec2<f32> {
    return ndc * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
}

fn aabb_visible(min: vec3<f32>, max: vec3<f32>) -> bool {
    let corners = array<vec3<f32>, 8>(
        vec3<f32>(min.x, min.y, min.z),
        vec3<f32>(max.x, min.y, min.z),
        vec3<f32>(min.x, max.y, min.z),
        vec3<f32>(max.x, max.y, min.z),
        vec3<f32>(min.x, min.y, max.z),
        vec3<f32>(max.x, min.y, max.z),
        vec3<f32>(min.x, max.y, max.z),
        vec3<f32>(max.x, max.y, max.z),
    );

    var all_left = true;
    var all_right = true;
    var all_bottom = true;
    var all_top = true;
    var all_near = true;
    var all_far = true;

    for (var i = 0u; i < 8u; i = i + 1u) {
        let p = culling.clip_from_world * vec4<f32>(corners[i], 1.0);
        all_left = all_left && (p.x < -p.w);
        all_right = all_right && (p.x > p.w);
        all_bottom = all_bottom && (p.y < -p.w);
        all_top = all_top && (p.y > p.w);
        all_near = all_near && (p.z > p.w);
        all_far = all_far && (p.z < 0.0);
    }

    return !(all_left || all_right || all_bottom || all_top || all_near || all_far);
}

fn project_aabb(min: vec3<f32>, max: vec3<f32>) -> ProjectedAabb {
    let corners = array<vec3<f32>, 8>(
        vec3<f32>(min.x, min.y, min.z),
        vec3<f32>(max.x, min.y, min.z),
        vec3<f32>(min.x, max.y, min.z),
        vec3<f32>(max.x, max.y, min.z),
        vec3<f32>(min.x, min.y, max.z),
        vec3<f32>(max.x, min.y, max.z),
        vec3<f32>(min.x, max.y, max.z),
        vec3<f32>(max.x, max.y, max.z),
    );

    var uv_min = vec2<f32>(1.0, 1.0);
    var uv_max = vec2<f32>(0.0, 0.0);
    var max_depth_ndc = 0.0;
    var valid = false;

    for (var i = 0u; i < 8u; i = i + 1u) {
        let clip = culling.clip_from_world * vec4<f32>(corners[i], 1.0);
        if (clip.w <= 0.0) {
            continue;
        }

        let ndc = clip.xyz / clip.w;
        let uv = ndc_to_uv(ndc.xy);
        uv_min = min(uv_min, uv);
        uv_max = max(uv_max, uv);
        max_depth_ndc = max(max_depth_ndc, clamp(ndc.z, 0.0, 1.0));
        valid = true;
    }

    if (!valid) {
        return ProjectedAabb(0u, vec2<f32>(0.0), vec2<f32>(0.0), 1.0);
    }

    uv_min = clamp(uv_min, vec2<f32>(0.0), vec2<f32>(1.0));
    uv_max = clamp(uv_max, vec2<f32>(0.0), vec2<f32>(1.0));
    if (any(uv_max <= uv_min)) {
        return ProjectedAabb(0u, uv_min, uv_max, max_depth_ndc);
    }

    return ProjectedAabb(1u, uv_min, uv_max, max_depth_ndc);
}

fn pick_hzb_mip(uv_min: vec2<f32>, uv_max: vec2<f32>) -> i32 {
    let pixel_size = (uv_max - uv_min) * vec2<f32>(culling.hzb_size);
    let max_span = max(max(pixel_size.x, pixel_size.y), 1.0);
    let level = i32(ceil(log2(max_span)));
    return clamp(level, 0, i32(culling.hzb_mip_count) - 1);
}

fn get_occluder_depth(uv_min: vec2<f32>, uv_max: vec2<f32>) -> f32 {
    let mip_level = pick_hzb_mip(uv_min, uv_max);
    let tex_size = vec2<u32>(textureDimensions(depth_pyramid, mip_level));
    let max_top_left = max(tex_size, vec2<u32>(2u, 2u)) - vec2<u32>(2u, 2u);
    let top_left = min(vec2<u32>(uv_min * vec2<f32>(tex_size)), max_top_left);

    let a = textureLoad(depth_pyramid, top_left + vec2<u32>(0u, 0u), mip_level).x;
    let b = textureLoad(depth_pyramid, top_left + vec2<u32>(1u, 0u), mip_level).x;
    let c = textureLoad(depth_pyramid, top_left + vec2<u32>(0u, 1u), mip_level).x;
    let d = textureLoad(depth_pyramid, top_left + vec2<u32>(1u, 1u), mip_level).x;
    return min(min(a, b), min(c, d));
}

fn hzb_visible(min: vec3<f32>, max: vec3<f32>) -> bool {
    if (culling.hzb_enabled == 0u || culling.hzb_mip_count == 0u) {
        return true;
    }

    let projected = project_aabb(min, max);
    if (projected.valid == 0u) {
        return true;
    }

    let occluder_depth_ndc = get_occluder_depth(projected.uv_min, projected.uv_max);
    return !(projected.max_depth_ndc + 1e-4 < occluder_depth_ndc);
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn cull(@builtin(global_invocation_id) gid: vec3<u32>) {
    let chunk_index = gid.x;
    if (chunk_index >= culling.chunk_count) {
        return;
    }

    let min = chunk_min(chunk_index);
    let max = chunk_max(chunk_index);
    let opaque_len = chunk_opaque_len(chunk_index);

    var visible = opaque_len > 0u;
    if (visible) {
        visible = aabb_visible(min, max);
    }
    if (visible) {
        visible = hzb_visible(min, max);
    }

    // DrawIndirectArgs: [vertex_count, instance_count, first_vertex, first_instance]
    let cmd_base = chunk_index * 4u;
    indirect[cmd_base + 0u] = 6u;
    indirect[cmd_base + 1u] = select(0u, opaque_len, visible);
    indirect[cmd_base + 2u] = chunk_index * 6u;
    indirect[cmd_base + 3u] = 0u;
}

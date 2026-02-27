const MAX_LAYERS: u32 = 256u;

struct LayerParams {
    seed: u32,
    octaves: u32,
    _pad0: vec2<u32>,

    noise_scale: f32,
    warp_strength: f32,
    _pad1: vec2<f32>,

    palette: array<vec4<f32>, 4>,
};

struct Params {
    layer_count: u32,
    _pad0: vec3<u32>,
    layers: array<LayerParams, MAX_LAYERS>,
};

@group(0) @binding(0) var out_tex: texture_storage_2d_array<rgba8unorm, write>;
@group(0) @binding(1) var<uniform> params: Params;

fn hash_u32(mut x: u32) -> u32 {
    // xorshift32
    x = x ^ (x << 13u);
    x = x ^ (x >> 17u);
    x = x ^ (x << 5u);
    return x;
}

fn hash2(ix: u32, iy: u32, seed: u32) -> f32 {
    let h = hash_u32(ix * 1664525u ^ iy * 1013904223u ^ seed);
    return f32(h) / 4294967295.0;
}

fn fade(t: f32) -> f32 {
    return t * t * (3.0 - 2.0 * t);
}

fn value_noise_periodic(uv: vec2<f32>, cell_count: u32, seed: u32) -> f32 {
    // uv: [0,1] inclusive on the last pixel, so wrap explicitly.
    let uv_wrapped = fract(uv);

    let p = uv_wrapped * f32(cell_count);
    let i0 = vec2<u32>(u32(floor(p.x)) % cell_count, u32(floor(p.y)) % cell_count);
    let i1 = vec2<u32>((i0.x + 1u) % cell_count, (i0.y + 1u) % cell_count);
    let f = fract(p);
    let fx = fade(f.x);
    let fy = fade(f.y);

    let v00 = hash2(i0.x, i0.y, seed);
    let v10 = hash2(i1.x, i0.y, seed);
    let v01 = hash2(i0.x, i1.y, seed);
    let v11 = hash2(i1.x, i1.y, seed);

    let vx0 = mix(v00, v10, fx);
    let vx1 = mix(v01, v11, fx);
    return mix(vx0, vx1, fy);
}

fn fbm(uv: vec2<f32>, dims: vec2<u32>, p: LayerParams) -> f32 {
    let min_dim = f32(min(dims.x, dims.y));
    let octave_count = min(p.octaves, 8u);

    var sum = 0.0;
    var norm = 0.0;
    var amp = 1.0;
    var freq = 1.0;

    for (var i = 0u; i < 8u; i = i + 1u) {
        if (i >= octave_count) {
            break;
        }

        // 与 Python 参考方案的直觉一致：noise_scale 越大，越“粗”（大块）。
        let cells_f = max(1.0, min_dim / (p.noise_scale * freq));
        let cell_count = max(1u, u32(cells_f));

        sum = sum + value_noise_periodic(uv, cell_count, p.seed + i * 101u) * amp;
        norm = norm + amp;
        amp = amp * 0.5;
        freq = freq * 2.0;
    }

    return sum / max(1e-6, norm);
}

fn sample_palette(n: f32, p: LayerParams) -> vec4<f32> {
    let v = clamp(n, 0.0, 0.999999);
    let idx = u32(floor(v * 4.0));
    return p.palette[idx];
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(out_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) {
        return;
    }

    let layer = gid.z;
    if (layer >= params.layer_count) {
        return;
    }

    let p = params.layers[layer];

    // 让边缘像素（x=0 与 x=width-1）生成相同取样点，从而更接近“像素级无缝”。
    let denom_x = max(1u, dims.x - 1u);
    let denom_y = max(1u, dims.y - 1u);
    var uv = vec2<f32>(f32(gid.x) / f32(denom_x), f32(gid.y) / f32(denom_y));

    if (p.warp_strength > 0.0) {
        let w1 = fbm(uv, dims, p);
        let w2 = fbm(uv + vec2<f32>(0.37, 0.73), dims, p);
        let warp = (vec2<f32>(w1, w2) - 0.5) * p.warp_strength;
        uv = fract(uv + warp);
    }

    let n = fbm(uv, dims, p);
    let color = sample_palette(n, p);
    textureStore(out_tex, vec2<i32>(i32(gid.x), i32(gid.y)), i32(layer), color);
}

// Current production style: minecraft_quantized (4-color palette quantization).
// Other styles are planned in schema/data, but not implemented in shader yet.
const MAX_LAYERS: u32 = 256u;
const STYLE_MINECRAFT: u32 = 0u;
const STYLE_HD_PIXEL_ART: u32 = 1u;
const STYLE_HD_REALISTIC: u32 = 2u;
const STYLE_VECTOR_TOON: u32 = 3u;

const BAYER8X8: array<u32, 64> = array<u32, 64>(
    0u, 32u, 8u, 40u, 2u, 34u, 10u, 42u,
    48u, 16u, 56u, 24u, 50u, 18u, 58u, 26u,
    12u, 44u, 4u, 36u, 14u, 46u, 6u, 38u,
    60u, 28u, 52u, 20u, 62u, 30u, 54u, 22u,
    3u, 35u, 11u, 43u, 1u, 33u, 9u, 41u,
    51u, 19u, 59u, 27u, 49u, 17u, 57u, 25u,
    15u, 47u, 7u, 39u, 13u, 45u, 5u, 37u,
    63u, 31u, 55u, 23u, 61u, 29u, 53u, 21u,
);

struct LayerParams {
    style: u32,
    seed: u32,
    octaves: u32,
    has_layer: u32,

    noise_scale: f32,
    warp_strength: f32,
    layer_ratio: f32,
    base_palette_offset: u32,

    base_palette_len: u32,
    top_palette_offset: u32,
    top_palette_len: u32,
    _pad0: u32,
};

struct Params {
    layer_count: u32,
    _pad0: vec3<u32>,
    _pad1: u32,
    layers: array<LayerParams, MAX_LAYERS>,
};

struct PaletteStorage {
    color_count: u32,
    colors: array<vec4<f32>>,
};

@group(0) @binding(0) var out_tex: texture_storage_2d_array<rgba8unorm, write>;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<storage, read> palettes: PaletteStorage;

fn hash_u32(x: u32) -> u32 {
    // xorshift32
    var v = x;
    v = v ^ (v << 13u);
    v = v ^ (v >> 17u);
    v = v ^ (v << 5u);
    return v;
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

fn sample_palette_step(n: f32, palette_offset: u32, palette_len: u32) -> vec4<f32> {
    let count = max(1u, palette_len);
    let v = clamp(n, 0.0, 0.999999);
    let idx = u32(floor(v * f32(count)));
    return palettes.colors[palette_offset + min(idx, count - 1u)];
}

fn sample_palette_linear(n: f32, palette_offset: u32, palette_len: u32) -> vec4<f32> {
    let count = max(1u, palette_len);
    if (count == 1u) {
        return palettes.colors[palette_offset];
    }

    let v = clamp(n, 0.0, 0.999999);
    let scaled = v * f32(count - 1u);
    let lo = u32(floor(scaled));
    let hi = min(count - 1u, lo + 1u);
    let t = fract(scaled);
    return mix(
        palettes.colors[palette_offset + lo],
        palettes.colors[palette_offset + hi],
        t,
    );
}

fn bayer_threshold(x: u32, y: u32) -> f32 {
    let idx = (y & 7u) * 8u + (x & 7u);
    return f32(BAYER8X8[idx]) / 64.0;
}

fn apply_emboss(base_noise: f32, shifted_noise: f32, strength: f32) -> f32 {
    return clamp(base_noise + (base_noise - shifted_noise) * strength, 0.0, 0.999999);
}

fn stylized_noise(uv: vec2<f32>, dims: vec2<u32>, p: LayerParams) -> f32 {
    let base_noise = fbm(uv, dims, p);
    let step = vec2<f32>(
        1.0 / f32(max(1u, dims.x)),
        1.0 / f32(max(1u, dims.y)),
    );
    let shifted_noise = fbm(fract(uv + step), dims, p);
    let emboss_strength = select(0.25, 0.1, p.style == STYLE_HD_REALISTIC);
    return apply_emboss(base_noise, shifted_noise, emboss_strength);
}

fn srgb_u8_to_linear(v: u32) -> f32 {
    let x = f32(v) / 255.0;
    if (x <= 0.04045) {
        return x / 12.92;
    }
    return pow((x + 0.055) / 1.055, 2.4);
}

fn sample_style_color(
    style: u32,
    noise: f32,
    uv: vec2<f32>,
    gid_xy: vec2<u32>,
    dims: vec2<u32>,
    p: LayerParams,
    palette_offset: u32,
    palette_len: u32,
) -> vec4<f32> {
    switch style {
        case STYLE_MINECRAFT: {
            return sample_palette_step(noise, palette_offset, palette_len);
        }
        case STYLE_HD_PIXEL_ART: {
            let spread = 1.0 / f32(max(1u, palette_len));
            let threshold = bayer_threshold(gid_xy.x, gid_xy.y) - 0.5;
            let dithered = clamp(noise + threshold * spread, 0.0, 0.999999);
            return sample_palette_step(dithered, palette_offset, palette_len);
        }
        case STYLE_HD_REALISTIC: {
            return sample_palette_linear(noise, palette_offset, palette_len);
        }
        case STYLE_VECTOR_TOON: {
            let dx = vec2<f32>(1.0 / f32(max(1u, dims.x)), 0.0);
            let dy = vec2<f32>(0.0, 1.0 / f32(max(1u, dims.y)));
            let noise_x = stylized_noise(fract(uv + dx), dims, p);
            let noise_y = stylized_noise(fract(uv + dy), dims, p);
            let is_edge = (abs(noise - noise_x) + abs(noise - noise_y)) > 0.04;
            if (is_edge) {
                let outline = srgb_u8_to_linear(20u);
                return vec4<f32>(outline, outline, outline, 1.0);
            }
            let quantized = floor(clamp(noise, 0.0, 0.999999) * 4.0) / 4.0;
            return sample_palette_step(quantized, palette_offset, palette_len);
        }
        default: {
            return sample_palette_step(noise, palette_offset, palette_len);
        }
    }
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

    let n = stylized_noise(uv, dims, p);
    var palette_offset = p.base_palette_offset;
    var palette_len = p.base_palette_len;

    if (p.has_layer != 0u && p.top_palette_len > 0u) {
        let boundary_y = f32(dims.y) * p.layer_ratio + (n - 0.5) * f32(dims.y) * 0.5;
        if (f32(gid.y) < boundary_y) {
            palette_offset = p.top_palette_offset;
            palette_len = p.top_palette_len;
        }
    }

    let color = sample_style_color(p.style, n, uv, gid.xy, dims, p, palette_offset, palette_len);
    textureStore(out_tex, vec2<i32>(i32(gid.x), i32(gid.y)), i32(layer), color);
}

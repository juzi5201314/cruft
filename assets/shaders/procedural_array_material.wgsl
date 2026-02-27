#import bevy_pbr::forward_io::{FragmentOutput, VertexOutput}
#import bevy_pbr::mesh_bindings::mesh

@group(2) @binding(0) var array_texture: texture_2d_array<f32>;
@group(2) @binding(1) var array_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    // MeshTag 被写入 mesh[].tag（见 bevy_pbr::mesh_types.wgsl）。
    // 这里直接用它作为 array layer index。
    let layer = i32(mesh[in.instance_index].tag);

    // 该材质用于预览，使用 mesh 的 UV0。
#ifdef VERTEX_UVS_A
    let uv = in.uv;
#else
    let uv = vec2<f32>(0.0, 0.0);
#endif

    let color = textureSample(array_texture, array_sampler, uv, layer);

    var out: FragmentOutput;
    out.color = color;
    return out;
}


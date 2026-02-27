# 3D / PBR（Bevy v0.18）：Mesh3d / StandardMaterial / Lights / Camera3d

本篇覆盖 3D 入门骨架与 PBR 常用能力（灯光、阴影、环境贴图、常见屏幕空间效果入口）。

## 0) 关键入口

- PBR crate 入口（插件、图节点、shader library）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_pbr/src/lib.rs`
- Core 3D pipeline：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_core_pipeline/src/core_3d`
- 示例（PBR 参数展示）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/pbr.rs`

## 1) 最小 3D 骨架

```rust
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::default())),
        MeshMaterial3d(materials.add(StandardMaterial::default())),
        Transform::from_xyz(0.0, 0.5, 0.0),
    ));

    commands.spawn((
        PointLight::default(),
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
```

这种写法在很多 3D examples 里都能找到。

## 2) `StandardMaterial`：最常用的 PBR 材质

你可以调这些关键字段（示例里有网格对比）：

- `base_color`
- `metallic`
- `perceptual_roughness`
- `unlit`
- `alpha_mode`

示例：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/pbr.rs`

## 3) 环境贴图与加载时机（资产系统联动）

`pbr.rs` 展示了如何把环境贴图挂在 `Camera3d` 上，并用 `AssetServer::load_state` 等待其加载完成后再移除 UI label：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/pbr.rs`（`EnvironmentMapLight` + `environment_map_load_finish`）

这类模式非常适合和 states 结合实现 Loading。

## 4) 常见 3D 渲染效果：SSAO / SSR / Tonemapping / Fog 等

在 `examples/3d/` 下有大量“开箱即用”的效果示例，例如：

- `ssao.rs`（屏幕空间 AO）
- `ssr.rs`（屏幕空间反射）
- `tonemapping.rs`
- `volumetric_fog.rs`

这些效果通常来自：

- `bevy_pbr`（SSAO/SSR/Fog/Volumetric 等）
- `bevy_core_pipeline`（Tonemapping/Upscaling/OIT 等）

## 5) 进一步扩展：自定义材质与 shader

当 `StandardMaterial` 不够用：

- 先看 “自定义 Material” 的标准路径：
  - `shaders_materials.md`
  - 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader/shader_material.rs`

再考虑更底层的 RenderGraph/自定义 pipeline（`rendering_architecture.md`）。

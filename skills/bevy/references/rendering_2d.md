# 2D 渲染（Bevy v0.18）：Sprite / Mesh2d / Camera2d

本篇是 2D 方向的“能跑模板 + 常见扩展点”。

## 0) 关键 examples

- 最小 sprite：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/sprite.rs`
- Mesh2d：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/mesh2d.rs`
- 更底层渲染 API：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/mesh2d_manual.rs`
- 2D 后处理：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/bloom_2d.rs`

## 1) Camera2d：2D 场景的起点

最常见：

```rust
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}
```

（Camera 相关的更细节如 viewport_to_world、multi-camera 等，参考 `examples/camera/*` 与 `input_window.md`）

## 2) Sprite：最快的 2D 上手方式

```rust
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn(Sprite::from_image(asset_server.load("branding/icon.png")));
}
```

- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/sprite.rs`

常见扩展：

- `Transform`：移动/缩放/旋转
- `Sprite` 字段：color、flip、rect、texture_atlas 等（按需求）

## 3) Mesh2d + `ColorMaterial`：更通用的 2D 渲染路径

2D 的 mesh 仍然使用 `Mesh` 资产，但 entity 上挂的是 `Mesh2d(...)`：

```rust
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::default())),
        MeshMaterial2d(materials.add(Color::srgb(0.7, 0.2, 0.8))),
        Transform::default().with_scale(Vec3::splat(128.)),
    ));
}
```

- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/mesh2d.rs`

你可以把 `ColorMaterial` 换成自定义 `Material`（见 `shaders_materials.md`）。

## 4) 2D 文本：Text2d（与 UI Text 的区别）

2D 世界空间文字通常用 `Text2d`（在 `bevy_sprite` 的 text2d 模块）；
UI 文字用 `bevy_ui::widget::Text`（UI 树里）。

参考：

- Text pipeline 解释：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_text/src/lib.rs`
- UI 文本 examples：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/text.rs`

## 5) 2D 与渲染架构的连接点

2D 的管线来自 core pipeline：

- `bevy_core_pipeline::core_2d`
  - Source：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_core_pipeline/src/core_2d`

通常你不需要手动操作 RenderGraph；除非你要写自定义后处理/自定义渲染阶段（见 `rendering_architecture.md`）。

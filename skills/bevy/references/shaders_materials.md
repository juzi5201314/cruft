# Shaders / Materials（Bevy v0.18）：自定义 `Material` 的标准路线

本篇聚焦“用户最常做的渲染扩展”：不改引擎渲染架构，只通过 `Material` + WGSL/GLSL/WESL 写自定义外观。

## 0) 最佳起点：示例直接抄骨架

3D 自定义材质：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader/shader_material.rs`

2D 自定义材质：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader/shader_material_2d.rs`

Array texture / texture array（`texture_2d_array<f32>`）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader/array_texture.rs`
- WGSL：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/assets/shaders/array_texture.wgsl`

扩展材质（ExtendedMaterial）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader/extended_material.rs`

## 1) 核心步骤（3D）

1. 定义材质数据结构：
   - `#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]`
   - 用 `#[uniform(n)]` / `#[texture(n)]` / `#[sampler(n)]` 映射到 bind group
2. 实现 `Material` trait：
   - 指定 `fragment_shader()` / `vertex_shader()`（按需）
   - 覆写 `alpha_mode()` 等行为（按需）
3. 注册插件：
   - `.add_plugins(MaterialPlugin::<MyMaterial>::default())`
4. 在实体上挂：
   - `Mesh3d(handle)`
   - `MeshMaterial3d(material_handle)`

这四步在示例里是完整闭环：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader/shader_material.rs`

## 2) Shader 资产：如何加载与引用

示例使用：

- `const SHADER_ASSET_PATH: &str = "shaders/custom_material.wgsl";`
- `fn fragment_shader() -> ShaderRef { SHADER_ASSET_PATH.into() }`

要点：

- shader 是资产（`AssetServer` 可加载）
- 如果你的 shader 有依赖（import/include），可能需要考虑嵌入与 `load_shader_library!`：
  - 宏定义：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_shader/src/lib.rs`
  - core pipeline / pbr 里大量使用 `load_shader_library!(app, "...")`（见 `bevy_pbr/src/lib.rs`）

## 3) 2D 材质的差异

2D 用 `Mesh2d` + `MeshMaterial2d<T>`：

- 参考：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/mesh2d.rs`（内建 `ColorMaterial`）
- 自定义 2D Material 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader/shader_material_2d.rs`

## 4) Shader defs / specialization（进阶）

当你需要用 shader defs 或根据材质/平台做 specialization：

- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader/shader_defs.rs`
- 更深入需要看 `bevy_render` 的 pipeline cache、render_resource 等模块：
  - `https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_render/src/render_resource`
  - `https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_render/src/render_asset`

## 5) Texture arrays：`texture_2d_array` 与 `ImageArrayLayout`

当你想在 WGSL 里声明：

- `texture_2d_array<f32>`

你需要两端都匹配：

1. **Rust 材质绑定**：在 `AsBindGroup` 上把纹理维度声明为 `2d_array`：
   - `#[texture(n, dimension = "2d_array")]`
2. **Image 加载方式**：用 `AssetServer::load_with_settings` + `ImageLoaderSettings.array_layout` 把一张图解释成 “多层 array texture”（例如按行切成 N 层）：
   - `ImageArrayLayout::RowCount { rows: N }`

权威模板（包含 WGSL + 材质绑定 + 运行时切 layer）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader/array_texture.rs`

## 6) 更底层：自定义 RenderGraph 节点 / RenderPipeline / bindless

当你要做：

- 屏幕空间后处理（读取主 pass 纹理）
- Specialized mesh pipeline（完全控制 `RenderPipelineDescriptor`）
- bindless / `binding_array<texture_2d<f32>>`

看：`render_pipeline_custom_rendering.md`（并优先对照 `examples/shader_advanced/*`）。

## 7) Compute shader 与 GPU readback（更偏“渲染系统”）

Compute 入门例子：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader/compute_shader_game_of_life.rs`

读回 GPU 数据：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader/gpu_readback.rs`

这类需求往往会涉及 RenderApp 与渲染阶段插入（参考 `rendering_architecture.md`）。

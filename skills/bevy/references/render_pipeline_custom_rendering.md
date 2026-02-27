# 自定义渲染管线（Bevy v0.18）：RenderGraph / RenderPhase / PipelineCache / WGSL

当 `Material + WGSL` 不够用（例如你要插自定义 render pass、做屏幕空间后处理、写自定义 RenderPhase、或完全掌控 mesh 的 pipeline），就需要进入 Bevy 的“渲染系统层”：

- RenderApp（渲染子世界）
- RenderGraph（按节点组织渲染）
- RenderPhase（把 draw call 分桶/排序/批处理）
- PipelineCache / Specialized*Pipelines（管线缓存与特化）
- render_resource（wgpu 资源与 bind group/pipeline 描述）

> 基础脉络先看 `rendering_architecture.md`；本篇提供“如何真的写出来”的入口与常见模式。

## 0) 最重要的参考：先抄 examples（再看源码）

这些 examples 基本覆盖了“写自定义渲染”的主路线：

- 自定义后处理（读取主 pass 纹理，写回 destination）：
  - Rust：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader_advanced/custom_post_processing.rs`
  - WGSL：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/assets/shaders/post_processing.wgsl`
- Specialized mesh pipeline（不用 material API，直接控制 RenderPipelineDescriptor）：
  - Rust：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader_advanced/specialized_mesh_pipeline.rs`
  - WGSL：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/assets/shaders/specialized_mesh_pipeline.wgsl`
- Depth-only camera（只渲深度到 depth buffer → 复制到纹理 → shader 采样）：
  - Rust：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader_advanced/render_depth_to_texture.rs`
  - WGSL：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/assets/shaders/show_depth_texture_material.wgsl`
- 绑定数组/Bindless（`binding_array<texture_2d<f32>>` + non-uniform indexing）：
  - Rust：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader_advanced/texture_binding_array.rs`
  - WGSL：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/assets/shaders/texture_binding_array.wgsl`
- 自定义 RenderPhase / 自定义 PhaseItem：
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader_advanced/custom_render_phase.rs`
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader_advanced/custom_phase_item.rs`
  - WGSL（phase item shader）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/assets/shaders/custom_phase_item.wgsl`
- 手写 material（更底层的 AsBindGroup / pipeline 控制）：
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader_advanced/manual_material.rs`
  - WGSL：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/assets/shaders/manual_material.wgsl`
- 自定义顶点属性（vertex attributes → shader locations）：
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader_advanced/custom_vertex_attribute.rs`
  - WGSL：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/assets/shaders/custom_vertex_attribute.wgsl`

提醒：

- 这些例子多以 3D 的 `Core3d` graph 为例；要适配 2D，把 `Core3d/Node3d` 换成 `Core2d/Node2d`。

## 1) 什么时候需要“自定义渲染管线”？

如果你遇到这些需求，Material API 往往不够：

- 需要在 tonemapping 后插一个屏幕空间 pass（后处理/自定义抗锯齿/色偏等）
- 需要读取主 pass 的颜色/深度/法线/运动矢量等纹理做效果
- 需要自定义 RenderPhase（非 Opaque/Transparent 的新阶段）
- 需要按你的规则决定 pipeline layout / shader defs / targets / depth state
- 需要绑定纹理数组（bindless）或做特殊资源绑定

推荐先判断哪类 example 最接近你的目标，再沿着那条链路抄。

## 2) RenderApp 的“工程化工作流”（你写自定义渲染时基本都这么走）

核心事实：

- 绝大多数渲染代码在 `RenderApp`（渲染子世界）中执行
- 主世界的组件/资源需要 **Extract** 到渲染世界，才能被渲染系统读取

典型步骤（对应 `custom_post_processing.rs` 的结构）：

1. 在主世界定义“控制渲染行为”的组件/资源（例如 PostProcessSettings、marker component）。
2. 用 `ExtractComponentPlugin<T>` / `ExtractResourcePlugin<T>` 把它们每帧提取到渲染世界。
3. 在 RenderStartup 初始化渲染世界资源（pipeline layout、sampler、CachedRenderPipelineId 等）。
4. 在 RenderGraph 插入节点（`add_render_graph_node::<ViewNodeRunner<MyNode>>(Core3d, ...)`），并加 edges 控制执行顺序。
5. 在节点 `run()` 里编码 draw/copy/dispatch 等命令。

看源码时优先关注这些模块：

- `bevy_render`：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_render/src`
  - `render_graph`：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_render/src/render_graph`
  - `render_phase`：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_render/src/render_phase`
  - `render_resource`：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_render/src/render_resource`
  - `extract_component` / `extract_resource`：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_render/src/extract_component`
  - `view`：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_render/src/view`

## 3) 模式 A：屏幕空间后处理（RenderGraph node + `ViewTarget::post_process_write()`）

“后处理 pass”最关键的 API 是 `ViewTarget::post_process_write()`：

- 它会给你一个 `source` + `destination`
- 你必须写入 `destination`（因为内部会翻转主纹理指向）

示例把关键 caveat 写得很清楚：

- `bind_group` 往往需要在 node `run()` 里按帧创建（因为 source/destination 会 ping-pong）

直接对照：

- Rust：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader_advanced/custom_post_processing.rs`
- WGSL：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/assets/shaders/post_processing.wgsl`

## 4) 模式 B：Specialized mesh pipeline（不走 Material，直接控制 mesh 渲染）

如果你想“自定义 mesh 的 pipeline”，但仍复用 Bevy 的 mesh 资产与 batching/visibility 等能力：

- 使用 `SpecializedMeshPipeline` + `SpecializedMeshPipelines`
- 在 Queue 阶段为 view/mesh 选择 pipeline（并缓存）
- 把实体放进某个 render phase（例如 `Opaque3d`）
- 用自定义 `RenderCommand` 组合（SetItemPipeline/SetMeshViewBindGroup/DrawMesh…）

关键点（示例里都有）：

- marker 组件需要 `ExtractComponent`
- 为了让 `check_visibility` 知道要检查这类实体，需要在 `on_add` hook 里添加 `VisibilityClass`

直接对照：

- Rust：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader_advanced/specialized_mesh_pipeline.rs`
- WGSL：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/assets/shaders/specialized_mesh_pipeline.wgsl`

## 5) 模式 C：Depth-only camera → 深度纹理采样（copy depth texture）

这是一个很“工程化”的示范：当你需要在 shader 里采样深度（做 SSR/自定义雾/软粒子等）但又不想侵入主 pipeline：

- 创建一个 `RenderTarget::None { size }` 的 camera（不输出颜色，只生成深度）
- 在 RenderGraph 节点里把 `ViewDepthTexture` copy 到一个 `Image`（让主世界也能拿到 handle）
- 用自定义 `Material` 在另一个 mesh 上显示/使用这张深度纹理

关键限制（示例里讲得很清楚）：

- wgpu 不允许同时 render 到深度纹理并采样它，所以必须 copy
- `RenderTarget::None` 时要显式指定 size，否则无法分配深度 buffer
- 多相机时用 `Camera.order` 控制先后

直接对照：

- Rust：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader_advanced/render_depth_to_texture.rs`
- WGSL：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/assets/shaders/show_depth_texture_material.wgsl`

## 6) 模式 D：Texture binding array（Bindless / `binding_array<texture_2d<f32>>`）

当你需要在 shader 里从一组纹理里做“非一致索引采样”（例如 tile set、材质库），可以用 binding array：

- 需要 GPU 支持特定 `WgpuFeatures`（示例在 RenderStartup 里检查，不支持就退出）
- `AsBindGroup` 需要“手写”到能组出 `&[&TextureView]` 形式（示例解释了为什么）
- WGSL 侧使用 `binding_array<texture_2d<f32>>` 并 non-uniform index

直接对照：

- Rust：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader_advanced/texture_binding_array.rs`
- WGSL：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/assets/shaders/texture_binding_array.wgsl`

## 7) WGSL 与资源绑定：从 “Material API” 到 “render_resource”

你会同时遇到两套“绑定资源”的方式：

- 高层：`AsBindGroup` 派生 + `Material`（见 `shaders_materials.md`）
- 低层：手写 `BindGroupLayoutDescriptor`/`BindGroupEntries`/`RenderPipelineDescriptor`（见 `custom_post_processing.rs`、`specialized_mesh_pipeline.rs`）

当你需要 `texture_2d_array` / array texture、atlas 等纹理组织方式：

- 纹理数组与加载设置见：`mesh_image_color.md`（含 `examples/shader/array_texture.rs`）

## 8) Debug / 排错建议

自定义渲染最常见的问题是“节点没跑/资源没提取/管线没缓存/纹理 usages 不对/feature 不支持”。

建议排查顺序：

1. 先确认组件/资源是否被 Extract 到 RenderApp（ExtractComponent/ExtractResource 是否安装）。
2. 再确认 RenderGraph edges 插入位置是否正确（是否在你想要的 Node3d/Node2d 之后）。
3. 再看 PipelineCache 是否拿到了 pipeline（否则可能还在编译或 shader 路径错误）。
4. 最后用 wgpu trace / shader error 输出定位（见 `debug_profiling_devtools_remote.md` + `docs/debugging.md`）。


# Rendering 架构（Bevy v0.18）：RenderApp / Extract / Prepare / Queue / Render

这篇用于理解 Bevy 的渲染“为什么这么写”，以及你要做自定义渲染时该从哪里切入。

## 0) 关键入口（先读这些）

- RenderPlugin 顶部文档与 `RenderSystems`/`Render` schedule：
  - Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_render/src/lib.rs`
- Main schedule 对渲染的说明（渲染在 SubApp 中执行）：
  - Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_app/src/main_schedule.rs`
- Debug/Profiling 与 wgpu trace：
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/debugging.md`
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/profiling.md`

## 1) 核心事实：渲染在独立的 SubApp（RenderApp）里跑

Bevy 默认渲染后端（wgpu）并不在主世界（Main world）直接执行：

- 主世界负责 gameplay/ECS 逻辑（Main schedule）
- 渲染世界（Render world / RenderApp）在两次 Main schedule 之间与主世界交换数据

含义：

- 主世界里改了组件，不一定“立刻”反映到这一帧渲染：通常要经过 Extract（抽取）阶段。
- 你在写自定义渲染系统时，经常要处理“主世界数据如何映射到渲染世界 GPU 资源”的问题。

## 2) Render schedule 的阶段化（粗粒度）

`bevy_render` 定义了渲染主调度 `Render`，并用 `RenderSystems` 这一组 SystemSet 做阶段划分：

- ExtractCommands
- PrepareAssets / PrepareMeshes / ManageViews
- Queue（把可绘制实体放进 render phases）
- PhaseSort
- Prepare（创建/更新 bind group、buffer、texture 等 GPU 资源）
- Render（真正发 draw/dispatch）
- Cleanup / PostCleanup

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_render/src/lib.rs`（`pub enum RenderSystems` 与 `Render::base_schedule()`）

实践含义：

- “我想在渲染前准备 GPU 资源”：看 Prepare 阶段的系统集与对应插件。
- “我想改变渲染顺序/插入后处理”：看 RenderGraph（下节）。

## 3) RenderGraph：渲染不是单管线，而是图

Bevy 把渲染组织成图（RenderGraph），不同 pipeline（2D/3D/PBR/UI）会向图里注册节点/边。

- 模块入口：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_render/src/render_graph`
- Core pipeline 插件：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_core_pipeline/src/lib.rs`
- PBR 图节点枚举：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_pbr/src/lib.rs`（`graph::NodePbr`）

当你需要：

- 自定义后处理 pass
- 增加新的渲染阶段（prepass、compute pass、屏幕空间效果）

就从 RenderGraph 切入。

## 4) 渲染相关的调试与可观测性

### wgpu trace（定位驱动/验证层问题）

Bevy 文档给了“如何抓 wgpu trace”的步骤：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/debugging.md`（WGPU Tracing）

关键点：

- `RenderPlugin` 的 `render_creation` / `WgpuSettings.trace_path` 能指定 trace 输出路径
- 需要给 `wgpu` 开 `trace` feature

### profiling（CPU/GPU）

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/profiling.md`

建议流程：

1. 先开 `trace`，看 ECS 系统与渲染阶段耗时分布
2. CPU bottleneck vs GPU bottleneck 分开分析（Tracy / vendor tools）

### 环境变量（选择 adapter / 关闭 validation / 输出详细 shader error）

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_render/src/lib.rs` 顶部列了一组 `WGPU_*` 与 `VERBOSE_SHADER_ERROR`

## 5) “我只是想改材质/写 shader”，需要懂这么多吗？

不需要。

大部分用户侧渲染扩展，优先走 “材质系统”：

- 3D：`Material` + `MaterialPlugin<T>` + `MeshMaterial3d<T>`
- 2D：`Material` + `MaterialPlugin<T>` + `MeshMaterial2d<T>`

看：

- `shaders_materials.md`
- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader/shader_material.rs`

只有当你要写“自定义渲染管线/图节点/相机视图管理”时，才需要深入 RenderApp/RenderGraph。

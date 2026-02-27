# 总览：Bevy v0.18 的 crate 结构与默认插件组成

这份文档用于“建立全局地图”：Bevy 的模块边界在哪里？DefaultPlugins 实际装了什么？哪些功能由哪些 crate 提供？

## 1) `bevy` / `bevy_internal` / `bevy::prelude::*`

- `bevy` 是一个**容器 crate**：把大量子 crate 重新导出，方便用户只依赖一个 crate。
  - Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/src/lib.rs`
- `bevy_internal` 是内部聚合层：按 feature re-export 各子 crate，并提供 `DefaultPlugins` / `MinimalPlugins`。
  - Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_internal/src/lib.rs`
  - Prelude：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_internal/src/prelude.rs`

实践建议：

- “快速写原型”：`use bevy::prelude::*;` 足够覆盖大多数常用类型（App/ECS/Time/Transform/默认插件等）。
- “写库/做裁剪”：直接依赖 `bevy_app` / `bevy_ecs` / `bevy_asset` 等子 crate 会更可控（但更啰嗦）。

## 2) `DefaultPlugins` 里包含什么？顺序有什么含义？

`DefaultPlugins` 是一个 `PluginGroup`，它会按固定顺序把常用功能装进 App（并受 Cargo features 影响）。

- Source（权威列表与顺序）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_internal/src/default_plugins.rs`

你会看到一些非常“有意为之”的顺序说明，例如：

- WebAssetPlugin 需要在 AssetPlugin 之前注册 http/https sources。
- WinitPlugin 需要在 AssetPlugin 之后（例如自定义鼠标光标依赖资产加载）。
- 渲染相关（RenderPlugin / ImagePlugin / MeshPlugin / CorePipeline / PBR / UI render 等）依赖链很长，通常不要手动重排，除非你知道你在做什么。

`MinimalPlugins` 则是“最小可运行骨架”，更多面向 headless / server / 工具型程序：

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_internal/src/default_plugins.rs`

## 3) Cargo features 与“profiles/collections”

Bevy 暴露了大量 feature 来裁剪功能与依赖（影响编译时间、二进制体积、平台支持）。

- Features 总文档（来自源码仓库）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/cargo_features.md`

里面把 feature 分成：

- Profiles（高层组合）：`2d` / `3d` / `ui` / `default`
- Collections（中层组合）：`default_app` / `default_platform` / `scene` / `audio` / `picking` / `dev` 等

建议：

- 如果你做的是普通 2D/3D 游戏原型：先用默认（`bevy = "0.18"`）。
- 如果你在做“服务端模拟 / 工具 / 纯 ECS”并且不需要渲染：从 `default-features = false` + `default_app`（再按需加 `asset`/`scene`/`audio` 等）起步。

## 4) 常见 crate 速查（按领域）

按 `bevy_internal` 的 re-export 与 DefaultPlugins 的构成，可以把 Bevy 拆成这些“面向用户”的核心域：

- App 与调度：
  - `bevy_app`：`App`、`Plugin`、`PluginGroup`、主调度标签（`Update`/`FixedUpdate` 等）
  - `bevy_state`：状态机与状态切换调度（`StateTransition` / `OnEnter` / `OnExit` / `NextState`）
- ECS：
  - `bevy_ecs`：Entity/Component/Resource/Query/SystemParam、Schedule、Observers/Events/Messages、Relationships
- 资产与场景：
  - `bevy_asset`：`AssetServer` / `Handle` / `Assets<T>`、loader/saver、watcher、asset sources
  - `bevy_scene`：`Scene` / `DynamicScene` / `SceneSpawner`、（序列化 feature 下）场景加载与 spawn
  - `bevy_gltf`：glTF loader、`GltfAssetLabel`、glTF 子资源（mesh/scene/skin/animation）
  - `bevy_reflect`：`Reflect`、type registry、反射序列化（DynamicScene 也依赖它）
- 渲染（wgpu 后端）：
  - `bevy_render`：Render 子世界（RenderApp/SubApp）、Extract/Prepare/Queue/Render pipeline、RenderGraph、Wgpu settings
  - `bevy_core_pipeline`：核心 2D/3D 管线、tonemapping/upscaling/prepass/oit 等
  - `bevy_pbr`：3D PBR、StandardMaterial、灯光/阴影/雾/SSR/SSAO 等
  - `bevy_sprite`：2D sprite、Text2d、（可选）sprite picking backend
  - `bevy_ui` / `bevy_ui_render` / `bevy_ui_widgets`：UI 树、布局、交互、渲染与 widgets
  - `bevy_shader`：Shader 资产、`load_shader_library!` 宏
- 交互与运行时：
  - `bevy_input`：`ButtonInput<T>`、键鼠手柄触摸消息流与系统集
  - `bevy_window` / `bevy_winit`：窗口与 winit 后端、窗口事件消息
  - `bevy_time`：`Time<Real/Virtual/Fixed>`、FixedMainLoop 驱动、time update strategy
  - `bevy_audio`：AudioPlayer/AudioSource/Spatial audio
  - `bevy_animation`：AnimationClip/Player/Graph、动画事件
  - `bevy_picking`：Pointer events + backends + observers（与 UI/mesh/sprite 集成）
- 开发者工具：
  - `bevy_diagnostic`：frame time / entity count 等 diagnostics
  - `bevy_gizmos`：即时调试绘制
  - `bevy_dev_tools`：fps overlay、state logging、frame graph 等
  - `bevy_remote`：Bevy Remote Protocol（JSON-RPC）远程读写 ECS

## 5) 与精炼文档的对照（Context7）

Context7 数据集 `/websites/rs_bevy` 能快速检索 docs.rs 的核心 API：

- `AssetServer::load` / `GltfAssetLabel` 的典型用法
- `Event` / `Observer` / `World::trigger` 的基本模型
- `App::new().add_plugins(DefaultPlugins)` 的启动骨架

但当你需要确定“某个系统集在哪个 schedule”“DefaultPlugins 的具体顺序”“某 feature 的真实效果”时，依然以 `https://github.com/bevyengine/bevy/tree/v0.18.0` 为准。

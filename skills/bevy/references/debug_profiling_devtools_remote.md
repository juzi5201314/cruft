# Debug / Profiling / DevTools / Remote（Bevy v0.18）

这篇把“开发者体验”相关能力串起来：诊断、gizmos、trace/profiling、远程协议（BRP）。

## 0) 入口

- Debugging（wgpu trace）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/debugging.md`
- Profiling（Tracy/Chrome trace/GPU profiling）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/profiling.md`
- DiagnosticsPlugin：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_diagnostic/src/lib.rs`
- GizmoPlugin：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_gizmos/src/lib.rs`
- Dev tools：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_dev_tools/src/lib.rs`
- Remote（BRP）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_remote/src/lib.rs`

## 1) Diagnostics：帧率/帧时间/实体计数等

Bevy 提供 diagnostics 插件集合：

- `FrameCountPlugin`
- `FrameTimeDiagnosticsPlugin`
- `EntityCountDiagnosticsPlugin`
- `LogDiagnosticsPlugin`（把诊断信息输出到 log）

入口：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_diagnostic/src/lib.rs`

用法通常是加插件：

```rust
app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin);
```

（具体配法以 docs.rs 或源码为准）

## 2) Gizmos：即时调试绘制（强烈推荐）

Gizmos 让你在场景里画线/圆/箭头等，适合调试：

- 物理碰撞体、射线、路径、导航网格、空间索引等

入口与示例：

- crate：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_gizmos/src/lib.rs`
- examples：`https://github.com/bevyengine/bevy/tree/v0.18.0/examples/gizmos`
- observer 示例里也用到了 gizmos：
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/observers.rs`（`Gizmos` 画 mine 圆）

## 3) Tracing / Profiling：先定位瓶颈再优化

推荐读完：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/profiling.md`

关键点：

- `trace` feature：开启 Bevy 内置 spans（ECS 系统、渲染阶段）
- Tracy：`trace_tracy` / `trace_tracy_memory`
- Chrome tracing：`trace_chrome`
- GPU profiling：优先用厂商工具（Nsight/RGP/Intel/Apple）

别用 RenderDoc 当 profiler（文档里明确说了）。

## 4) wgpu trace：渲染错误/验证层问题的杀手锏

抓 trace 的步骤在：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/debugging.md`

你会用到：

- `bevy_render::settings::WgpuSettings.trace_path`
- 或 RenderPlugin 的构造参数（按你是否手动创建 renderer）

## 5) Dev tools：FPS overlay / frame graph / states 工具

`bevy_dev_tools` 提供一些开发期功能：

- fps overlay
- frame_time_graph
- states helpers（例如打印状态切换）

入口：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_dev_tools/src/lib.rs`

示例（states）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/state/states.rs`（`bevy::dev_tools::states::log_transitions`）

## 6) Remote（BRP）：远程检查与修改 ECS（JSON-RPC）

bevy_remote 提供 Bevy Remote Protocol：

- 可以在外部工具里查询 world、读写组件、运行 query 等
- 基于 JSON-RPC 2.0

入口文档非常完整：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_remote/src/lib.rs`

examples：

- server：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/remote/server.rs`
  - 启用方式：`--features="bevy_remote"`
  - 加 `RemotePlugin` + `RemoteHttpPlugin`
- client：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/remote/client.rs`

实践提醒：

- 这是强大的调试工具，但也意味着“远程可修改你的 ECS 状态”，要注意暴露面与安全边界。

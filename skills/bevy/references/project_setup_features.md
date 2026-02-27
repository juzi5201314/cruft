# 项目创建与 Cargo features（Bevy v0.18）

这份文档聚焦“如何把 Bevy 以正确的 feature 组合装进项目”，以及开发期常见开关（热重载、trace、平台后端）。

## 1) 最小可跑骨架

最常见（全默认）：

```toml
[dependencies]
bevy = "0.18"
```

对应的最小入口：

```rust
use bevy::prelude::*;

fn main() {
    App::new().add_plugins(DefaultPlugins).run();
}
```

## 2) 裁剪编译：`default-features = false` + profiles/collections

Bevy 提供“profiles/collections”帮助你只编译需要的子系统：

- Source（完整列表）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/cargo_features.md`

示例：只启用 2D profile（仍包含 UI/scene/audio/picking 等 2D 默认体验）：

```toml
[dependencies]
bevy = { version = "0.18", default-features = false, features = ["2d"] }
```

示例：headless / server / 工具（只要 App/ECS/Time 等，不要渲染）：

```toml
[dependencies]
bevy = { version = "0.18", default-features = false, features = ["default_app"] }
```

注意：

- 想用资产系统：需要 `bevy_asset`（通常被 profiles/collections 间接启用）。
- 想加载 glTF：需要 `bevy_gltf`（以及渲染相关 feature 才能真正显示）。
- 想跑窗口：需要 `bevy_winit` + 对应平台后端（x11/wayland 等）。

## 3) 开发期常用 feature 开关

这些 feature 往往只在开发期启用：

- 热重载：`file_watcher`
  - 关联概念见 `assets.md`
  - Doc：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/cargo_features.md`（`file_watcher` 条目）
- tracing/性能分析：
  - `trace`：开启 Bevy 内部 spans
  - `trace_tracy` / `trace_tracy_memory`：Tracy
  - `trace_chrome`：Chrome tracing 格式
  - 参考：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/profiling.md`
- 调试辅助：
  - `bevy_dev_tools`：FPS overlay、frame graph、state 工具等
  - `bevy_gizmos` / `bevy_gizmos_render`：gizmos
  - 参考：`debug_profiling_devtools_remote.md`
- 远程检查：
  - `bevy_remote`：Bevy Remote Protocol（BRP）
  - examples：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/remote/server.rs`

## 4) 平台与窗口后端（Linux/Windows/macOS/Wasm/Android）

不同平台的窗口/输入后端由 feature 控制（常见：`x11`、`wayland`、`webgl2`、`webgpu`、Android activity 选择等）。

- 参考：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/linux_dependencies.md`
- Wasm 示例与说明：`https://github.com/bevyengine/bevy/tree/v0.18.0/examples/wasm` + `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/README.md` 中的 Wasm 小节

## 5) 渲染相关环境变量（调试 wgpu）

当你在排查渲染问题或需要不同 adapter/limits 时，这些环境变量很常用：

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_render/src/lib.rs`（顶部注释）
  - `WGPU_DEBUG=1`
  - `WGPU_VALIDATION=0`
  - `WGPU_FORCE_FALLBACK_ADAPTER=1`
  - `WGPU_ADAPTER_NAME=...`
  - `WGPU_SETTINGS_PRIO=webgl2|compatibility`
  - `VERBOSE_SHADER_ERROR=1`

## 6) 版本与硬切换提醒

本 skill 只面向 Bevy v0.18。

- 如果用户给的是旧代码（例如旧的 event API、旧的 UI/渲染类型名等），不要给“兼容写法”，直接按 v0.18 改写并解释改写点。

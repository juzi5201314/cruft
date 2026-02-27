# Logging / Tracing（Bevy v0.18）

这篇聚焦 Bevy 的日志与 tracing 体系（`bevy_log`）：如何配置 `LogPlugin`、如何用 `RUST_LOG` 精确筛选、以及如何加自定义 layer（例如把 ECS/渲染 spans 送进 profiler）。

> 性能分析与 trace 开关见 `debug_profiling_devtools_remote.md` 与 `docs/profiling.md`；本篇更偏“如何把日志系统用对、用稳”。

## 0) 入口与 examples

源码入口：

- `bevy_log` crate 文档（强烈建议读顶部注释）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_log/src/lib.rs`
- DefaultPlugins 中 LogPlugin 的位置与开关：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_internal/src/default_plugins.rs`

关键 examples：

- Logs（最小配置、RUST_LOG 说明）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/app/logs.rs`
- 自定义 tracing layers：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/app/log_layers.rs`
- ECS + tracing layer 的组合示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/app/log_layers_ecs.rs`
- 纯 ECS 场景下只用 LogPlugin：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/system_piping.rs`
- 移动端示例里也展示了 LogPlugin 的典型写法：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/mobile/src/lib.rs`

## 1) 基本事实：Bevy 的日志宏就是 `tracing` 宏

`bevy_log` re-export 了 `tracing` 的宏与类型（`info!`/`warn!`/`debug!`/`trace!` 等），并通过 `LogPlugin` 默认装配一个合适的 subscriber（桌面 stdout、Android logcat、Wasm 浏览器 console 等）。

权威说明见：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_log/src/lib.rs`

## 2) 项目级配置：`DefaultPlugins.set(LogPlugin { ... })`

典型（也是 examples 的写法）：

- 把 `LogPlugin` 通过 `DefaultPlugins.set(...)` 覆盖默认配置
- 重点字段：
  - `level`：基础日志级别
  - `filter`：更细粒度过滤（推荐按 crate/module 配）

示例：

- `examples/app/logs.rs`
- `examples/mobile/src/lib.rs`（移动端常用）

## 3) 运行期配置：`RUST_LOG` 与 `NO_COLOR`

当你需要“不开改代码就切换日志级别/过滤规则”：

- 用 `RUST_LOG`（语法与 `tracing_subscriber::EnvFilter` 一致）
- 例如只看渲染与 ECS 的 trace：
  - `RUST_LOG=wgpu=error,bevy_render=info,bevy_ecs=trace`

禁用彩色输出：

- `NO_COLOR=1`

这些规则与示例都在 `bevy_log` crate 文档中写明：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_log/src/lib.rs`

## 4) 高级：自定义 tracing layers（把数据送去你自己的系统）

当你需要：

- 把 spans/event 输出到文件
- 做结构化日志（JSON）
- 把关键系统的 spans 送去 profiler

就需要自定义 layer。建议直接从示例抄骨架：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/app/log_layers.rs`
- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/app/log_layers_ecs.rs`

实践建议：

- 保持 `filter` 简洁：先收窄到你关注的 crate，再逐步放宽
- 小心 `trace` 级别的日志量：容易影响性能并淹没信息

## 5) “为什么我看不到日志？”

按优先级排查：

1. 你是否装了 `LogPlugin`？
   - DefaultPlugins 默认包含（见 `default_plugins.rs`），但如果你用的是 `MinimalPlugins`/自定义插件组，可能没加。
2. `RUST_LOG` 是否覆盖了 `LogPlugin` 的设置？
   - `bevy_log` 文档明确：一旦设置 `RUST_LOG`，LogPlugin 的设置会被忽略。
3. Wasm/Android 的输出位置是否正确？
   - Wasm：浏览器 devtools console
   - Android：logcat（示例见 `examples/mobile/src/lib.rs`）

## 6) 与精炼文档的对照（Context7）

Context7 `/websites/rs_bevy` 适合快速查：

- `LogPlugin` 字段
- `info!/warn!/debug!/trace!` 的 re-export 路径

但“默认平台行为、RUST_LOG 覆盖规则、layer 组合方式”，以 `bevy_log` crate 文档与 examples 为准。


# 平台与窗口后端：Winit / Android / Wasm（Bevy v0.18）

这篇聚焦“把程序跑在不同平台上”时最常踩的坑与权威入口：

- 桌面窗口与事件循环：`bevy_winit`
- Android：`#[bevy_main]`、AndroidApp、移动端设置
- Wasm：构建/绑定/运行（WebGL2/WebGPU）、体积与音频限制

> 输入/窗口事件模型见 `input_window.md`；Cargo features 选型见 `project_setup_features.md`；调试与性能见 `debug_profiling_devtools_remote.md`。

## 0) 入口（源码/文档/工具）

源码入口：

- `bevy_winit`（WinitPlugin、runner、窗口创建与系统集）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_winit/src/lib.rs`
- `bevy_android`（AndroidApp 入口）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_android/src/lib.rs`

官方文档（来自源码仓库）：

- Linux 依赖：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/linux_dependencies.md`
- Examples README（包含 Mobile/Wasm 的权威步骤）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/README.md`

Wasm 构建工具（仓库内工具）：`https://github.com/bevyengine/bevy/tree/v0.18.0/tools/build-wasm-example`

示例：

- Mobile 示例（iOS/Android，触摸与窗口设置）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/mobile/src/lib.rs`
- Wasm 目录（HTML/静态资源）：`https://github.com/bevyengine/bevy/tree/v0.18.0/examples/wasm`

## 1) 桌面：`bevy_winit::WinitPlugin` 与 runner

你需要记住的事实：

- `WinitPlugin` 会替换 `App` 的 runner：驱动 winit `EventLoop`，并把 OS 的窗口/输入事件同步到 Bevy 的消息/资源模型里。
- 它还会装配 AccessKit（a11y）与 cursor 支持（见 `bevy_winit` 源码）。

入口：

- `WinitPlugin` 文档与 build：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_winit/src/lib.rs`

平台差异：

- Linux/Windows 可选 `run_on_any_thread`（并受 x11/wayland feature 与平台限制影响）
- 原始 winit window event 可通过 `RawWinitWindowEvent` 作为“逃生口”获取（但已被 Bevy 主循环处理过）

## 2) Linux：依赖与 feature 组合（x11/wayland）

Linux 上的坑通常不是 Bevy API，而是系统依赖与 feature：

- 依赖列表与安装方式：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/linux_dependencies.md`

工程建议：

- CI/容器里先把依赖装对，再谈渲染问题
- 若你裁剪 features（`default-features = false`），确认你还启用了 `bevy_winit` 与对应后端 feature（x11/wayland）

## 3) Android：`#[bevy_main]` 与 `WinitPlugin` 的要求

Android 上的关键点在 `bevy_winit` 源码里写得很直接：

- Winit 的 event loop 需要 Android app handle
- Bevy 需要通过 `#[bevy_main]` proc-macro 生成 boilerplate 并初始化 `ANDROID_APP`

相关入口：

- `bevy_android::ANDROID_APP`：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_android/src/lib.rs`
- `bevy_winit` 中 Android 分支（查 `target_os = \"android\"`）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_winit/src/lib.rs`

最推荐直接抄的工程模板：

- Mobile 示例（含 `#[bevy_main]`、LogPlugin/WindowPlugin 配置、`WinitSettings::mobile()`）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/mobile/src/lib.rs`

实践提醒（示例里也写了）：

- 某些 Android 设备对 MSAA/阴影等功能可能更脆弱：先从示例的保守配置跑通，再逐步加效果。

## 4) Wasm：构建、绑定、运行（WebGL2/WebGPU）

权威步骤在 examples README 的 “Wasm” 小节：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/README.md`

要点（按 README）：

- target：`wasm32-unknown-unknown`
- 构建：`cargo build --release --example <name> --target wasm32-unknown-unknown`
- 绑定：用 `wasm-bindgen` 生成 JS glue
- 运行：用静态服务器把 `examples/wasm` 目录 serve 给浏览器

WebGL2 vs WebGPU：

- WebGPU 仍是实验性，需要 `webgpu` feature（会覆盖 `webgl2`）
- 仓库提供了 helper：`build-wasm-example`（README 有命令示例）

相关工具目录：

- `https://github.com/bevyengine/bevy/tree/v0.18.0/tools/build-wasm-example`

浏览器音频限制：

- 用户交互前通常不能自动播放音频（README 专门说明了）
- 单线程限制可能导致 stutter（需结合具体浏览器评估）

## 5) 与精炼文档的对照（Context7）

Context7 `/websites/rs_bevy` 更适合查“类型与 API”，例如：

- `WinitPlugin`、`WinitSettings`
- `WindowPlugin` / `Window` 配置项

平台部署步骤（Android/Wasm）以 examples README 与示例工程为准。


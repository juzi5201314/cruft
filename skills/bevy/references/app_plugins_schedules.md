# App / Plugins / Schedules（Bevy v0.18）

本篇聚焦 Bevy 应用层：`App` 的生命周期、插件系统、主调度（Main schedule）结构、Fixed timestep、SubApp（尤其 RenderApp）。

## 1) `App`：你真正操控的对象

Bevy 的用户侧入口几乎总是：

```rust
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, update)
        .run();
}
```

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_app/src/app.rs`
- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/hello_world.rs`

关键点：

- `App::new()` 等价于 `App::default()`，会安装核心结构（含 MainSchedulePlugin、消息更新等）。
- `App::empty()` 更“裸”，适合你要深度自定义 runner / schedules / cleanup 策略时用。

## 2) Plugins / PluginGroup：模块化的基本单位

插件与插件组是 Bevy 的组织方式：所有“引擎功能”都是插件组合的结果。

- 自定义 Plugin 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/app/plugin.rs`
- 自定义 PluginGroup 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/app/plugin_group.rs`
- DefaultPlugins/MinimalPlugins 组成：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_internal/src/default_plugins.rs`

典型模式：

- 小功能写成 `Plugin`（把资源 + 系统 + 资产类型注册聚在一起）。
- 大功能写成 `PluginGroup`（把一组 plugin 以固定顺序装配起来）。

## 3) Main schedule（一次 update 里发生了什么）

Bevy v0.18 的主调度结构在 `bevy_app::main_schedule` 文档里写得很清晰。

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_app/src/main_schedule.rs`

你应该记住的“粗粒度”顺序：

1. 首次运行时：`PreStartup` → `Startup` → `PostStartup`
2. 每帧（每次 `App::update()`）：
   - `First`
   - `PreUpdate`
   - `StateTransition`（如果启用了 bevy_state）
   - `RunFixedMainLoop`（可能跑 0..N 次 FixedMain）
   - `Update`
   - `SpawnScene`
   - `PostUpdate`
   - `Last`

渲染不是 Main schedule 的一部分：

- 默认渲染在一个独立的 `SubApp`（RenderApp）里执行，并在 Main schedule 迭代之间交换数据。
- 参考：`main_schedule.rs` 的 Rendering 说明、`rendering_architecture.md`

## 4) Fixed timestep：`RunFixedMainLoop` / `FixedUpdate`

Bevy 的 fixed update 不是一个“单独的 while”，而是被 `RunFixedMainLoop` 调度驱动的：

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_app/src/main_schedule.rs`（Fixed* labels）
- Time 驱动细节：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_time/src/lib.rs`（`run_fixed_main_schedule` / `TimeUpdateStrategy`）
- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/movement/physics_in_fixed_timestep.rs`

经验法则：

- “必须固定频率”的逻辑放 `FixedUpdate`（物理、确定性规则等）。
- “每帧一次”的逻辑放 `Update`（输入/UI/音频控制等）。

## 5) 系统顺序控制：`before/after`、sets、`chain()`、`run_if`

Bevy v0.18 常用的“顺序控制”工具：

- `.before(x)` / `.after(x)`：显式依赖
- `SystemSet` + `configure_sets(...).chain()`：把一串 set 变成有序阶段
- `run_if(condition)`：运行条件（尤其与 states/input 搭配）

最完整的“讲解型示例”：

- ECS guide：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/ecs_guide.rs`
  - 展示 `SystemSet`、`configure_sets(...).chain()`、`(a,b).chain()`、以及 `Last` schedule。

State 的 `run_if` 示例：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/state/states.rs`

Input condition 示例（common_conditions）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/remote/server.rs`（`input_just_pressed`）

## 6) Runner：窗口化 vs headless

`App::run()` 会调用 runner。常见 runner：

- 窗口化：通常由 `bevy_winit::WinitPlugin` 设置 runner
- headless：`ScheduleRunnerPlugin`（每隔一段时间 `app.update()`）

示例：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/ecs_guide.rs` 使用 `ScheduleRunnerPlugin::run_loop(...)`
- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/app/headless.rs` / `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/app/headless_renderer.rs`

## 7) SubApp：多世界并行（RenderApp 是关键）

Bevy 的 `App` 内可以包含多个 `SubApp`，其中最重要的是 `RenderApp`（渲染子世界）。

何时需要关心 SubApp？

- 你要写自定义渲染（Extract/Prepare/Queue/Render）或插入 RenderGraph 节点。
- 你在解释为什么主世界里改了组件，渲染要下一帧才看到（Extract 延迟）。

参考：

- `rendering_architecture.md`
- RenderPlugin 注释：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_render/src/lib.rs`

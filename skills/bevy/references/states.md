# States（Bevy v0.18）：高层控制流与状态切换

Bevy 的 states 是“全局有限状态机（FSM）”，用来管理游戏/应用的大尺度流程：Menu/Loading/InGame/Paused 等。

## 0) 入口与源码

- states crate 文档总览：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_state/src/lib.rs`
- App 扩展（init_state/insert_state/add_*）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_state/src/app.rs`
- 示例：
  - 基础：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/state/states.rs`
  - SubStates：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/state/sub_states.rs`
  - ComputedStates：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/state/computed_states.rs`

## 1) 基础 States：`State<S>` + `NextState<S>`

定义：

```rust
#[derive(States, Default, Clone, Copy, Eq, PartialEq, Hash, Debug)]
enum AppState {
    #[default]
    Menu,
    InGame,
}
```

初始化：

- `app.init_state::<AppState>()`（或 `insert_state(AppState::Menu)`）

切换：

- 修改 `NextState<AppState>`（`next_state.set(AppState::InGame)`）

运行条件：

- `run_if(in_state(AppState::Menu))`

示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/state/states.rs`

## 2) 过渡调度：`OnEnter` / `OnExit` / `OnTransition`

states 不是靠“每帧 if 判断”来跑 setup/cleanup，而是提供独立 schedule：

- `OnEnter(S::Variant)`：进入某具体状态时跑
- `OnExit(S::Variant)`：退出某具体状态时跑
- `OnTransition<S>`：更通用的过渡钩子（按需求）

这些系统在 `StateTransition` 调度里执行（插入点在 `Update` 之前）。

主调度插入点与顺序，见：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_app/src/main_schedule.rs`（`StateTransition` 的说明）

## 3) SubStates：状态只在“父状态满足条件”时存在

SubStates 用于更复杂的层级状态：

- 只有在 source state 满足某个值时，该子状态资源才存在
- 适合：Paused 只在 InGame 时存在、战斗子状态只在 InCombat 时存在

示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/state/sub_states.rs`

关键语法：

- `#[derive(SubStates)]`
- `#[source(AppState = AppState::InGame)]` 指定依赖
- `app.add_sub_state::<IsPaused>()` 安装

### scoped_entities：按状态自动管理实体生命周期

SubStates 示例启用了：

- `#[states(scoped_entities)]`
- `DespawnOnExit(IsPaused::Paused)`：退出 Paused 时自动 despawn 标记的 UI

相关类型导出在：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_state/src/lib.rs`（`state_scoped` / `DespawnOnEnter/DespawnOnExit`）

## 4) ComputedStates：从其它状态推导出来的“视图状态”

当你不想在一个大 enum 里列出所有组合，而是希望“从多个 source state 推导”，用 `ComputedStates`：

- 典型：`InGame` 是 marker（ZST），存在与否代表是否处在游戏中
- 典型：`TurboMode` 是 marker，由 `AppState` 的字段推导
- 典型：`IsPaused` 是 enum，由 `AppState` 推导为 Paused/NotPaused
- 甚至可以从多个 state + Option state 推导（示例里 `Tutorial`）

示例（强烈建议读）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/state/computed_states.rs`

安装：

- `app.add_computed_state::<MyComputedState>()`

重要限制：

- 不支持循环依赖（会编译失败）。

## 5) 运行时保障与调试

### StatesPlugin 是否安装

`AppExtStates` 的实现里会 warn：如果你使用 states API 但没装 `StatesPlugin`。

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_state/src/app.rs`（`warn_if_no_states_plugin_installed`）

DefaultPlugins 默认包含 states plugin（见 `bevy_internal/src/default_plugins.rs`）。

### Startup 前的 StateTransition

Bevy 有测试保证：`OnEnter(initial_state)` 会在 `PreStartup` 之前跑（用于“进入初始状态时先注册资源/组件”）。

- Source 测试：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_state/src/lib.rs`（`state_transition_runs_before_pre_startup`）

### 打印状态切换（dev tools）

示例里展示了：

- `bevy::dev_tools::states::log_transitions::<AppState>`（需 feature `bevy_dev_tools`）

参见：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/state/states.rs`

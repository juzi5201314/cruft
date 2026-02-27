# Time / Fixed timestep（Bevy v0.18）

Bevy 的时间系统同时支持：

- Real time：真实时间（通常来自系统时钟或渲染世界传回的 Instant）
- Virtual time：可暂停、可变速的“游戏时间”
- Fixed time：固定步长的逻辑时间（驱动 FixedUpdate）

## 0) 入口与 examples

- TimePlugin 与资源结构：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_time/src/lib.rs`
- examples：
  - `examples/time/time.rs`（强烈建议跑一次，理解三种时钟与 schedule）
  - `examples/time/timers.rs`
  - `examples/time/virtual_time.rs`

## 1) `Time` 资源的三个常用形态

在系统参数里你会常见：

- `Res<Time>`：默认（通常是 Virtual time 的视图，取决于上下文）
- `Res<Time<Real>>`
- `Res<Time<Virtual>>`
- `Res<Time<Fixed>>`

示例（在不同 schedule 打印不同 time）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/time/time.rs`

## 2) FixedUpdate：为何会“0 次或多次”

FixedUpdate 的执行由 `RunFixedMainLoop` 驱动，会根据累计的 real/virtual 时间决定这一帧跑几次 fixed step。

- 主调度结构：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_app/src/main_schedule.rs`
- TimePlugin 驱动 fixed loop：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_time/src/lib.rs`（`run_fixed_main_schedule`）

含义：

- 在性能较差的机器上，一帧可能跑多次 FixedUpdate（追赶时间）
- 在极端情况下，如果没有积累到足够的 fixed timestep，这一帧可能不跑 FixedUpdate

因此：

- FixedUpdate 系统必须保证“每次 step 都正确”，不要依赖“每帧必跑一次”的假设。

## 3) Timer：节流、CD、周期任务

Timer 是常用的“逻辑节拍器”：

- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/time/timers.rs`
- 在 plugin 示例里也出现：
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/app/plugin.rs`（`TimerMode::Repeating`）

## 4) 手动推进时间（测试/网络同步等）

TimePlugin 支持不同的更新策略：

- `TimeUpdateStrategy::Automatic`
- `ManualInstant` / `ManualDuration`
- `FixedTimesteps(n)`（一次 `App::update()` 跑固定 n 次 fixed step）

源码入口：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_time/src/lib.rs`（`TimeUpdateStrategy`）

这对：

- 单元测试
- 回放/确定性模拟
- 网络同步（锁步/快照）

很有帮助。

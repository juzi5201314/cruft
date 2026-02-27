# Events / Messages / Observers（Bevy v0.18）

Bevy v0.18 把“事件”拆成两套互补机制：

- **Events + Observers（push-based）**：触发时立即运行 observer（回调/系统），适合交互、层级冒泡、实体定向事件。
- **Messages（pull-based）**：把消息缓存在 `Messages<T>` 资源中，在 schedule 点批量读取/处理，适合高吞吐、可预测的分阶段处理。

## 1) Events：触发即执行（Observer 立即运行）

概念与 API：

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ecs/src/event/mod.rs`
- Observer：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ecs/src/observer/mod.rs`

基本流程：

1. 定义事件类型：`#[derive(Event)]`
2. 注册 observer：`world.add_observer(|on: On<MyEvent>| { ... })` 或对某个 entity `.observe(...)`
3. 触发事件：`World::trigger(event)` 或 `Commands::trigger(event)`

示例（全局 observer）：

```rust
use bevy::prelude::*;

#[derive(Event)]
struct Speak {
    message: String,
}

fn main() {
    let mut world = World::new();
    world.add_observer(|s: On<Speak>| {
        println!("{}", s.message);
    });
    world.trigger(Speak { message: "hi".into() });
}
```

工程侧建议：

- 如果你需要**事件冒泡/传播**、对“某个 entity 的点击/拖拽”等做**实体定向**处理，优先用 Events + Observers。
- 如果你需要在 Update 内分阶段处理大量消息（例如 damage pipeline），优先用 Messages（见第 3 节）。

## 2) EntityEvent：事件带目标 entity（支持 entity.observe）

EntityEvent 的意义：

- 触发时除了跑全局 observer，还会跑“绑定到某个实体”的 observer（`entity.observe(...)`）。

示例与讲解：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/observers.rs`

常见触发方式：

- `commands.trigger(MyEntityEvent { entity })`

在实体上绑定 observer（例如点击一个 UI entity 或 mesh entity）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/picking/simple_picking.rs`（`.observe(on_click_spawn_cube)`、`On<Pointer<Drag>>`）

## 3) Messages：缓冲、批处理、按调度点读取

核心文档（强烈建议读顶部注释）：

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ecs/src/message/mod.rs`

关键特性：

- 写入：`MessageWriter<T>::write(...)`
- 读取：`MessageReader<T>::read()`（reader 必须是 `mut`，内部维护 cursor）
- 变更：`MessageMutator<T>::read()`（返回 `&mut T` 迭代器，可原地修改消息）
- 存储：`Messages<T>` 资源
- 必须提前注册：`app.add_message::<T>()`

讲解型示例（必读）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/message.rs`

这个例子同时展示了两件重要事：

- writer → mutator → reader 需要用 `.chain()` 保证顺序（否则读到上一帧）
- 多个 reader 可以消费同一类消息（广播式）

## 4) Events vs Messages：怎么选？

经验法则：

- 用 **Events + Observers**：
  - 需要“触发即响应”（例如交互）
  - 需要事件传播（bubbling/propagate）
  - 需要把回调绑定到某个 entity（`.observe(...)`）
- 用 **Messages**：
  - 高吞吐批处理（大量同类事件）
  - 需要明确的“阶段化处理”（Update 里先 A 再 B 再 C）
  - 更容易写出可预测的流水线（damage/AI/网络包处理等）

Bevy 源码里的原话（简化转述）：

- Messages 的 polling 有一点点开销，但 batch 处理大量消息时可能比 Events 更高效。
- Events 更适合 observer-based 即时响应。

## 5) “组件生命周期事件”（Add/Remove）也是 Observers

Bevy v0.18 的 observer 系统可以监听组件生命周期事件：

- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/observers.rs`
  - `On<Add, Mine>`、`On<Remove, Mine>` 用于维护 spatial index

这类模式非常适合：

- 自动维护索引/缓存（空间索引、name map、关系反向表等）
- 自动挂载派生组件（注意避免写出隐式难追踪的副作用）

## 6) 与调度（Schedule）交互：不要“猜”时序

Events 是在 `trigger(...)` 调用点立即运行；Messages 是在 schedule 点读取。

因此：

- 如果你从某 system 里 `commands.trigger(...)`，observer 可能在本 system 执行时就跑（取决于调用点）。
- 如果你 `MessageWriter::write(...)`，下游 `MessageReader` 何时看到它完全由系统顺序决定（通常用 `.chain()` 或 sets）。

调度顺序控制详见：

- `app_plugins_schedules.md`
- `ecs_advanced_patterns.md`（关于 `.chain()` 的建议）

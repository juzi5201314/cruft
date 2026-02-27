# ECS 进阶模式（Bevy v0.18）

这篇用于“更工程化、更复杂”的 ECS 用法：SystemParam 组合、SystemState、消息/观察者与调度顺序、关系组件、并行优化等。

## 1) `Local<T>`：System 私有状态

当 system 需要跨帧记忆状态，用 `Local<T>`：

- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/ecs_guide.rs`（`print_at_end_round`）

要点：

- `Local<T>` 的初始化来自 `T: FromWorld`（`Default` 也会自动实现）。
- `Local<T>` 不在 World 中，不会被别的 system 访问（隔离性好）。

## 2) `SystemState`：在非 system 环境中使用 ECS 参数

当你在这些场景需要访问资源/查询：

- 自定义 command（`FnOnce(&mut World)`）
- async task 完成后要“回写世界”

典型模式是 `SystemState::<(Res<_>, Query<_>, ...)>::new(world)`：

- 示例（强烈建议直接读）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/async_tasks/async_compute.rs`

该示例使用：

- `AsyncComputeTaskPool` 在后台生成 `CommandQueue`
- `SystemState` 在 command closure 里安全地读取资源（拿 handle）并插入组件

## 3) 并行查询与性能

Bevy 的 Query 支持并行迭代（前提是满足数据借用规则）：

- 例子可以在引擎内部找到：例如 `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_sprite/src/lib.rs` 的 `par_iter_mut()`
- examples：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/parallel_query.rs`

建议：

- 先写正确，再通过 profile 找瓶颈，再考虑并行/减少 archetype 变更/减少 asset churn。
- 性能分析入口：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/profiling.md`

## 4) Change detection 的边界

你常见到：

- `Changed<T>` / `Added<T>`：过滤 query
- `Mut<T>` / `Ref<T>`：可用于检测具体字段变动（通过 `DetectChanges`）

Source（模块结构与导出）：

- `https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_ecs/src/change_detection`

注意：

- Change detection 依赖“在 ECS 调度点之间”的变更记录；命令缓冲的 apply 时机、系统执行顺序都会影响你何时观察到变化。
- 需要稳定的消息更新时机时，要结合 `Messages` 的更新策略（见 `events_messages_observers.md`）。

## 5) Exclusive system：`fn(&mut World)`（慎用）

当你必须做“全局、立即、复杂”的写操作，exclusive system 是最直接的：

- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/ecs_guide.rs`（`exclusive_player_system`）

代价：

- 阻塞其它系统并行执行
- 让调度更难推理（更容易写出隐性依赖）

优先考虑：

- 通过 `Commands` 组织写入
- 拆小系统，显式 `before/after` 或 set ordering

## 6) 调度顺序：`chain()` 与消息/读写一致性

很多“逻辑上必须按顺序”的处理，最简单就是 `chain()`：

- Messages 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/message.rs`
  - 展示 writer → mutator → reader 必须在同一条 chain 上，才能避免“读到上一帧消息”的延迟。

更复杂的情况用 `SystemSet`：

- ECS guide：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/ecs_guide.rs`

## 7) 关系组件（Relationships）与遍历 API

Bevy v0.18 引入/强化了 relationship 体系，用 component hooks 自动维护反向索引（RelationshipTarget）：

- 讲解型示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/relationships.rs`
- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ecs/src/relationship/mod.rs`

如果你要做：

- 自定义“目标指向”“依赖图”“技能链路”等实体关系
- 事件传播（propagate）沿关系上溯/下钻

优先看 `relationships_hierarchy.md`，再回来看这段。

## 8) Observer/事件的工程化用法（简述）

Observers 是 push-based：触发时立即执行；Messages 是 pull-based：在 schedule 点批处理。

- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/observers.rs`
- 详细说明：`events_messages_observers.md`

## 9) Debug 你的 ECS：诊断、gizmos、remote

- 性能分析：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/profiling.md`
- gizmos：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_gizmos/src/lib.rs` + `examples/gizmos/*`
- remote（BRP）：`examples/remote/server.rs` + `bevy_remote` crate docs

综合入口：`debug_profiling_devtools_remote.md`

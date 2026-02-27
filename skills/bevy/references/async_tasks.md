# Async tasks（Bevy v0.18）：Task pools、后台计算、回写 World

Bevy 提供任务池与并行工具，用于把重计算/IO 从主线程挪走。

## 0) 入口与 examples

- tasks crate：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_tasks/src/lib.rs`
- examples：
  - `examples/async_tasks/async_compute.rs`（最推荐：AsyncComputeTaskPool + CommandQueue 回写）
  - `examples/async_tasks/async_channel_pattern.rs`
  - `examples/async_tasks/external_source_external_thread.rs`

## 1) 三类常用任务池

从 `bevy_tasks` 导出：

- `AsyncComputeTaskPool`：适合 async/并行计算
- `ComputeTaskPool`：一般计算
- `IoTaskPool`：IO 任务

入口：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_tasks/src/lib.rs`（`pub use usages::{...}`）

## 2) 推荐模式：Task 结果回写 ECS（CommandQueue）

`async_compute.rs` 给了非常工程化的闭环：

- 在 Startup 里为每个 entity spawn 一个后台 task
- task 计算完成后返回 `CommandQueue`
- Update 里用 `check_ready(&mut task)` 轮询
- ready 后 `commands.append(&mut command_queue)`，并移除 task 组件

重要警告（示例里写了）：

- 不要用 `block_on(poll_once)` 去等任务，会阻塞且可能留下不能二次 await 的 Task
- 用 `check_ready` 做非阻塞轮询

示例：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/async_tasks/async_compute.rs`

## 3) 从 task 内访问资源/查询：`SystemState`

当 task 完成后要构造 `FnOnce(&mut World)`：

- 在 closure 里不要直接捕获 `Res`/`Query`（它们只在 system 上下文有效）
- 用 `SystemState::<(Res<_>, ResMut<_>, Query<_>, ...)>::new(world)` 在回写点临时构造读取器

示例里就是这么拿到 `BoxMeshHandle` / `BoxMaterialHandle`：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/async_tasks/async_compute.rs`

## 4) Channel pattern：适合“任务主动推送结果”

如果你更想让任务“算完就发消息”，而不是轮询 Task：

- 看 `async_channel_pattern.rs`

这类模式通常会结合：

- Messages（批量处理结果）
- 或 Commands（把结果写回）

选择建议见：`events_messages_observers.md`

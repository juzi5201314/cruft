# ECS 入门（Bevy v0.18）：Component / Entity / Resource / System

这篇用于“把 Bevy 当成 ECS 框架来用”的核心写法。建议配合讲解型示例一起看：

- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/ecs_guide.rs`
- ECS prelude（常用导出）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ecs/src/lib.rs`（`pub mod prelude`）

## 1) 核心概念与约束

- Component：贴在 Entity 上的数据（`#[derive(Component)]`）。
- Entity：一组组件的集合（唯一 id）。
- Resource：全局数据（`#[derive(Resource)]`）。
- System：读写组件/资源的函数（默认可并行执行）。

并行约束是 Bevy ECS 的设计中心：

- 普通 system 不能直接拿 `&mut World` 做任意写入（会破坏并行安全）。
- 写入世界通常通过 `Commands`（命令缓冲，延迟应用）。
- 需要“立即、独占”访问时用 exclusive system（`fn(&mut World)`），但会阻塞并行。

## 2) 定义组件与资源

```rust
use bevy::prelude::*;

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Velocity(Vec3);

#[derive(Resource, Default)]
struct Score(u32);
```

## 3) Spawn / Despawn：用 `Commands` 改世界

最常见：

```rust
fn setup(mut commands: Commands) {
    commands.spawn((Player, Velocity(Vec3::new(1.0, 0.0, 0.0))));
}
```

拿 entity id（常用于“稍后补组件 / 建关系”）：

```rust
let e = commands.spawn(Player).id();
commands.entity(e).insert(Velocity(Vec3::ZERO));
```

批量 spawn：

- 参考：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/ecs_guide.rs`（`spawn_batch`）

## 4) Query：按组件过滤并遍历

读取组件：

```rust
fn print_positions(query: Query<&Transform, With<Player>>) {
    for transform in &query {
        info!("{:?}", transform.translation);
    }
}
```

读写组合：

```rust
fn movement(mut query: Query<(&mut Transform, &Velocity)>) {
    for (mut transform, vel) in &mut query {
        transform.translation += vel.0;
    }
}
```

过滤器与 change detection：

```rust
fn only_when_changed(
    mut query: Query<&mut Transform, (With<Player>, Changed<Transform>)>,
) {
    for mut t in &mut query {
        // ...
    }
}
```

常用过滤器：

- `With<T>` / `Without<T>`
- `Added<T>` / `Changed<T>`
- `RemovedComponents<T>`（见进阶篇）

## 5) 资源读写：`Res<T>` / `ResMut<T>`

```rust
fn add_score(mut score: ResMut<Score>) {
    score.0 += 1;
}
```

资源初始化方式：

- `App::init_resource::<T>()`（`T: Default` 或 `FromWorld`）
- `Commands::insert_resource(T { ... })`

示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/ecs_guide.rs`

## 6) System 的注册：Startup vs Update（以及更多 schedule）

```rust
App::new()
  .add_plugins(DefaultPlugins)
  .add_systems(Startup, setup)
  .add_systems(Update, (movement, add_score))
  .run();
```

Main schedule 的整体顺序见：

- `app_plugins_schedules.md`
- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_app/src/main_schedule.rs`

## 7) “我该用 Commands 还是直接 World？”

经验法则：

- 大多数情况下：用 `Commands`（并行安全、可组合）。
- 需要完全独占、一步到位：用 exclusive system（`fn(world: &mut World)`），但要谨慎。
  - `ecs_guide.rs` 有示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/ecs_guide.rs`（`exclusive_player_system`）

## 8) 下一步（进阶入口）

如果你遇到这些问题，去进阶篇：

- “我在 task/闭包里想用 Query/Res”：看 `SystemState`（`ecs_advanced_patterns.md` + `examples/async_tasks/async_compute.rs`）
- “我需要事件系统”：看 `events_messages_observers.md`
- “我需要实体关系/层级”：看 `relationships_hierarchy.md`

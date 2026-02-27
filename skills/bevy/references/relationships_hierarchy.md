# Hierarchy / Relationships（Bevy v0.18）

Bevy v0.18 有两类常用“实体关系”：

1. 内建层级：`ChildOf` / `Children`（用于 Transform/Visibility 传播）
2. 自定义 relationships：用 derive 宏生成 hooks，自动维护反向索引（RelationshipTarget）

## 1) 内建层级：`ChildOf` / `Children`

用途：

- 在 DefaultPlugins 下，会自动传播 `Transform` 与 `Visibility`：
  - 最终得到每个实体的 `GlobalTransform` 与 `InheritedVisibility`

示例（包含 `with_children` / `add_child` / despawn 行为）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/hierarchy.rs`

要点：

- `with_children(|parent| { ... })`：在 spawn 时创建子实体
- `commands.entity(parent).add_child(child)`：后续追加子实体
- despawn 父实体会递归 despawn 子树（注意生命周期）

## 2) 自定义 Relationships：`Relationship` / `RelationshipTarget`

Source（权威实现 + hooks 语义）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ecs/src/relationship/mod.rs`

讲解型示例（必读）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/relationships.rs`

该示例定义了：

- `Targeting(Entity)`：source-of-truth（谁指向谁）
- `TargetedBy(Vec<Entity>)`：反向集合（谁指向我）

关键机制：

- 插入/替换 `Targeting` 时，通过 component hooks 自动更新目标实体上的 `TargetedBy`
- 关系目标不存在/关系指向自己时会 warn 并移除无效关系（源码里有保护逻辑）

## 3) 派生宏写法（推荐）

示例里这种写法是标准模式：

```rust
#[derive(Component)]
#[relationship(relationship_target = TargetedBy)]
struct Targeting(Entity);

#[derive(Component)]
#[relationship_target(relationship = Targeting)]
struct TargetedBy(Vec<Entity>);
```

实践建议：

- `RelationshipTarget` 的关系集合字段最好保持私有，避免用户代码直接 mutate，破坏一致性。
- 需要“目标 entity 被 despawn 时自动处理关系”时，考虑 `linked_spawn`（见源码注释）。

## 4) 遍历 API 与循环风险

Bevy 提供了基于关系的遍历 helper（示例里用 `iter_ancestors` 做 DFS 检测环）：

- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/relationships.rs`（`check_for_cycles`）

注意：

- 避免在 relationship 图上引入环；引擎为性能不会帮你做全局环检测。
- 如果你的关系本质是 DAG（技能树、依赖图），建议在调试期加一套检测（类似示例里的 DFS）。

## 5) 与 Events/Observers 的组合：传播（propagate）

EntityEvent 支持沿层级或自定义 relationship 传播（bubbling）。

概念入口：

- Event 文档里专门有 “Propagation” 的说明：
  - Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ecs/src/event/mod.rs`

当你想实现：

- 点击子节点触发父节点响应
- 沿自定义关系上溯（比如“最近可交互祖先”）

就把 propagation 与 relationship 结合起来（具体写法按事件类型与 Traversal 选择）。

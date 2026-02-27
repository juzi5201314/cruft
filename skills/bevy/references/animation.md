# Animation（Bevy v0.18）：AnimationClip / AnimationPlayer / AnimationGraph

Bevy 的动画系统既支持：

- 直接播放 glTF 动画 clip
- 使用 AnimationGraph 做混合/状态机式动画
- 动画事件（在特定时间点触发回调）

## 0) 入口与 examples

- 动画 crate：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_animation/src/lib.rs`
- examples：
  - `examples/animation/animated_mesh.rs`（从 glTF 加载动画并播放）
  - `examples/animation/animated_mesh_control.rs`（控制/切换动画）
  - `examples/animation/animation_graph.rs`（混合图）
  - `examples/animation/animation_events.rs`（动画事件）

## 1) glTF 动画的常见运行链路

核心模式在 `animated_mesh.rs` 里非常清楚：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/animation/animated_mesh.rs`

关键步骤：

1. 用 `GltfAssetLabel::Animation(n)` 加载某个动画 clip
2. 构建 `AnimationGraph`（例如 `AnimationGraph::from_clip(...)`）
3. 把 graph 存成资产（`Assets<AnimationGraph>`）
4. 用 `SceneRoot(GltfAssetLabel::Scene(0)...)` 让场景与骨骼层级自动 spawn
5. 等 scene 实例 ready 后，遍历子孙找 `AnimationPlayer` 并：
   - `player.play(index).repeat()`
   - 在 player entity 上插入 `AnimationGraphHandle(graph_handle)`

该示例使用 observer 监听 `SceneInstanceReady`：

- `.observe(play_animation_when_ready)`
- `fn play_animation_when_ready(scene_ready: On<SceneInstanceReady>, ...) { ... }`

这意味着 `SceneInstanceReady` 是一个可观察事件（Observer 体系）。

## 2) AnimationPlayer / targets / retargeting 的关键概念

动画 crate 定义了这些核心类型：

- `AnimationClip`（资产）
- `AnimationPlayer`（组件，负责播放/混合）
- `AnimationTargetId`（骨骼/目标标识，基于 UUID namespace）
- `AnimatedBy`（把被动画影响的实体连接到 player）

入口与注释很详细：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_animation/src/lib.rs`

实践建议：

- 如果你从 glTF 导入骨骼动画，loader 会为你生成 targets 与 player。
- 你自己做 runtime 生成骨骼/修改层级时，要特别小心 target id 与 player 连接关系。

## 3) AnimationGraph：混合与状态机式结构

动画图允许你把多个 clip 组织起来做混合、过渡等：

- 示例：`examples/animation/animation_graph.rs`

当你要做：

- 行走/跑步 blend
- 上半身/下半身分层
- 动画状态机

建议优先用 AnimationGraph，而不是手写一堆 “if else 切换 player.play(...)”。

## 4) 动画事件（Events）

动画系统支持在 clip 的时间轴上触发事件回调（例如脚步声、攻击判定）。

入口：

- `bevy_animation` 的 `animation_event` 模块（可从 lib.rs 找到）
- 示例：`examples/animation/animation_events.rs`

事件与 ECS 的结合方式取决于你想把事件投递为：

- Observers 的 Event（即时响应）
- Messages（批处理）

选择建议见：`events_messages_observers.md`

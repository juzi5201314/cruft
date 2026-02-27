# Picking（Bevy v0.18）：Pointer events、点击/拖拽、UI 与 3D 交互统一

Bevy picking 是一套“输入无关、后端可插拔”的交互体系：

- 鼠标/触摸/手柄等输入被抽象成 Pointer
- 不同后端负责 hit testing（mesh/sprite/ui…）
- 最终产出 Pointer 事件，并用 Observers 分发（`.observe(...)`）

## 0) 入口与 examples

- picking crate 文档非常完整（推荐直接读）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_picking/src/lib.rs`
- examples：
  - `examples/picking/simple_picking.rs`（最小点击/拖拽，UI + mesh）
  - `examples/picking/dragdrop_picking.rs`
  - `examples/picking/mesh_picking.rs`
  - `examples/picking/sprite_picking.rs`

## 1) 最小上手：MeshPickingPlugin + `.observe(...)`

示例：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/picking/simple_picking.rs`

你会看到：

- `App::new().add_plugins((DefaultPlugins, MeshPickingPlugin))`
- 在 UI Text 实体上：
  - `.observe(on_click_spawn_cube)`
  - `.observe(|over: On<Pointer<Over>>, ...| { ... })`
- 在 mesh 实体上：
  - `.observe(on_drag_rotate)`（`On<Pointer<Drag>>`）

这说明：

- Pointer 事件是 Observers 的 Event（即时触发）
- `.observe` 让你把交互逻辑绑定到具体实体，非常“组件化”

## 2) Picking pipeline（理解模块化与组合）

crate 文档把 pipeline 拆成：

1. Pointers：把输入更新成 `PointerLocation` 等
2. Backend：读取 pointers，产出 `PointerHits`
3. Hover：合并排序，得到真正 hover 的实体集合
4. Events：生成 Click/Drag/Drop 等高层事件（并支持冒泡）

入口：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_picking/src/lib.rs`（大段说明）

实践含义：

- 你可以同时启用多个 backend（例如 UI + 3D mesh）
- 你也可以写自己的 backend（文档说大约 100 行即可）

## 3) `Pickable`：控制“是否可被拾取/是否阻挡下层”

`Pickable` 组件允许你：

- 让某实体不参与 hover/click
- 或让它不阻挡下层实体（例如透明 UI 覆盖层）

入口：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_picking/src/lib.rs`（`pub struct Pickable`）

## 4) 与 UI / Sprite 的关系

除了 mesh picking，还可以有：

- UI picking backend（在 `bevy_ui` 中按 feature 提供）
- Sprite picking backend（在 `bevy_sprite` 中按 feature 提供）

这让“UI 与 3D 交互统一”成为可能（例如 UI 拖拽到 3D 物体上）。

UI 入口：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ui/src/lib.rs`（`picking_backend`）

Sprite 入口：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_sprite/src/lib.rs`（`picking_backend`）

## 5) 事件系统基础

Picking 依赖 Observers/Event：

- `events_messages_observers.md`

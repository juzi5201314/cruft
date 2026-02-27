# Scenes / glTF（Bevy v0.18）

本篇解决：

- 如何 spawn Scene / DynamicScene
- glTF 的“整文件 vs 子资源”加载方式
- `GltfAssetLabel` / `SceneRoot` 的正确用法
- 反射与序列化在 Scene 中扮演的角色

## 1) Scene：世界的一段可复用“实体集合”

Scene crate 提供：

- `Scene`：包含一个 `World`
- `DynamicScene`：基于反射的动态场景（可序列化/反序列化）
- `SceneSpawner`：把 scene 实例化到主世界里

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_scene/src/lib.rs`

重要调度点：

- Scene spawn 系统在 Main schedule 的 `SpawnScene` label 执行
  - Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_scene/src/lib.rs`（`app.add_systems(SpawnScene, ...)`）
  - Main schedule 位置：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_app/src/main_schedule.rs`

## 2) `SceneRoot` / `DynamicSceneRoot`：把 scene handle 贴到实体上

典型用法（spawn 一个 scene 作为实体树）：

```rust
fn spawn_scene(mut commands: Commands, asset_server: Res<AssetServer>) {
    let scene: Handle<Scene> = asset_server.load("my_scene.scn.ron");
    commands.spawn(SceneRoot(scene));
}
```

（具体扩展名与 loader 取决于你是否启用了 `serialize` 等 feature）

## 3) glTF：Bevy 通过 AssetLoader 加载 glTF 2.0

glTF 插件与类型：

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_gltf/src/lib.rs`

### 3.1 最重要的坑：必须指定你要加载 glTF 的哪一部分

如果你直接：

```rust
asset_server.load("model.gltf");
```

那你得到的是 `Handle<Gltf>`（整文件解析结果），不是某个 scene/mesh。

要直接拿“第 0 个 scene”：

```rust
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let scene: Handle<Scene> =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/FlightHelmet/FlightHelmet.gltf"));
    commands.spawn((SceneRoot(scene), Transform::from_xyz(0.0, 0.0, 0.0)));
}
```

这段写法在 glTF crate 顶部 Quick Start 中就有（建议直接读）：

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_gltf/src/lib.rs`

Context7（docs.rs 精炼检索）也能查到同样片段：`/websites/rs_bevy`（`GltfAssetLabel`）。

### 3.2 先 load `Handle<Gltf>`，再访问子资源（命名 scene 等）

当你需要按名称拿某个子场景/mesh，先 load 整体 `Gltf` 再查询 `Assets<Gltf>`：

- Source 示例在 glTF crate 文档：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_gltf/src/lib.rs`

典型模式：

- 用资源保存 `Handle<Gltf>`
- 在 Update 里等 `Assets<Gltf>::get(handle)` 变成 Some
- 再 spawn `SceneRoot(gltf.scenes[0].clone())` 或 `gltf.named_scenes["..."]`

## 4) 反射与序列化：DynamicScene 的关键

DynamicScene 依赖反射：

- 你的组件要 `#[derive(Reflect)]` 并注册到 type registry
- 反射序列化示例：
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/reflection/serialization.rs`
- reflect crate 总览：
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_reflect/src/lib.rs`

如果你要做：

- 关卡编辑器（保存/加载）
- 运行时生成与持久化场景片段

就把 “Reflect + DynamicScene + SceneSpawner” 当成一条完整链路来设计。

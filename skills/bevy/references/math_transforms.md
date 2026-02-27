# Math / Transforms（Bevy v0.18）：Vec/Quat、Transform/GlobalTransform、层级传播

本篇聚焦“空间/姿态”相关的常用写法与坑。

## 0) examples

- 层级与传播：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/hierarchy.rs`
- Transform 综合示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/transforms/transform.rs`
- 旋转/平移/缩放：`examples/transforms/*`

## 1) `Transform` 与 `GlobalTransform`

典型实体会挂：

- `Transform`：局部空间（相对父节点）
- `GlobalTransform`：世界空间（系统计算出来）

当 DefaultPlugins 启用时：

- 会自动把父子层级（ChildOf/Children）上的局部 transform 传播，得到正确的 `GlobalTransform`
- 这就是为什么 UI、层级 sprite、骨骼动画等能工作

入门示例：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ecs/hierarchy.rs`

## 2) 常用构造与操作

```rust
Transform::from_xyz(x, y, z)
Transform::from_translation(Vec3::new(...))
Transform::from_rotation(Quat::from_rotation_y(...))
transform.translation += direction * speed * time.delta_secs()
```

相机朝向：

```rust
Transform::from_xyz(0.0, 10.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y)
```

示例里有完整的“轨道运动 + lerp 朝向”：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/transforms/transform.rs`

## 3) 2D 与 3D：同一套 Transform，不同的约定

- 2D 常用 `Camera2d`，通常在 XY 平面工作（Z 用于层级/深度排序）
- 3D 用 `Camera3d`，默认 Y 轴向上（示例基本都这么写）

如果你发现“物体不见了”：

- 先检查 camera 位置与朝向
- 再检查 Z（2D）/裁剪面（3D projection）
- 最后看 Visibility/RenderLayers 等（进阶）

## 4) `Rot2` / UI transform

UI 有自己的 `UiTransform`（例如旋转 UI 文本）：

- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/pbr.rs`（用 `UiTransform { rotation: Rot2::degrees(90.), .. }` 旋转 “Metallic” 文本）

## 5) 与 Relationships/Hierarchy 的连接

层级关系与 transform 传播强相关：

- ChildOf/Children 是关系组件（Relationship）
- transform propagation 系统按层级更新 GlobalTransform

更深入的 relationships 机制见：

- `relationships_hierarchy.md`

# Anti-Aliasing / Post-Process（Bevy v0.18）

这篇聚焦“主渲染结束后的画面处理”两大块：

- 抗锯齿（FXAA / SMAA / TAA / CAS /（可选）DLSS）
- 后处理（Bloom / DOF / Motion Blur / Auto Exposure / Effect stack 等）

这些功能大多以“**给相机插组件**”的方式启用，并通过 RenderGraph 节点在主 pass 后执行。

## 0) 入口与 examples

源码入口：

- `bevy_anti_alias` crate：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_anti_alias/src/lib.rs`
  - FXAA：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_anti_alias/src/fxaa/mod.rs`
  - SMAA：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_anti_alias/src/smaa`
  - TAA：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_anti_alias/src/taa/mod.rs`
  - CAS（锐化）：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_anti_alias/src/contrast_adaptive_sharpening`
  - DLSS（特性门控）：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_anti_alias/src/dlss`
- `bevy_post_process` crate：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_post_process/src/lib.rs`
  - bloom/dof/motion blur/effect stack 等在该 crate 子模块：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_post_process/src`

DefaultPlugins 中的安装顺序（理解“为何能工作/为何被插在这里”）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_internal/src/default_plugins.rs`

关键 examples：

- 抗锯齿总览：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/anti_aliasing.rs`
- 后处理总览：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/post_processing.rs`
- Bloom：
  - 3D：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/bloom_3d.rs`
  - 2D：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/bloom_2d.rs`
- Motion blur：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/motion_blur.rs`
- Depth of field：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/depth_of_field.rs`
- Auto exposure：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/auto_exposure.rs`
- Color grading：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/color_grading.rs`

## 1) 抗锯齿怎么选（工程化的选型）

你基本会在三类方案里选：

- **FXAA / SMAA（空间 AA）**：简单、便宜、稳定；但细线/闪烁改善有限，且可能变糊。
- **TAA（时间 AA）**：对闪烁改善明显，画面更稳；但可能 ghosting，且对 motion vectors/深度写入更敏感。
- **DLSS（上采样+降噪/重建，硬件与 feature 依赖）**：效果与性能潜力大，但门槛高（设备/驱动/项目配置）。

建议路径：

1. 先用 FXAA/SMAA 把项目跑稳（开发成本低、debug 友好）
2. 再上 TAA（需要正确 motion vectors 与 prepass）
3. 最后再考虑 DLSS（如果你明确需要且能承担依赖）

## 2) FXAA：给相机插 `Fxaa`

FXAA 的核心是一个相机组件 `Fxaa`：

- 定义：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_anti_alias/src/fxaa/mod.rs`

典型用法（把组件插到 camera entity）：

```rust
commands.spawn((Camera3d::default(), bevy::anti_alias::fxaa::Fxaa::default()));
```

（更完整的参数/效果对照直接看 `anti_aliasing.rs`）

## 3) TAA：别忽略“必需前提”（MSAA、prepass、motion vectors）

`TemporalAntiAliasing` 的文档非常关键，里面明确写了工程约束：

- 定义与 usage notes：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_anti_alias/src/taa/mod.rs`

你需要重点记住：

- **相机必须 `Msaa::Off`**（否则会 warn 并跳过）
- 需要深度与 motion vectors prepass（该组件通过 `#[require(...)]` 约束了一组必需组件）
- alpha-blend 的对象与 motion vectors 缺失会导致 ghosting/拖影
- 镜头切换/瞬移时要 reset history（`TemporalAntiAliasing { reset: true }`）

这类细节很难靠“纯 docs”记住：建议直接跑 `anti_aliasing.rs` 并读源码注释。

## 4) Bloom / DOF / Motion blur：以“相机/视图组件”启用

后处理通常也是“插组件到相机”：

- Bloom 组件定义在 `bevy_post_process::bloom`，示例：
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/bloom_3d.rs`
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/bloom_2d.rs`
- Motion blur 示例：
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/motion_blur.rs`
- DOF 示例：
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/depth_of_field.rs`

总览示例把多个后处理组合在一起，适合当模板：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/post_processing.rs`

## 5) 与渲染架构的连接点：RenderGraph 节点插在哪？

如果你在排查：

- “为什么效果没生效？”
- “为什么多个效果叠加顺序不对？”
- “为什么 2D/3D 的节点不一样？”

你需要回到源码看 RenderGraph 的边与节点插入位置（例如 FXAA 插在 tonemapping 后）。

建议从这些入口读：

- FXAA 插图节点（Core2d/Core3d graph）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_anti_alias/src/fxaa/mod.rs`
- 渲染架构总览：`rendering_architecture.md`

## 6) 与精炼文档的对照（Context7）

Context7 `/websites/rs_bevy` 适合快速查：

- `Fxaa` / `TemporalAntiAliasing` / `Msaa::Off`
- bloom/motion blur/dof 的组件名与字段

但“某效果需要哪些 prepass/限制、节点插入顺序、与 HDR/tonemapping 的交互”等，以 v0.18.0 源码与 examples 为准。


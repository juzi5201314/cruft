# Raytraced Lighting：Bevy Solari（Bevy v0.18）

这篇聚焦 `bevy_solari`：Bevy 的实验性光追照明（实时 raytraced lighting + 可选 pathtracer 验证），以及它对相机/纹理 usages/WGPU features 的硬要求。

> 这是偏“渲染深水区”的功能：建议先把 `rendering_architecture.md` 与 `anti_alias_post_process.md` 的基本脉络看明白。

## 0) 入口与 examples（从示例开始）

源码入口：

- `bevy_solari` crate 总览（含必需 `WgpuFeatures`）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_solari/src/lib.rs`
- 目录（realtime/pathtracer/scene 等）：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_solari/src`

权威示例：

- Solari demo：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/solari.rs`

相关依赖点（示例里会用到）：

- `CameraMainTextureUsages`（主纹理需要 `STORAGE_BINDING`）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_camera/src/camera.rs`
- 后处理 bloom（示例 UI/画面辅助）：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_post_process/src`
- DLSS（可选，特性门控）：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_anti_alias/src/dlss`
- 相机控制器（示例用 free camera）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_camera_controller/src/lib.rs`

## 1) Solari 的最小接入：加插件组 + 给实体加 `RaytracingMesh3d`

从示例可归纳出最小闭环：

1. 安装插件：
   - `SolariPlugins`（realtime lighting + scene plugin）
2. 给需要参与光追的 mesh entity 加：
   - `RaytracingMesh3d`
   - 并确保它是 `Mesh3d + MeshMaterial3d::<StandardMaterial>`（示例注释明确说了）
3. 在相机上插入：
   - `SolariLighting`（realtime）
   - 或 `Pathtracer`（参考 pathtracer plugin）

以上都在 `examples/3d/solari.rs` 里有完整代码。

## 2) 硬要求：WGPU features 与相机纹理 usages

### 2.1 必需的 `WgpuFeatures`

`bevy_solari` 提供了 `required_wgpu_features()`，列出了必需的 wgpu feature（例如 `EXPERIMENTAL_RAY_QUERY` 等）。

权威来源：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_solari/src/lib.rs`

工程含义：

- 你的 GPU/驱动/平台必须支持这些 features，否则 Solari 无法工作
- 这也是它“实验性”的重要原因之一

### 2.2 相机主纹理必须支持 `STORAGE_BINDING`（并且关闭 MSAA）

Solari 示例里写得非常直白：

- “Msaa::Off and CameraMainTextureUsages with STORAGE_BINDING are required for Solari”

对应类型定义：

- `CameraMainTextureUsages`：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_camera/src/camera.rs`

因此你会看到类似：

- `CameraMainTextureUsages::default().with(TextureUsages::STORAGE_BINDING)`
- `Msaa::Off`

## 3) DLSS Ray Reconstruction（可选但强烈建议：降噪 + 上采样）

Solari demo 在支持 DLSS 的情况下，会在相机上插入 DLSS Ray Reconstruction 组件来做降噪与更便宜的渲染：

- DLSS 模块（特性门控）：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_anti_alias/src/dlss`
- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/solari.rs`

工程提醒：

- 示例里明确提示：`DlssProjectId` 不要照抄，要生成你自己的 UUID

## 4) Debug/Profiling 建议

Solari 属于 GPU-heavy 功能，建议：

- 先用 `RenderDiagnosticsPlugin`/tracing 确定瓶颈位置（示例也装了）
- 再考虑启用/裁剪后处理、降低分辨率、减少光追对象等

更通用的工具入口见：

- `debug_profiling_devtools_remote.md`
- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/profiling.md`

## 5) 与精炼文档的对照（Context7）

Context7 `/websites/rs_bevy` 适合快速查：

- `SolariPlugins` / `SolariLighting` / `RaytracingMesh3d`
- `CameraMainTextureUsages`

但 Solari 的“能否跑起来”高度依赖平台与 wgpu features：以 `bevy_solari` 源码与 `examples/3d/solari.rs` 为准。


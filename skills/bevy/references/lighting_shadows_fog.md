# Lighting / Shadows / Fog（Bevy v0.18）

这篇聚焦光照相关：灯光类型、单位与参数、阴影（级联/偏差/PCSS 等示例）、环境光照（环境贴图/探针/体积）、以及雾/体积雾。

> PBR 材质与 3D 入门骨架见 `rendering_3d_pbr.md`；渲染子世界/RenderGraph 见 `rendering_architecture.md`。

## 0) 入口与 examples（不要猜，直接对照）

源码入口：

- `bevy_light` crate（`LightPlugin` + 主要类型）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/lib.rs`
- 环境光：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/ambient_light.rs`
- 平行光：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/directional_light.rs`
- 点光：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/point_light.rs`
- 聚光灯：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/spot_light.rs`
- 级联阴影配置：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/cascade.rs`
- 环境贴图/探针：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/probe.rs`
- 体积相关（FogVolume/VolumetricFog/VolumetricLight）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/volumetric.rs`

关键 examples（强烈建议按主题逐个跑）：

- 基础光照：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/lighting.rs`
- 聚光灯：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/spotlight.rs`
- 阴影偏差与常见问题（Peter-panning / acne）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/shadow_biases.rs`
- 阴影投射者/接收者控制：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/shadow_caster_receiver.rs`
- PCSS（软阴影）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/pcss.rs`
- 环境贴图旋转（环境光照/反射感知）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/rotate_environment_map.rs`
- Irradiance volumes：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/irradiance_volumes.rs`
- Reflection probes：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/reflection_probes.rs`
- 雾（Fog）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/fog.rs`
- 雾体积（FogVolume）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/fog_volumes.rs`
- 体积雾：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/volumetric_fog.rs`
- 大气/大气雾：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/atmosphere.rs`、`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/atmospheric_fog.rs`

## 1) 最小光照骨架（PBR 世界）

3D PBR 的最小骨架通常包含：

- `Mesh3d` + `MeshMaterial3d(StandardMaterial)`
- 至少一个光源（`DirectionalLight` 或 `PointLight`）
- `Camera3d`

可以直接从 `examples/3d/pbr.rs` 与 `examples/3d/lighting.rs` 抄骨架：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/pbr.rs`
- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/lighting.rs`

## 2) 单位与“正确的数值范围”：不要乱填强度

Bevy 提供了光照单位常量（lux / lumen 等），用于让强度参数落在“合理区间”。

- 定义在 `bevy_light::light_consts`：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/lib.rs`

经验法则（推荐做法）：

- `DirectionalLight.illuminance`：用 `light_consts::lux::*` 作为起点，再微调
- `PointLight.intensity` / `SpotLight.intensity`：根据示例与场景规模调；别从 1.0 开始瞎试

## 3) 阴影：优先用 examples 对照“偏差与质量”

阴影常见问题基本都绕不开：

- Shadow acne（自遮挡条纹）
- Peter-panning（物体漂浮）
- Shadow map 分辨率/级联覆盖范围
- 软阴影质量与性能

最推荐的学习路径：

1. 先跑 `shadow_biases.rs`，理解 bias 的效果与取舍：
   - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/shadow_biases.rs`
2. 再跑 `shadow_caster_receiver.rs`，理解“谁投/谁收”的控制方式：
   - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/shadow_caster_receiver.rs`
3. 想要更高级的软阴影：看 `pcss.rs`：
   - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/pcss.rs`

如果你需要理解级联阴影的配置结构（DirectionalLight 常见），入口在：

- `CascadeShadowConfig`/builder：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/cascade.rs`

## 4) 灯光类型速查：Directional / Point / Spot

三类灯大体区别：

- **DirectionalLight**：无限远平行光（太阳）。更依赖级联阴影配置。
- **PointLight**：点光源（球形衰减）。对室内/局部照明常用。
- **SpotLight**：圆锥形。适合手电筒、舞台灯。

各自权威字段与注释分别在：

- `DirectionalLight`：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/directional_light.rs`
- `PointLight`：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/point_light.rs`
- `SpotLight`：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/spot_light.rs`

对应示例：

- `lighting.rs`、`spotlight.rs`

## 5) 环境光照：环境贴图 / Probes / Irradiance Volumes

在 PBR 里，“看起来真实”通常离不开环境光照：

- 环境贴图（IBL）影响反射与整体氛围
- Reflection probes/irradiance volumes 用来改进局部环境一致性

示例（按顺序看）：

- 环境贴图旋转：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/rotate_environment_map.rs`
- Reflection probes：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/reflection_probes.rs`
- Irradiance volumes：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/irradiance_volumes.rs`

相关核心类型定义：

- `EnvironmentMapLight` / `LightProbe` / `IrradianceVolume`：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/probe.rs`

## 6) 雾与体积：Fog / FogVolume / VolumetricFog

建议先用 examples 快速区分能力边界：

- `fog.rs`：全局雾（更像“后处理/大气”）
- `fog_volumes.rs`：局部雾体积（FogVolume）
- `volumetric_fog.rs`：体积雾（与灯光交互）
- `atmosphere.rs` / `atmospheric_fog.rs`：大气散射相关效果

源码入口：

- 体积相关：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_light/src/volumetric.rs`

## 7) 调试与性能：别盲优化

灯光/阴影/体积效果非常吃 GPU/带宽。推荐流程：

1. 先用 `docs/profiling.md` 的方式把瓶颈定位清楚：
   - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/profiling.md`
2. 再针对性降低成本（减少投影灯数量、降低阴影分辨率、裁剪体积范围、减少透明层等）。

调试工具入口见：`debug_profiling_devtools_remote.md`

## 8) 与精炼文档的对照（Context7）

Context7 `/websites/rs_bevy` 适合快速查：

- `DirectionalLight` / `PointLight` / `SpotLight`
- `AmbientLight`
- `CascadeShadowConfig`
- `EnvironmentMapLight` / probes

但**具体的质量/性能取舍**（bias、PCSS、体积雾参数）更依赖 examples 与源码注释。


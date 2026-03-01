# Procedural Textures

本文档按 **implemented / planned** 标注当前能力，避免规范与实现漂移。

## Runtime status

- Implemented: RenderApp compute 一次性生成 `texture_2d_array`。
- Implemented: 生产 shader 风格为 `minecraft_quantized`（periodic value noise + fbm + 可选 warp + 4 色量化）。
- Implemented: Boot 聚合可感知 `Loading / Ready / Failed`（失败不再 panic 卡死）。
- Planned: 多风格 shader（`hd_pixel_art` / `hd_realistic` / `vector_toon`）。
- Planned: layer 覆盖（`has_layer/layer_ratio/top_layer_palette`）在 shader 端生效。

## Data schema

## v1 (implemented)

支持两种 JSON 外形：

1) 兼容旧格式（顶层数组）
2) 显式版本格式（推荐）

顶层字段：
- `name`（string，必填）：材质名（用于输出命名）。
- `size`（int，必填）：贴图尺寸，建议 16–1024 且为 2 的幂。
- `seed`（int，必填）：随机噪声种子；相同配置应生成一致结果。
- `style`（string，必填）：`minecraft` | `hd_pixel_art` | `hd_realistic` | `vector_toon`。
- `faces`（object，必填）：面配置集合；键名建议使用 `all`、`top`/`bottom`/`sides`，以及可选的 `north`/`south`/`east`/`west`（用于单独覆盖某个侧面）。

每个 face 的字段：
- `has_layer`（bool，必填）：是否启用“覆盖层”（常用于 sides 的雪线/苔藓线）。
- `layer_ratio`（float，`0..1`，可选）：覆盖层占比；`has_layer=true` 时建议提供。
- `top_layer_palette`（RGB[]，可选）：覆盖层调色板；`has_layer=true` 时必填。
- `base`（object，必填）：基础材质参数
  - `palette`（RGB[]，必填）：调色板；RGB 为 `[0..255, 0..255, 0..255]`。
    - 当前实现对 `RGB[]` 采用 **可变长度**（不再固定 4 色）；约束由显存/缓冲区大小决定。
  - `noise_scale`（float，>0，可选，默认 1.0）：噪声缩放。
  - `octaves`（int，>=1，可选，默认 4）：分形层数。
  - `warp_strength`（float，>=0，可选，默认 0.0）：定义域扭曲强度；0 关闭。

输出约定（示例实现）：
- 生成器按 `faces` 中出现的键输出 `{name}_{key}.png`（例如 `top`/`bottom`/`sides` 或 `north` 等）。
- 引擎侧面选择建议按优先级：`north/south/east/west` > `top/bottom` > `sides` > `all`。

## 校验规则（建议）

- `palette` 至少 2 个颜色（否则大部分风格会缺乏层次）。
- `palette` / `top_layer_palette` 没有固定长度上限；但过长会增加显存占用与启动生成耗时。
- `noise_scale` 过大可能导致周期过小（极端情况下会退化成常量噪声），建议限制在合理范围并在实现侧做防御性处理。
- `has_layer=true` 时强制要求 `layer_ratio` 与 `top_layer_palette`。

## Faces / UV 约定（建议）

允许的 `faces` 键：
- `all`：所有面默认使用同一份配置/贴图。
- `top`、`bottom`：分别用于上下表面。
- `sides`：用于四个侧面（`north`/`south`/`east`/`west`）。
- `north`、`south`、`east`、`west`：单独指定某个侧面（用于覆盖 `sides` 的默认值）。

建议固定一套“方块局部坐标 + 面朝向”约定（只要全链路一致即可）：
- +Y：上（`top`），-Y：下（`bottom`）
- -Z：北（`north`），+Z：南（`south`）
- +X：东（`east`），-X：西（`west`）

UV（图片）朝向建议：
- 约定图片坐标：u 向右、v 向下（PNG 像素坐标直觉）。
- 为了让 `has_layer + layer_ratio` 在 `sides` 上表现为“从顶部往下覆盖”，建议 `sides` 的 v 方向对应世界 -Y（即图片最上方对应方块更高处）。
- 推荐各面的 u/v 轴对应（u→右、v↓下）：
  - `top (+Y)`：u→+X，v→+Z
  - `bottom (-Y)`：u→+X，v→+Z（通常不敏感，保持与 `top` 一致即可）
  - `north (-Z)`：u→+X，v→-Y
  - `south (+Z)`：u→-X，v→-Y
  - `east (+X)`：u→-Z，v→-Y
  - `west (-X)`：u→+Z，v→-Y
- 若只提供 `sides`（未单独配置 `north/south/east/west`），引擎可复用同一张 `sides` 贴图渲染四个侧面；关键是按上面的 u 轴方向为每个面设置正确的 UV（避免镜像/旋转带来的违和）。

## 跨面连续（可选，进阶）

2D 的“可平铺”并不等价于 cube 各面边界连续：如果每个面都独立在 2D 域里生成噪声，`top↔sides` 的边缘大概率无法无缝衔接。

若你确实需要跨面连续（比如花纹从 `top` 自然延伸到 `sides`），更稳妥的方向是：
- 使用 **周期化 3D 噪声场** `f(x, y, z)`（XYZ 三个轴都可 wrap），然后对 cube 表面做一致的 3D 采样来生成各面贴图。
- 关键点：共享边界的像素必须映射到 **同一组 3D 坐标**，这样两面的采样值天然一致；domain warp 也需要是 3D 且周期化，否则会破坏边界连续性。

备选方案（不推荐作为主路径）：先生成主面，再把边界 copy/stitch 到相邻面并做局部重采样/混合。这会牺牲噪声统计特性，且容易在 mipmap/过滤后重新暴露接缝。

## 多贴图输出（可选）

如果希望材质更“PBR 友好”，建议把输出从“只有 Albedo”扩展到多张贴图（命名仅建议）：
- `{name}_{key}_albedo.png`：sRGB
- `{name}_{key}_normal.png`：线性；tangent-space normal（建议统一 OpenGL 约定：+Y/绿色向上）
- `{name}_{key}_roughness.png`：线性
- `{name}_{key}_ao.png`：线性
- `{name}_{key}_height.png`：线性（若用于 parallax/细节，建议 16-bit）

mipmap / atlas 注意事项（很容易踩坑）：
- **色彩空间**：`albedo` 生成 mipmap 时应在 **线性空间** 下采样，再转回 sRGB；否则会偏暗。
- **法线贴图**：mipmap 下采样后必须重新归一化，否则会发灰/发黑。
- **Atlas bleeding**：如果最终会打进 texture atlas，建议对每个 tile 做 2–4px extrusion/padding（对可平铺贴图可用 wrap 补边），避免 mipmap/线性过滤把邻居 tile “漏色”到边缘形成接缝。
- **风格与采样**：`minecraft`/`hd_pixel_art` 通常用 nearest；`hd_realistic` 更适合 linear + mipmaps + 各向异性过滤。
- **运行时采样切换**：可通过环境变量 `CRUFT_VOXEL_SAMPLING` 选择方块采样模式：`pixel`（默认，`mag/min=nearest + mipmap=linear`）或 `smooth`（`mag/min/mipmap=linear + anisotropy=16`）。

## Bevy + WGSL（启动时 GPU 动态生成贴图）

目标：在 **游戏启动时** 用 WGSL compute shader 直接把程序化贴图写入 GPU（显存）中的纹理资源，然后把这张纹理当作普通 `Image`/材质贴图去使用，尽量做到 **零 CPU 像素循环、零 GPU→CPU 回读**。

本仓库示例采用 **texture array**（`texture_2d_array`）：一次 dispatch 批量生成 N 层，每层一张 64×64 贴图。

### 推荐架构（高性能）

- **主世界（Main World）**：只负责“声明要生成什么”
  - 创建目标 `Image`（带 `STORAGE_BINDING` 用途）并保留 `Handle<Image>` 供材质引用
  - 把生成参数（seed、palette、style、噪声参数等）放在 `Resource` 里
- **渲染子世界（RenderApp / Render World）**：负责“真正生成”
  - `RenderStartup`：创建 bind group layout + queue compute pipeline（`PipelineCache`）
  - `PrepareBindGroups`：上传参数 uniform buffer（纹理 view 在 node 内按 mip 动态创建）
  - `RenderGraph Node`：在每帧渲染前 dispatch compute；按 mip 逐级 dispatch，**只跑 1 次**（生成完成后进入 Done 状态）

Bevy v0.18 官方同类参考（强烈建议先对照骨架再改逻辑）：
- `examples/shader/compute_shader_game_of_life.rs`
- `assets/shaders/game_of_life.wgsl`

本仓库内的最小可运行骨架（与本文一致，启动时生成一次）：
- `crates/app/src/plugins/procedural_texture.rs`
- `assets/shaders/procedural_texture.wgsl`
- `assets/shaders/procedural_array_material.wgsl`
- `assets/texture_data/blocks.texture.json`

资产嵌入方式：使用 `bevy_embedded_assets`（ReplaceDefault 模式）把 `assets/` 整体嵌入到可执行文件中，运行时照常通过 `AssetServer` 路径加载（无需 `embedded://` 前缀）。

### 最小实现要点（关键坑位都在这里）

1) **创建目标纹理（主世界）**

- 纹理格式建议从 `TextureFormat::Rgba8Unorm` 或 `TextureFormat::Rgba16Float` 开始：
  - `rgba8unorm` 支持 storage write，带宽/显存占用更低
  - 注意：**storage texture 通常不支持 sRGB 格式**，因此不要用 `Rgba8UnormSrgb` 写入
- 若要一次性生成多张方块贴图，建议用 **texture array**：
  - `image.texture_descriptor.size.depth_or_array_layers = layer_count`
  - `image.texture_view_descriptor.dimension = D2Array`
- 纹理 usage 必须包含：
  - `TextureUsages::STORAGE_BINDING`（compute 写入）
  - `TextureUsages::TEXTURE_BINDING`（后续采样）
  - `TextureUsages::COPY_DST`（可选：后续 CPU 填充/调试）
- 为了避免在主世界保留 CPU 侧像素数据（省内存 & 减少同步），建议：
  - `image.asset_usage = RenderAssetUsages::RENDER_WORLD;`

2) **数据从主世界传到渲染世界**

- 用 `ExtractResourcePlugin<T>` 把以下资源抽取进渲染世界：
  - 目标贴图句柄（`Handle<Image>`）
  - 生成参数（`#[derive(ShaderType)]`，走 uniform buffer）

3) **RenderApp：pipeline + bind group**

- `RenderStartup`：
  - `BindGroupLayoutDescriptor`（`texture_storage_2d_array` + `uniform_buffer` + `storage_buffer`）
  - `pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor { .. })`
- `RenderSystems::PrepareBindGroups`：
  - 从 `RenderAssets<GpuImage>` 取 `GpuImage.texture_view`
  - 用 `UniformBuffer::from(params)` + `write_buffer()` 上传每层参数（style/offset/len 等）
  - 用 `StorageBuffer` 上传调色板颜色池（runtime-sized array）
  - `render_device.create_bind_group(...)` 写入 resource

4) **只在启动时生成 1 次（RenderGraph Node）**

- Node 内部维护一个状态机：
  - `Loading`：等 pipeline 编译完
  - `Init`：dispatch 一次
  - `Done`：后续不再 dispatch（避免每帧浪费）
- 把节点加到 render graph，并连一条 edge 到 `CameraDriverLabel`，保证它在相机驱动渲染前执行（同官方示例）。

### 性能进一步优化建议（面向“很多方块材质”）

- **批量生成**：不要为每个方块类型单独 dispatch / 单独纹理。
  - 推荐把输出组织成 **texture atlas** 或 **texture array**：
    - Atlas：1 张大 2D storage texture，一次 compute 生成 N 个 tile（用 `tile_id` 映射到输出坐标）
    - Array：1 张 `texture_2d_array`（`texture_storage_2d_array`），一层一个 tile/面（采样时用 layer 索引）
- **减少 bind group 变化**：尽量复用同一个 pipeline/layout；参数用 uniform（或按需拆成多个 pipeline entry point）。
- **避免 readback**：除非做测试/导出，否则不要 GPU→CPU 拷贝（会强制同步，拖垮帧时间）。
- **工作组大小**：`8x8` 通常比较稳；想榨性能可试 `16x16`，但要结合 GPU 的 occupancy/寄存器压力。

### JSON size 归一（当前实现：统一到 64×64）

为了避免 JSON 中不同 `size` 导致资源碎片与采样差异，示例实现将所有贴图输出统一到常量 `64×64`（见 `CANONICAL_TEXTURE_SIZE`）。

当 JSON 中 `size != 64` 时，不做传统“图像滤波缩放”，而采用 **参数语义缩放** 来尽量保持观感：

- `noise_scale' = noise_scale * (64 / size)`
- `warp_strength' = warp_strength * (size / 64)`

直觉：让噪声周期与 warp 位移在“像素尺度”上尽量保持一致，输出更清晰且不引入缩放模糊。

## Examples
```json
{
  "schema_version": 1,
  "textures": [
    {
      "name": "minecraft_grass",
      "size": 16,
      "seed": 9001,
      "octaves": 3,
      "noise_scale": 2.8,
      "warp_strength": 0.1,
      "palette": [[48,94,36],[62,116,44],[85,140,58],[120,170,78]],
      "style": "minecraft_quantized"
    }
  ]
}
```

字段状态：

- `schema_version`: implemented（必须为 `1`，否则报错）。
- `textures[].name`: implemented（用于 `TextureRegistry`，必须唯一）。
- `size/seed/octaves/noise_scale/warp_strength/palette`: implemented。
- `style`: implemented as data field；当前 shader 仅真正支持 `minecraft_quantized`，其他值会告警并回退。

## v2 (planned)

以下仅为规划结构，不代表已完成 shader 行为：

- `faces`: `top/bottom/sides` 与 `north/south/east/west`
- `has_layer/layer_ratio/top_layer_palette`
- 多风格完整 shader 分支

## Validation (implemented)

Loader 在资产加载阶段 fail-fast：

- 空纹理数组
- 重名
- `size == 0`
- `noise_scale <= 0`
- 非法 `schema_version`
- 超过 `MAX_LAYERS`

## Registry contract (implemented)

- 启动时构建 `TextureRegistry(name -> layer_index)`。
- block/material 不应再依赖 JSON 顺序约定。
- 引用不存在名字时必须报错。

# Cruft WorldGen（Modern Surface / 可插拔）

本文档定义当前阶段（仅地表）的 worldgen 契约：

- 硬切换到 `WorldGenerator` 可插拔接口
- 默认 preset 为 `modern_surface`
- 本阶段仅生成地表（无洞穴/矿脉/结构）

## 1. 接口契约（写死）

- `WorldGenerator::sample_surface_height(wx, wz) -> i32`
- `WorldGenerator::generate_chunk(chunk_key) -> GeneratedChunk`
- `GeneratedChunk` 当前主路径为 `SurfaceColumns([ColumnSurface; 32*32])`

`ColumnSurface` 字段：

- `height`: 该列地表高度
- `top`: 顶层方块
- `filler`: 表层填充方块
- `stone_depth`: 地表层厚度（其下回落到 STONE）

## 2. Modern Surface 参数（写死默认）

- `sea_level = 24`
- `warp_scale = 1/1024`
- `warp_amplitude = 96`
- `continental_gain = 52`
- `mountain_gain = 72`
- `erosion_gain = 26`
- `detail_gain = 8`
- `mountain_start = 0.15`
- `mountain_end = 0.75`

## 3. 高度公式（写死）

`height = sea_level + c*52 + mountain_mask*72 - e*26 + d*8`

其中：

- `c`: continentalness
- `e`: erosion
- `d`: detail
- `mountain_mask = smoothstep(0.15, 0.75, ridge)`

## 4. 生物群系与地表材质（当前阶段）

- `Plains` -> `GRASS / DIRT`
- `Beach` -> `SAND / SAND`
- `Rocky` -> `GRAVEL / STONE`
- `SnowPeak` -> `SNOW / STONE`

说明：这是“地表材质规则”，不是完整 biome 生态系统。

## 5. 与存档契约

- worldgen 配置写入 `header.cruft`（`WorldHeaderV2`）
- 运行时以 `header.cruft` 为权威，不从 UI/名称临时派生
- 旧格式（非 v2）不兼容读取

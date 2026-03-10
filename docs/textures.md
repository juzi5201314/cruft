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


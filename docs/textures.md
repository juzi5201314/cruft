# Procedural Voxel Texture Generation Pipeline

这套方案是一个完全数据驱动的程序化体素贴图生成管线，旨在将自然语言解耦为极轻量级的 JSON 配置，进而通过纯数学算法实时渲染出支持无缝平铺（Seamless Tiling）的多分辨率（16x16 至 1024x1024）方块贴图。

**数据格式**: 采用包含 `name`、`size`、`seed`、`style` 以及 `faces`（支持定义 `all`、`top/bottom/sides` 与可选的 `north/south/east/west`）的 JSON 结构，每个面可独立配置基础属性（调色板 `palette`、噪声缩放 `noise_scale`、细节层级 `octaves`、扭曲强度 `warp_strength`）与动态分层逻辑（`has_layer` 配合 `layer_ratio` 生成如“雪覆盖泥土”的参差边界）。

**底层算法**: 基于带环绕映射（Wrap Mode）的分形噪声与定向梯度浮雕光照，并集成了四种可切换的核心渲染风格。

**核心渲染风格（Styles）**：`minecraft`（严格阶梯量化的经典像素风）、`hd_pixel_art`（结合 8x8 Bayer 矩阵抖动与定义域扭曲的高清复古点阵风）、`hd_realistic`（基于连续线性插值与流体扭曲的平滑拟真风）以及 `vector_toon`（基于邻域差分的边缘检测与描边的赛璐璐风）。该方案实现了高度的工业化，LLM 只需输出控制大自然物理法则的参数集，即可在引擎端零空间占用地产出完美无缝、支持复杂光影与多面异构的高质量游戏资产。

## 设计目标 / 边界

- **Seamless Tiling 的定义**：这里指同一张 2D 贴图在平面上重复拼接无接缝；并不自动保证 cube 不同面之间（top↔sides 等）的边界连续。
- **可复现性**：建议把 `seed` 作为一等公民（相同 JSON ⇒ 相同输出），便于构建缓存、联机一致性与回归测试。
- **生产实现**：下方 Python 代码是“参考实现/快速预览”，不是实时渲染实现；生产建议在引擎侧（Rust/Shader）实现并做 SIMD/GPU 优化。
- **色彩空间**：输出贴图默认作为 sRGB 的 Albedo/Color；若后续扩展高度/法线/粗糙度等，应以线性空间处理相应通道。

## JSON Schema（草案）

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
  - `noise_scale`（float，>0，可选，默认 1.0）：噪声缩放。
  - `octaves`（int，>=1，可选，默认 4）：分形层数。
  - `warp_strength`（float，>=0，可选，默认 0.0）：定义域扭曲强度；0 关闭。

输出约定（示例实现）：
- 生成器按 `faces` 中出现的键输出 `{name}_{key}.png`（例如 `top`/`bottom`/`sides` 或 `north` 等）。
- 引擎侧面选择建议按优先级：`north/south/east/west` > `top/bottom` > `sides` > `all`。

## 校验规则（建议）

- `palette` 至少 2 个颜色（否则大部分风格会缺乏层次）。
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

## Bevy + WGSL（启动时 GPU 动态生成贴图）

目标：在 **游戏启动时** 用 WGSL compute shader 直接把程序化贴图写入 GPU（显存）中的纹理资源，然后把这张纹理当作普通 `Image`/材质贴图去使用，尽量做到 **零 CPU 像素循环、零 GPU→CPU 回读**。

本仓库示例采用 **texture array**（`texture_2d_array`）：一次 dispatch 批量生成 N 层，每层一张 64×64 贴图。

### 推荐架构（高性能）

- **主世界（Main World）**：只负责“声明要生成什么”
  - 创建目标 `Image`（带 `STORAGE_BINDING` 用途）并保留 `Handle<Image>` 供材质引用
  - 把生成参数（seed、palette、style、噪声参数等）放在 `Resource` 里
- **渲染子世界（RenderApp / Render World）**：负责“真正生成”
  - `RenderStartup`：创建 bind group layout + queue compute pipeline（`PipelineCache`）
  - `PrepareBindGroups`：把 `GpuImage.texture_view` + uniform buffer 组成 bind group
  - `RenderGraph Node`：在每帧渲染前 dispatch compute；**只跑 1 次**（生成完成后进入 Done 状态）

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
  - `BindGroupLayoutDescriptor`（`texture_storage_2d_array` + `uniform_buffer`）
  - `pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor { .. })`
- `RenderSystems::PrepareBindGroups`：
  - 从 `RenderAssets<GpuImage>` 取 `GpuImage.texture_view`
  - 用 `UniformBuffer::from(params)` + `write_buffer()` 上传参数
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
    "name": "snowy_dirt",
    "size": 32,
    "seed": 1337,
    "style": "minecraft",
    "faces": {
        "top": {
            "has_layer": false,
            "base": {
                "palette": [
                    [
                        220,
                        220,
                        235
                    ],
                    [
                        240,
                        240,
                        255
                    ],
                    [
                        255,
                        255,
                        255
                    ]
                ],
                "noise_scale": 1.5,
                "octaves": 2
            }
        },
        "sides": {
            "has_layer": true,
            "layer_ratio": 0.35,
            "top_layer_palette": [
                [
                    220,
                    220,
                    235
                ],
                [
                    240,
                    240,
                    255
                ],
                [
                    255,
                    255,
                    255
                ]
            ],
            "base": {
                "palette": [
                    [
                        71,
                        47,
                        28
                    ],
                    [
                        91,
                        62,
                        38
                    ],
                    [
                        110,
                        80,
                        50
                    ]
                ],
                "noise_scale": 2.0,
                "octaves": 4,
                "warp_strength": 0.0
            }
        },
        "bottom": {
            "has_layer": false,
            "base": {
                "palette": [
                    [
                        71,
                        47,
                        28
                    ],
                    [
                        91,
                        62,
                        38
                    ],
                    [
                        110,
                        80,
                        50
                    ]
                ],
                "noise_scale": 2.0,
                "octaves": 4
            }
        }
    }
}
```

Python 参考实现依赖：`numpy`、`pillow`、`scipy`。

```python
import json
import zlib

import numpy as np
from PIL import Image
from scipy.ndimage import map_coordinates


# --- MODIFY THIS BLOCK WITH GENERATED JSON ---

DATA_JSON = r"""
{
    "name": "snowy_dirt",
    "size": 32,
    "seed": 1337,
    "style": "minecraft",
    "faces": {
        "all": {
            "has_layer": false,
            "base": {
                "palette": [
                    [71, 47, 28],
                    [110, 80, 50]
                ],
                "noise_scale": 2.0,
                "octaves": 4,
                "warp_strength": 0.0
            }
        }
    }
}
"""
DATA = json.loads(DATA_JSON)

# ---------------------------------------------


def stable_u32(text: str) -> int:
    return zlib.crc32(text.encode("utf-8")) & 0xFFFFFFFF


def face_seed(global_seed: int, face_name: str) -> int:
    return (int(global_seed) ^ stable_u32(face_name)) & 0xFFFFFFFF


def normalize01(arr: np.ndarray) -> np.ndarray:
    min_v = float(arr.min())
    max_v = float(arr.max())
    denom = max_v - min_v
    if denom <= 1e-12:
        return np.zeros_like(arr, dtype=np.float32)
    return ((arr - min_v) / denom).astype(np.float32)


def generate_seamless_noise(size, octaves, rng, scale=1.0):
    noise = np.zeros((size, size), dtype=np.float32)
    amplitude, frequency = 1.0, 1.0
    y, x = np.mgrid[0:size, 0:size]
    for _ in range(octaves):
        period = max(1, int(size / (scale * frequency)))
        grid = rng.random((period, period)).astype(np.float32)
        coords = [y * period / size, x * period / size]
        octave_noise = map_coordinates(grid, coords, order=1, mode="wrap")
        noise += octave_noise.astype(np.float32) * amplitude
        amplitude *= 0.5
        frequency *= 2.0

    return normalize01(noise)


def domain_warp(size, base_noise, rng, warp_strength):
    if warp_strength <= 0:
        return base_noise
    warp_x = generate_seamless_noise(size, 4, rng, 2.0) * warp_strength * size
    warp_y = generate_seamless_noise(size, 4, rng, 2.0) * warp_strength * size
    y, x = np.mgrid[0:size, 0:size]
    warp_coords = [(y + warp_y) % size, (x + warp_x) % size]
    return map_coordinates(base_noise, warp_coords, order=1, mode="wrap")


def apply_emboss(noise, strength=0.2):
    shifted = np.roll(noise, shift=(-1, -1), axis=(0, 1))
    return np.clip(noise + (noise - shifted) * strength, 0, 1)


def color_minecraft(val, palette):
    idx = int(np.clip(val * len(palette), 0, len(palette) - 1))
    return palette[idx]


def color_realistic(val, palette):
    n = len(palette) - 1
    idx_exact = val * n
    idx_floor = int(np.floor(idx_exact))
    idx_ceil = min(n, idx_floor + 1)
    frac = idx_exact - idx_floor
    c1, c2 = np.array(palette[idx_floor]), np.array(palette[idx_ceil])
    return (c1 * (1.0 - frac) + c2 * frac).astype(np.uint8)


BAYER_MATRIX = (
    np.array(
        [
            [0, 32, 8, 40, 2, 34, 10, 42],
            [48, 16, 56, 24, 50, 18, 58, 26],
            [12, 44, 4, 36, 14, 46, 6, 38],
            [60, 28, 52, 20, 62, 30, 54, 22],
            [3, 35, 11, 43, 1, 33, 9, 41],
            [51, 19, 59, 27, 49, 17, 57, 25],
            [15, 47, 7, 39, 13, 45, 5, 37],
            [63, 31, 55, 23, 61, 29, 53, 21],
        ]
    )
    / 64.0
)


def color_hd_pixel(val, x, y, palette, levels=4):
    spread = 1.0 / levels
    threshold = BAYER_MATRIX[y % 8, x % 8] - 0.5
    return color_minecraft(np.clip(val + threshold * spread, 0.0, 0.999), palette)


def generate_single_face(size, style, face_data, rng):
    base_cfg = face_data["base"]
    pal = base_cfg["palette"]
    noise = generate_seamless_noise(
        size, base_cfg.get("octaves", 4), rng, base_cfg.get("noise_scale", 1.0)
    )
    warp_str = base_cfg.get("warp_strength", 0.0)
    if warp_str > 0:
        noise = domain_warp(size, noise, rng, warp_str)
    light_str = 0.1 if style == "hd_realistic" else 0.25
    noise = apply_emboss(noise, light_str)
    edges = np.zeros((size, size))
    if style == "vector_toon":
        shifted_x = np.roll(noise, shift=1, axis=1)
        shifted_y = np.roll(noise, shift=1, axis=0)
        edges = (np.abs(noise - shifted_x) + np.abs(noise - shifted_y)) > 0.04
    img_array = np.zeros((size, size, 3), dtype=np.uint8)
    for y in range(size):
        for x in range(size):
            n_val = noise[y, x]
            # Layering Logic
            active_pal = pal
            if face_data.get("has_layer"):
                boundary_y = (
                    size * face_data["layer_ratio"] + (noise[y, x] - 0.5) * size * 0.5
                )
                if y < boundary_y:
                    active_pal = face_data["top_layer_palette"]

            if style == "minecraft":
                color = color_minecraft(n_val, active_pal)

            elif style == "hd_realistic":
                color = color_realistic(n_val, active_pal)

            elif style == "hd_pixel_art":
                color = color_hd_pixel(n_val, x, y, active_pal, len(active_pal))

            elif style == "vector_toon":
                if edges[y, x]:
                    color = (20, 20, 20)
                else:
                    color = color_minecraft(int(n_val * 4) / 4.0, active_pal)
            img_array[y, x] = color

    return Image.fromarray(img_array)


def generate_and_save_faces():
    size = DATA["size"]
    style = DATA.get("style", "minecraft")
    name = DATA["name"]
    global_seed = int(DATA["seed"])
    faces = DATA["faces"]
    generated_files = []

    for face_name in sorted(faces.keys()):
        face_data = faces[face_name]
        rng = np.random.default_rng(face_seed(global_seed, face_name))
        img = generate_single_face(size, style, face_data, rng)
        filename = f"{name}_{face_name}.png"
        img.save(filename)
        generated_files.append(filename)
        print(f"Generated: {filename}")

    return generated_files


if __name__ == "__main__":
    generate_and_save_faces()

```

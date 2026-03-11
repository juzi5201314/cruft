# Cruft Procedural Texture Specification

本文档定义 **Cruft 唯一正式、完整、可生产落地** 的程序化纹理规范。  
本文中的 JSON 结构、字段语义、求值顺序、坐标约定、颜色空间、失败策略、确定性要求与导出契约，均为**唯一权威定义**。

本文不讨论兼容层、迁移层、过渡层，也不采用 “implemented / planned” 双口径。  
任何实现、工具链、运行时、导出器、CI 校验器，都必须以本文为唯一依据。

---

## 1. 规范词与合规级别

本文使用以下规范词：

- **MUST**：必须；违反即视为实现错误或资产错误。
- **MUST NOT**：禁止；违反即视为实现错误或资产错误。
- **SHOULD**：强烈建议；如不遵循，必须有明确理由并承担后果。
- **SHOULD NOT**：不建议；如采用，必须明知其后果。
- **MAY**：可选；实现可按需要支持。

本文定义四类合规实现：

1. **Parser**
   - 负责读取、解析、校验 JSON 结构与基础字段类型。

2. **Compiler**
   - 负责默认值展开、简写展开、引用解析、信号图拓扑排序、face 解析、规范化与指纹生成。

3. **Runtime Generator**
   - 负责运行时生成逻辑贴图、构建 registry、上传 GPU 资源、暴露状态机。

4. **Exporter**
   - 负责把逻辑贴图或打包贴图导出为 PNG/EXR/KTX2 等离线文件。

一个实现可以同时承担多类角色，但只要宣称支持某类角色，就必须满足对应要求。

---

## 2. 设计目标

本规范的目标是：

1. 用**单一、固定、严格**的 schema 覆盖方块/体素材质的程序化纹理生成。
2. 用 **surface + texture + face binding** 的模型拆分“可复用表面定义”和“游戏内引用材质”。
3. 用**显式生成参数**替代模糊风格枚举，避免“写错了仍然 fallback 成某个风格”的隐性错误。
4. 明确定义 **face precedence / face UV / block-space 坐标 / seam 连续性**。
5. 同时支撑 **运行时生成** 与 **离线导出**，并保证两者共享完全一致的语义。
6. 从一开始就把 **albedo / normal / height / roughness / ao / metallic / emissive / opacity** 纳入统一模型。
7. 明确 **fail-fast / determinism / canonicalization / cache fingerprint / CI 质量门槛**。
8. 让作者资产、loader、compiler、runtime、exporter 的行为保持一致，避免“文档会写、实现不接”的漂移。

---

## 3. 非目标

本规范不定义以下内容：

- 方块/物品的物理、交互、掉落、配方、玩法属性
- chunk、mesh、LOD、光照系统、材质系统的完整实现
- 世界生成、生态区着色、时变特效、动态湿度/雪盖等高阶玩法逻辑
- 任意通用材质图编辑器语言或完整节点图系统

如需扩展上述能力，必须通过本文定义的扩展机制进行，而不是在 core 字段中“悄悄夹带”。

---

## 4. 核心对象模型

本文把程序化纹理资产拆成五层：

### 4.1 Texture Set

一个 `*.texture.json` 文件表示一个 **texture set**。  
它是唯一的顶层编译单元，包含：

- 顶层元数据
- 输出规则
- 默认值
- 一组可复用的 `surfaces`
- 一组游戏内可引用的 `textures`

### 4.2 Surface

`surface` 表示“如何生成一类可复用表面”的定义。  
它负责描述：

- 坐标域
- 逻辑像素密度
- 种子
- 命名信号图
- base layer
- 后续叠加 layers
- normal 生成方式

一个 surface 本身不直接等于游戏里的某个 block。  
它是可复用表面模板，例如：

- `dirt`
- `grass_top`
- `grass_side`
- `stone`
- `iron_ore_surface`

### 4.3 Signal

`signal` 是 surface 内部的**命名标量信号**。  
它始终输出 `0..1` 的标量，用于：

- 驱动调色板映射
- 生成 height / roughness / ao / metallic / opacity
- 生成 mask
- 做层间复用，避免同一噪声重复声明并导致细节漂移

signal 图必须是**有向无环图**。

### 4.4 Texture

`texture` 是“一个可被游戏内系统引用的命名材质”。  
它负责：

- 给出对外可引用的 `name`
- 把六个物理面绑定到某些 surfaces
- 定义 sampler、透明度模式与材质级行为

### 4.5 Face Binding

face binding 负责把一个 texture 的某个面映射到某个 surface，并可携带：

- 旋转
- 翻转

它是 surface 与最终物理面的最后一层连接。

---

## 5. 顶层 JSON 结构

### 5.1 顶层对象

每个 `*.texture.json` 文件 MUST 是一个严格 JSON 对象，结构如下：

```json
{
  "spec": "cruft.procedural_texture",
  "spec_version": "1.0.0",
  "profile": "voxel_cube_pbr",
  "meta": {},
  "output": {},
  "defaults": {},
  "surfaces": {},
  "textures": {},
  "extensions_used": [],
  "extensions": {}
}
```

### 5.2 顶层字段说明

#### `spec`

- 类型：`string`
- 必填：是
- 固定值：`"cruft.procedural_texture"`

#### `spec_version`

- 类型：`string`
- 必填：是
- 固定值：`"1.0.0"`

#### `profile`

- 类型：`string`
- 必填：是
- 固定值：`"voxel_cube_pbr"`

`profile` 定义几何语义、面命名、block-space 映射与输出通道契约。  
当前 core 规范只定义一个 profile：`voxel_cube_pbr`。

#### `meta`

- 类型：`object`
- 必填：否

推荐字段：

- `author: string`
- `license: string`
- `description: string`
- `source: string`
- `tags: string[]`

`meta` 仅用于描述，不得参与渲染结果与缓存指纹计算，除非实现明确把其纳入外部构建系统。

#### `output`

- 类型：`object`
- 必填：是

定义输出分辨率、mipmap 规则与 normal 编码方向。详见第 10 节。

#### `defaults`

- 类型：`object`
- 必填：否

定义 `surface` 与 `texture` 的默认值。详见第 9 节。

#### `surfaces`

- 类型：`object`
- 必填：是
- 约束：至少 1 个 surface

键名为 surface 名称，值为 `SurfaceSpec`。

#### `textures`

- 类型：`object`
- 必填：是
- 约束：至少 1 个 texture

键名为 texture 名称，值为 `TextureSpec`。

#### `extensions_used`

- 类型：`string[]`
- 必填：否
- 默认：空数组

列出本文件显式使用的扩展 ID。

#### `extensions`

- 类型：`object`
- 必填：否
- 默认：空对象

承载扩展 payload。扩展 ID 必须与 `extensions_used` 对齐。

---

## 6. 严格 JSON 规则

为了避免不同 parser 的宽松行为导致资产结果漂移，必须遵守以下规则：

1. 源文件 MUST 是 **UTF-8 编码的严格 JSON**。
2. 源文件 MUST NOT 使用 JSON5 / HJSON / 注释 / trailing comma。
3. **重复对象键 MUST 视为错误**；禁止 last-wins 或 first-wins。
4. 未知字段 MUST 视为错误，除非它位于已声明且受支持的扩展 payload 中。
5. 所有数字 MUST 是有限数；`NaN`、`Infinity`、`-Infinity` 一律非法。
6. 所有整数字段 MUST 在其定义的合法范围内；禁止无界自动截断。
7. 所有名称、枚举值、路径字符串 MUST 区分大小写。
8. 除 `meta` 与 `extensions` 外，对象键顺序 MUST NOT 影响语义与结果。
9. 同一文件中的空对象 `{}` 与省略字段在有默认值时可能等价，但 compiler MUST 先做规范化后再计算缓存指纹。

---

## 7. 基础数据类型

### 7.1 Name

所有 `surface` 名称、`texture` 名称、`signal` 名称 MUST 满足：

```text
^[a-z0-9][a-z0-9_]*$
```

原因：

- 便于作为 registry key
- 便于导出命名
- 便于日志、路径、shader debug 输出
- 避免跨平台大小写与符号差异

### 7.2 Color

core `Color` 使用字符串表示，格式必须为：

```text
#RRGGBB
```

示例：

- `"#305E24"`
- `"#78AA4E"`
- `"#F0B166"`

颜色语义：

- 作者输入始终按 **sRGB authored color** 解释
- 在混合、渐变插值、mipmap 生成、PBR 运算前 MUST 转换到**线性空间**
- core 规范中不允许 `#RRGGBBAA`
- 不透明度 MUST 通过 `opacity` 标量通道表达，而不是借道颜色 alpha

### 7.3 Scalar

除非字段另有说明，标量使用 `number`，推荐范围 `0..1`。

### 7.4 Integer

除 `seed` 相关字段外，整数字段默认按有符号 32 位整数解释。  
若字段声明为非负，则 `value < 0` 必须报错。

### 7.5 Seed

`seed` 与 `seed_offset` 按 `u32` 语义解释，范围：

```text
0..4294967295
```

任何超界值都必须报错，不允许自动模运算。

### 7.6 Vec2 / Vec3

- `Vec2`：长度为 2 的数字数组
- `Vec3`：长度为 3 的数字数组

示例：

```json
[1.0, 1.0]
[1.0, 1.0, 1.0]
```

### 7.7 Palette

`palette` 有两种合法写法。

#### 简写形式

```json
["#305E24", "#3E742C", "#558C3A", "#78AA4E"]
```

含义：颜色停靠点按 `[0,1]` 区间均匀分布。

#### 显式停靠点形式

```json
[
  { "at": 0.0, "color": "#305E24" },
  { "at": 0.4, "color": "#3E742C" },
  { "at": 0.8, "color": "#558C3A" },
  { "at": 1.0, "color": "#78AA4E" }
]
```

约束：

- 停靠点数 MUST 为 `2..64`
- `at` MUST 在 `0..1`
- 第一个停靠点 MUST 为 `0.0`
- 最后一个停靠点 MUST 为 `1.0`
- `at` MUST 严格递增

---

## 8. 扩展机制

扩展 ID 格式 MUST 为：

```text
<vendor>.<name>
```

示例：

- `cruft.biome_tint`
- `studio.custom_export`

规则：

1. 文件若使用扩展，MUST 在 `extensions_used` 中列出。
2. `extensions_used` 中列出的扩展，MUST 在 `extensions` 中具有对应 payload。
3. compiler / runtime / exporter 若不支持某个 `extensions_used` 项，MUST 直接失败。
4. core 实现 MUST NOT 静默忽略未知扩展。
5. 扩展 payload 只能放在 `extensions` 对象中；不得把自定义字段直接塞进 core 对象。

---

## 9. 默认值与继承规则

### 9.1 `defaults` 结构

```json
{
  "surface": {},
  "texture": {}
}
```

### 9.2 `defaults.surface`

推荐默认值：

```json
{
  "logical_size": 16,
  "pixel_snap": true,
  "domain": "face_uv",
  "tile_mode": "repeat",
  "seed": 0,
  "normal": {
    "mode": "flat",
    "strength": 1.0
  }
}
```

### 9.3 `defaults.texture`

推荐默认值：

```json
{
  "sampler": {
    "mag_filter": "nearest",
    "min_filter": "nearest",
    "mipmap_filter": "nearest",
    "anisotropy": 1,
    "address_u": "clamp_to_edge",
    "address_v": "clamp_to_edge"
  },
  "alpha_mode": "opaque",
  "cutout_threshold": 0.5
}
```

### 9.4 继承规则

继承顺序 MUST 为：

1. core 默认值
2. `defaults.surface` 或 `defaults.texture`
3. 对象自身显式字段

规则：

- 只有“未提供字段”才继承默认值
- 显式字段始终覆盖继承值
- `null` 不是合法“清空默认值”的手段；core 字段不允许使用 `null`

---

## 10. 输出规则 `output`

### 10.1 结构

```json
{
  "size": 64,
  "mipmaps": "full",
  "normal_format": "opengl"
}
```

### 10.2 字段说明

#### `size`

- 类型：`integer`
- 必填：是
- 约束：`16..1024`
- 约束：MUST 为 2 的幂

语义：每个 face、每个逻辑通道的输出分辨率。

#### `mipmaps`

- 类型：`string`
- 必填：否
- 默认：`"full"`
- 允许值：`"none" | "full"`

语义：

- `none`：不生成 mip
- `full`：生成完整 mip 链直到 `1x1`

#### `normal_format`

- 类型：`string`
- 必填：否
- 默认：`"opengl"`
- 允许值：`"opengl" | "directx"`

语义：控制最终 normal 图的绿色通道方向。

---

## 11. Surface 结构

### 11.1 `SurfaceSpec` 总体结构

```json
{
  "logical_size": 16,
  "pixel_snap": true,
  "domain": "face_uv",
  "tile_mode": "repeat",
  "seed": 0,
  "signals": {},
  "base": {},
  "layers": [],
  "normal": {}
}
```

### 11.2 字段说明

#### `logical_size`

- 类型：`integer`
- 必填：否
- 默认：继承 `defaults.surface.logical_size`
- 约束：`4..output.size`
- SHOULD 为 2 的幂

语义：作者意图的逻辑像素密度。

#### `pixel_snap`

- 类型：`boolean`
- 必填：否
- 默认：继承 `defaults.surface.pixel_snap`

语义：

- `true`：在逻辑网格上采样，再映射到 `output.size`
- `false`：直接在 `output.size` 连续采样

#### `domain`

- 类型：`string`
- 必填：否
- 默认：继承 `defaults.surface.domain`
- 允许值：`"face_uv" | "block_space"`

#### `tile_mode`

- 类型：`string`
- 必填：否
- 默认：继承 `defaults.surface.tile_mode`
- 允许值：`"repeat" | "clamp"`

#### `seed`

- 类型：`integer`
- 必填：否
- 默认：继承 `defaults.surface.seed`
- 范围：`u32`

#### `signals`

- 类型：`object`
- 必填：否
- 默认：空对象

键名为 signal 名称，值为 `SignalSpec`。

#### `base`

- 类型：`object`
- 必填：是

定义基础层。`base.albedo` MUST 存在。

#### `layers`

- 类型：`array`
- 必填：否
- 默认：空数组
- 约束：最多 16 层

按数组顺序从前到后叠加。

#### `normal`

- 类型：`object`
- 必填：否
- 默认：继承 `defaults.surface.normal`

---

## 12. Signal 图

### 12.1 总规则

signal 图用于复用中间标量结果。  
所有 signal 都输出 `0..1`。

规则：

1. signal 名称 MUST 全局唯一（在 surface 内）。
2. signal 图 MUST 是有向无环图。
3. signal 的声明顺序不得影响语义。
4. compiler MUST 进行拓扑排序。
5. 任意未定义引用 MUST 报错。
6. 任意循环依赖 MUST 报错。

### 12.2 `SignalSpec` 允许的 `kind`

- `constant`
- `noise`
- `curve`
- `combine`
- `mask`

### 12.3 `constant`

```json
{
  "kind": "constant",
  "value": 0.5
}
```

约束：

- `value` MUST 在 `0..1`

### 12.4 `noise`

```json
{
  "kind": "noise",
  "noise": {},
  "remap": {}
}
```

说明：

- `noise` MUST 存在
- `remap` 可省略；若省略则使用默认 remap

### 12.5 `curve`

```json
{
  "kind": "curve",
  "source": "base_noise",
  "curve": {},
  "clamp": [0.0, 1.0]
}
```

说明：

- `source` MUST 引用已定义 signal
- `curve` MUST 存在
- `clamp` 可省略；默认 `[0,1]`

### 12.6 `combine`

```json
{
  "kind": "combine",
  "op": "add",
  "inputs": ["a", "b"],
  "clamp": [0.0, 1.0]
}
```

`op` 允许值：

- `add`
- `multiply`
- `min`
- `max`
- `subtract`
- `average`

规则：

- `inputs` 长度：
  - `subtract` MUST 为 2
  - 其余 MUST 为 `2..8`
- `subtract` 结果为 `max(a - b, 0)`
- `average` 为算术平均
- 若 `clamp` 缺失，默认 `[0,1]`

### 12.7 `mask`

```json
{
  "kind": "mask",
  "mask": {}
}
```

说明：

- `mask` MUST 为合法 `MaskSpec`
- mask signal 仍是普通 signal，可被 color/scalar/mask 重用

---

## 13. Base 与 Layer 结构

### 13.1 通道集合

surface 的 `base` 与每个 `layer` 允许写入以下逻辑通道：

- `albedo`
- `height`
- `roughness`
- `ao`
- `metallic`
- `emissive`
- `opacity`

其中：

- `albedo` 是唯一必需的颜色通道
- `normal` 不直接由 layer 写入，而由 `surface.normal` 统一生成
- 缺失通道使用默认值，见第 13.5 节

### 13.2 `base` 结构

```json
{
  "albedo": {},
  "height": {},
  "roughness": {},
  "ao": {},
  "metallic": {},
  "emissive": {},
  "opacity": {}
}
```

规则：

- `base.albedo` MUST 存在
- 其余通道可省略

### 13.3 `layer` 结构

```json
{
  "name": "grass_cap",
  "mask": {},
  "strength": 1.0,
  "albedo": {},
  "height": {},
  "roughness": {},
  "ao": {},
  "metallic": {},
  "emissive": {},
  "opacity": {},
  "blend": {}
}
```

字段说明：

#### `name`

- 类型：`string`
- 必填：否

仅用于日志、调试和错误路径增强。

#### `mask`

- 类型：`object`
- 必填：是

定义该 layer 生效范围。

#### `strength`

- 类型：`number`
- 必填：否
- 默认：`1.0`
- 约束：`0..1`

它是 layer 的总强度因子。

#### 各通道字段

- 类型：对应的 field spec
- 必填：否
- 约束：每个 layer 至少必须写入 1 个通道

#### `blend`

- 类型：`object`
- 必填：否

定义每个通道的混合模式。若省略，则所有已写通道默认 `mix`。

### 13.4 通道允许的 blend mode

#### 颜色通道

- `albedo`：只允许 `mix`
- `emissive`：只允许 `mix`

这样可以避免颜色加法/乘法在不同后端与颜色空间中产生歧义。

#### 标量通道

- `height`
- `roughness`
- `ao`
- `metallic`
- `opacity`

允许值：

- `mix`
- `add`
- `multiply`
- `max`
- `min`

### 13.5 通道默认值

若一个 surface 在最终合成后没有得到某个通道，则 MUST 使用：

- `height = 0.5`
- `roughness = 1.0`
- `ao = 1.0`
- `metallic = 0.0`
- `emissive = #000000`
- `opacity = 1.0`
- `normal = flat`

---

## 14. Color Field Spec

Color field 仅用于：

- `albedo`
- `emissive`

### 14.1 结构

```json
{
  "mode": "palette",
  "value": "#FFFFFF",
  "source": "base_noise",
  "noise": {},
  "palette": [],
  "mapping": {},
  "intensity": 1.0
}
```

### 14.2 `mode`

允许值：

- `constant`
- `palette`

### 14.3 `constant`

当 `mode = "constant"` 时：

- `value` MUST 存在
- `source` MUST NOT 存在
- `noise` MUST NOT 存在
- `palette` MUST NOT 存在
- `mapping` MUST NOT 存在

### 14.4 `palette`

当 `mode = "palette"` 时：

- `palette` MUST 存在
- `source` 与 `noise` 必须二选一
- `mapping` 可省略；若省略使用默认 mapping

说明：

- `source` 表示使用命名 signal 作为输入
- `noise` 表示 inline noise shorthand，compiler MUST 将其规范化为匿名 signal 语义再求值

### 14.5 `intensity`

- 类型：`number`
- 必填：否
- 默认：`1.0`
- 约束：`>= 0`

规则：

- 仅在目标通道是 `emissive` 时有意义
- 若目标通道是 `albedo` 且提供 `intensity != 1.0`，必须报错

---

## 15. Scalar Field Spec

Scalar field 用于：

- `height`
- `roughness`
- `ao`
- `metallic`
- `opacity`

### 15.1 结构

```json
{
  "mode": "signal",
  "value": 1.0,
  "source": "base_noise",
  "noise": {},
  "remap": {}
}
```

### 15.2 `mode`

允许值：

- `constant`
- `signal`
- `noise`

### 15.3 `constant`

当 `mode = "constant"` 时：

- `value` MUST 存在
- `source` MUST NOT 存在
- `noise` MUST NOT 存在
- `remap` MUST NOT 存在

### 15.4 `signal`

当 `mode = "signal"` 时：

- `source` MUST 存在
- `noise` MUST NOT 存在
- `remap` 可省略；省略时使用默认 remap

### 15.5 `noise`

当 `mode = "noise"` 时：

- `noise` MUST 存在
- `source` MUST NOT 存在
- `remap` 可省略；省略时使用默认 remap

---

## 16. Mapping、Remap 与 Curve

### 16.1 Color Mapping

结构：

```json
{
  "mode": "quantized",
  "levels": 0,
  "contrast": 1.0,
  "bias": 0.0,
  "invert": false,
  "dither": "none"
}
```

字段说明：

#### `mode`

允许值：

- `quantized`
- `gradient`

#### `levels`

- 类型：`integer`
- 默认：`0`

规则：

- 仅在 `quantized` 下有效
- `0` 表示自动使用 `palette` 的停靠点数量
- 非 0 时 MUST 在 `2..64`

#### `contrast`

- 类型：`number`
- 默认：`1.0`
- 约束：`> 0`

#### `bias`

- 类型：`number`
- 默认：`0.0`
- 推荐范围：`-1..1`

#### `invert`

- 类型：`boolean`
- 默认：`false`

#### `dither`

- 类型：`string`
- 默认：`"none"`
- 允许值：`"none" | "bayer4"`

规则：

- 仅在 `quantized` 下有效
- `bayer4` 的矩阵 MUST 固定为：

```text
0   8   2  10
12  4  14   6
3  11   1   9
15  7  13   5
```

归一化方式：

```text
threshold = (matrix[x mod 4][y mod 4] + 0.5) / 16
```

### 16.2 默认 Color Mapping

若缺失 `mapping`，默认等价于：

```json
{
  "mode": "quantized",
  "levels": 0,
  "contrast": 1.0,
  "bias": 0.0,
  "invert": false,
  "dither": "none"
}
```

### 16.3 Scalar Remap

结构：

```json
{
  "range": [0.0, 1.0],
  "contrast": 1.0,
  "bias": 0.0,
  "invert": false,
  "curve": {
    "interp": "linear",
    "stops": [
      [0.0, 0.0],
      [1.0, 1.0]
    ]
  },
  "clamp": [0.0, 1.0]
}
```

字段说明：

#### `range`

- 类型：`number[2]`
- 默认：`[0.0, 1.0]`
- 约束：`range[0] <= range[1]`

#### `contrast`

- 类型：`number`
- 默认：`1.0`
- 约束：`> 0`

#### `bias`

- 类型：`number`
- 默认：`0.0`

#### `invert`

- 类型：`boolean`
- 默认：`false`

#### `curve`

- 类型：`object`
- 必填：否

若提供，必须满足 `CurveSpec`。

#### `clamp`

- 类型：`number[2]`
- 默认：`[0.0, 1.0]`
- 约束：`clamp[0] <= clamp[1]`

### 16.4 `CurveSpec`

结构：

```json
{
  "interp": "linear",
  "stops": [
    [0.0, 0.0],
    [0.5, 0.7],
    [1.0, 1.0]
  ]
}
```

规则：

- `interp` 允许值：`"linear" | "smoothstep"`
- `stops` 长度 MUST 为 `2..64`
- `x` 值 MUST 从 `0.0` 开始，以 `1.0` 结束，并严格递增
- `y` 值推荐在 `0..1`；若超出，最终仍会受 `clamp` 约束

### 16.5 统一处理顺序

对所有 `signal -> mapping/remap` 流程，输入标量 `n` 的处理顺序 MUST 为：

1. 原始输入 `n ∈ [0,1]`
2. 若 `invert = true`，则 `n = 1 - n`
3. `n = (n - 0.5) * contrast + 0.5 + bias`
4. `n = clamp(n, 0, 1)`
5. 若定义了 `curve`，则按 `curve` 重映射
6. 对 scalar，映射到 `range`
7. 执行最终 `clamp`

### 16.6 Palette 求值规则

#### `gradient`

- 把 `n` 作为 `0..1` 连续坐标
- 在 palette 停靠点之间做**线性空间插值**
- 结果再按目标通道编码

#### `quantized`

- 先得到 `n`
- 若 `levels = 0`，令 `levels = palette_stop_count`
- 若 `levels != palette_stop_count`，先把 palette 视为连续 gradient，再从 `[0,1]` 上均匀抽取 `levels` 个离散颜色
- 若 `dither = none`，按离散级直接取色
- 若 `dither = bayer4`，必须使用固定 `4x4` Bayer 阈值抖动

---

## 17. Noise Spec

### 17.1 结构

```json
{
  "basis": "value",
  "fractal": "fbm",
  "cellular_return": "f1",
  "scale": 2.8,
  "stretch": [1.0, 1.0, 1.0],
  "octaves": 4,
  "lacunarity": 2.0,
  "gain": 0.5,
  "offset": [0.0, 0.0, 0.0],
  "seed_offset": 0,
  "warp": {
    "amplitude": 0.0,
    "basis": "gradient",
    "fractal": "fbm",
    "scale_multiplier": 1.0,
    "octaves": 2,
    "lacunarity": 2.0,
    "gain": 0.5,
    "seed_offset": 4096
  }
}
```

### 17.2 字段说明

#### `basis`

允许值：

- `value`
- `gradient`
- `cellular`

#### `fractal`

允许值：

- `none`
- `fbm`
- `billow`
- `ridged`

规则：

- `cellular` 仅允许 `fractal = none`
- `fractal = none` 时，`octaves` MUST 为 `1`

#### `cellular_return`

- 允许值：`"f1"`
- 仅在 `basis = cellular` 时有效
- core 规范只定义 `f1`

#### `scale`

- 类型：`number`
- 必填：是
- 约束：`> 0`

#### `stretch`

- 类型：`number[3]`
- 默认：`[1,1,1]`
- 约束：每项 `> 0`

#### `octaves`

- 类型：`integer`
- 默认：`4`
- 约束：`1..8`

#### `lacunarity`

- 类型：`number`
- 默认：`2.0`
- 约束：`> 1`

#### `gain`

- 类型：`number`
- 默认：`0.5`
- 约束：`0..1`

#### `offset`

- 类型：`number[3]`
- 默认：`[0,0,0]`

#### `seed_offset`

- 类型：`integer`
- 默认：`0`
- 范围：`u32`

#### `warp`

- 类型：`object`
- 必填：否

若提供：

- `amplitude` 默认 `0.0`
- `basis` 默认 `gradient`
- `fractal` 默认 `fbm`
- `scale_multiplier` 默认 `1.0`
- `octaves` 默认 `2`
- `lacunarity` 默认 `2.0`
- `gain` 默认 `0.5`
- `seed_offset` 默认 `4096`

规则：

- `amplitude = 0` 等价于无 warp
- `warp` 的结果必须是确定性的

### 17.3 坐标域语义

#### `face_uv`

- 每个面在自己的二维 UV 域上生成
- 使用 `(u, v, 1)` 作为基础三维采样坐标
- 若存在 face binding 变换，则先应用变换，再参与采样

#### `block_space`

- 在统一的方块局部三维坐标中生成
- 使用 `(x, y, z)` 作为采样坐标
- 若 surface 绑定到多个相邻面，可形成跨边连续性

### 17.4 采样步骤

每次 noise 求值 MUST 按以下顺序执行：

1. 获取基础坐标：
   - `face_uv`：`(u, v, 1)`
   - `block_space`：`(x, y, z)`
2. 若 `pixel_snap = true`，把坐标 snap 到逻辑网格中心
3. 加上 `offset`
4. 乘以 `scale * stretch`
5. 若定义了 `warp` 且 `amplitude > 0`：
   - 用 warp noise 生成三维扰动向量
   - 每个分量先从 `[0,1]` 重映射到 `[-1,1]`
   - 再乘以 `amplitude`
   - 加到当前坐标
6. 若 `tile_mode = repeat`，则对采样坐标按周期 1 做 wrap
7. 按 `basis + fractal` 求值，得到 `0..1` 输出

### 17.5 `pixel_snap` 的精确定义

#### 在 `face_uv` 下

- `u` 和 `v` 必须 snap 到 `logical_size x logical_size` 的像素中心

#### 在 `block_space` 下

- `x`、`y`、`z` 必须 snap 到边长为 `1 / logical_size` 的三维网格中心

规则：

- 若 `pixel_snap = true`，则 `output.size % logical_size == 0` MUST 成立

### 17.6 确定性要求

对同一 canonical 资产，noise 实现 MUST 满足：

- 相同 compiler 版本 + 相同 backend：结果 bit-exact
- 不同 conforming backend：结果在第 28 节定义的容差内一致

---

## 18. Mask Spec

mask 返回 `0..1` 标量，用于控制 layer 的覆盖范围。

### 18.1 允许的 `mode`

- `full`
- `signal`
- `axis_band`
- `threshold`
- `edge_distance`
- `and`
- `or`
- `subtract`
- `not`

### 18.2 `full`

```json
{ "mode": "full" }
```

全域为 1。

### 18.3 `signal`

```json
{
  "mode": "signal",
  "source": "ore_mask"
}
```

规则：

- `source` MUST 引用已定义 signal

### 18.4 `axis_band`

```json
{
  "mode": "axis_band",
  "axis": "v",
  "from": 0.0,
  "to": 0.3,
  "falloff": 0.05,
  "invert": false,
  "jitter": {
    "amount": 0.03,
    "source": "band_jitter"
  }
}
```

字段说明：

- `axis` 允许值：`"u" | "v" | "x" | "y" | "z"`
- `from`、`to`：必须满足 `to > from`
- `falloff`：默认 `0`
- `invert`：默认 `false`
- `jitter.amount`：默认 `0`
- `jitter.source`：可选，必须引用 signal

语义：

- `u/v` 使用**已应用 face binding 变换后的局部 UV**
- `x/y/z` 使用**规范化 block-space 坐标**
- 若提供 `jitter`，则先用 source 生成边界扰动，再计算带宽

### 18.5 `threshold`

```json
{
  "mode": "threshold",
  "source": "ore_noise",
  "threshold": 0.72,
  "softness": 0.08,
  "invert": false
}
```

也允许 inline noise：

```json
{
  "mode": "threshold",
  "noise": {},
  "threshold": 0.72,
  "softness": 0.08
}
```

规则：

- `source` 与 `noise` 必须二选一
- `threshold` MUST 在 `0..1`
- `softness` 默认 `0`
- `softness` MUST 在 `0..1`

数学语义：

- `softness = 0`：硬阈值
- `softness > 0`：以 `threshold` 为中心做 smoothstep 过渡
- 若 `invert = true`，最终结果取 `1 - mask`

### 18.6 `edge_distance`

```json
{
  "mode": "edge_distance",
  "from": 0.0,
  "to": 0.12,
  "falloff": 0.02,
  "invert": false
}
```

规则：

- 只基于**变换后的 face UV**
- 表示当前像素到最近 UV 边界的归一化距离

常见用途：

- 边框
- 砖缝
- 边缘磨损
- 边缘积雪

### 18.7 组合 mask

#### `and`

```json
{
  "mode": "and",
  "items": [maskA, maskB]
}
```

结果为所有子 mask 的 `min()`。

#### `or`

```json
{
  "mode": "or",
  "items": [maskA, maskB]
}
```

结果为所有子 mask 的 `max()`。

#### `subtract`

```json
{
  "mode": "subtract",
  "items": [maskA, maskB]
}
```

结果为：

```text
max(maskA - maskB, 0)
```

#### `not`

```json
{
  "mode": "not",
  "item": maskA
}
```

结果为：

```text
1 - maskA
```

约束：

- `and/or.items` 长度 MUST 为 `2..8`
- `subtract.items` 长度 MUST 为 `2`
- mask 递归深度 MUST 不超过第 30 节规定的上限

---

## 19. Layer 合成规则

### 19.1 总顺序

一个 surface 的求值顺序 MUST 为：

1. 初始化默认通道值
2. 求值 `base`
3. 按数组顺序依次求值 `layers`
4. 得到最终 `albedo / height / roughness / ao / metallic / emissive / opacity`
5. 按 `surface.normal` 生成 normal

### 19.2 Layer 覆盖系数

每个 layer 的实际覆盖系数为：

```text
coverage = clamp(mask * strength, 0, 1)
```

### 19.3 颜色通道合成

对 `albedo` 与 `emissive`：

- 唯一允许的 blend mode 是 `mix`
- 必须在线性空间混合
- 公式为：

```text
out = lerp(prev, layer, coverage)
```

### 19.4 标量通道合成

对 `height / roughness / ao / metallic / opacity`：

#### `mix`

```text
out = lerp(prev, layer, coverage)
```

#### `add`

```text
out = prev + layer * coverage
```

#### `multiply`

```text
out = prev * lerp(1, layer, coverage)
```

#### `max`

```text
out = max(prev, layer * coverage)
```

#### `min`

```text
out = min(prev, lerp(1, layer, coverage))
```

### 19.5 合成后的 clamp 规则

- `height / roughness / ao / metallic / opacity`：MUST clamp 到 `0..1`
- `emissive`：MUST clamp 到 `0..+∞`
- `albedo`：MUST clamp 到 `0..1`（线性空间），编码前再转回 sRGB

---

## 20. Normal Spec

### 20.1 结构

```json
{
  "mode": "derive_from_height",
  "strength": 1.2
}
```

### 20.2 `mode`

允许值：

- `flat`
- `derive_from_height`

### 20.3 `flat`

输出平面法线。

### 20.4 `derive_from_height`

规则：

- MUST 使用**最终合成后的 `height` 通道**
- 若 surface 最终没有有效 `height`，这是加载错误
- `strength` 默认 `1.0`
- `strength` MUST `>= 0`

### 20.5 精确求导算法

在 `output.size x output.size` 的离散纹理上，`derive_from_height` MUST 使用中心差分：

设：

- `du = 1 / output.size`
- `dv = 1 / output.size`

对每个 texel 中心 `(u, v)`：

```text
hx0 = h(u - du, v)
hx1 = h(u + du, v)
hy0 = h(u, v - dv)
hy1 = h(u, v + dv)

dx = hx1 - hx0
dy = hy1 - hy0
```

先构造**图像坐标系**下的法线：

```text
n_img = normalize( -dx * strength, -dy * strength, 1 )
```

其中：

- `x` 对应图像右方向
- `y` 对应图像下方向
- `z` 指向观察者外法线方向

然后编码为目标 normal format：

- `directx`：`n = n_img`
- `opengl`：`n = (n_img.x, -n_img.y, n_img.z)`

最后再编码到 `[0,1]` 纹理空间。

### 20.6 face binding 对 normal 的影响

若 surface 使用 `domain = face_uv` 且该 face 存在旋转/翻转，则：

- 变换 MUST 先作用于所有信号、mask、base、layer 的采样坐标
- 因而 height 与 normal 的方向会自然一致
- 不允许在生成后再“额外旋转 normal 图像”来补偿

---

## 21. Texture 结构

### 21.1 `TextureSpec` 总体结构

```json
{
  "sampler": {},
  "alpha_mode": "opaque",
  "cutout_threshold": 0.5,
  "faces": {}
}
```

### 21.2 字段说明

#### `sampler`

- 类型：`object`
- 必填：否
- 默认：继承 `defaults.texture.sampler`

#### `alpha_mode`

- 类型：`string`
- 必填：否
- 默认：继承 `defaults.texture.alpha_mode`
- 允许值：`"opaque" | "mask" | "blend"`

#### `cutout_threshold`

- 类型：`number`
- 必填：否
- 默认：继承 `defaults.texture.cutout_threshold`
- 约束：`0..1`

#### `faces`

- 类型：`object`
- 必填：是

允许的键：

- `all`
- `top`
- `bottom`
- `sides`
- `north`
- `south`
- `east`
- `west`

值可以是：

1. 字符串：surface 名称
2. `FaceBinding` 对象

---

## 22. Sampler 结构

### 22.1 结构

```json
{
  "mag_filter": "nearest",
  "min_filter": "nearest",
  "mipmap_filter": "nearest",
  "anisotropy": 1,
  "address_u": "clamp_to_edge",
  "address_v": "clamp_to_edge"
}
```

### 22.2 字段说明

- `mag_filter`: `nearest | linear`
- `min_filter`: `nearest | linear`
- `mipmap_filter`: `none | nearest | linear`
- `anisotropy`: `1..16`
- `address_u`: `clamp_to_edge | repeat`
- `address_v`: `clamp_to_edge | repeat`

说明：

- `address_u/v` 只影响**运行时采样**，不影响程序化生成过程
- 若材质系统明确需要重复采样，可使用 `repeat`
- voxel/block 场景下默认应使用 `clamp_to_edge`

---

## 23. FaceBinding

### 23.1 字符串简写

```json
"grass_top"
```

等价于：

```json
{ "surface": "grass_top" }
```

### 23.2 对象结构

```json
{
  "surface": "grass_top",
  "rotate": 0,
  "flip_u": false,
  "flip_v": false
}
```

### 23.3 字段说明

#### `surface`

- 类型：`string`
- 必填：是
- 约束：必须引用已存在 surface

#### `rotate`

- 类型：`integer`
- 默认：`0`
- 允许值：`0 | 90 | 180 | 270`

表示图像空间顺时针旋转。

#### `flip_u`

- 类型：`boolean`
- 默认：`false`

#### `flip_v`

- 类型：`boolean`
- 默认：`false`

### 23.4 变换顺序

对 `domain = face_uv` 的 surface，face binding 变换 MUST 按以下顺序应用：

1. `rotate`
2. `flip_u`
3. `flip_v`

### 23.5 变换适用范围

- 只对 `domain = face_uv` 生效
- 对所有 color/scalar field、mask、height、normal 推导都生效
- 对 `domain = block_space` 的 surface MUST NOT 使用旋转/翻转；否则报错

---

## 24. 面解析优先级

对于最终六个物理面，解析顺序 MUST 为：

### 24.1 `top`

1. `faces.top`
2. `faces.all`
3. 否则报错

### 24.2 `bottom`

1. `faces.bottom`
2. `faces.all`
3. 否则报错

### 24.3 `north / south / east / west`

1. 精确键：`north/south/east/west`
2. `faces.sides`
3. `faces.all`
4. 否则报错

规则：

- `top/bottom` 不参与 `sides` 回退
- `sides` 不参与 `top/bottom` 回退
- 编译后的 texture MUST 始终能解析出六个物理面

---

## 25. 坐标约定

### 25.1 Block 局部坐标

在 `profile = voxel_cube_pbr` 中，规范化方块局部坐标为：

- `x ∈ [0,1]`：西到东（`-X -> +X`）
- `y ∈ [0,1]`：下到上（`-Y -> +Y`）
- `z ∈ [0,1]`：北到南（`-Z -> +Z`）

物理面方向：

- `top` = `+Y`
- `bottom` = `-Y`
- `north` = `-Z`
- `south` = `+Z`
- `east` = `+X`
- `west` = `-X`

### 25.2 图片坐标

- `u` 向右
- `v` 向下
- 范围均为 `[0,1]`

### 25.3 `block_space` 面映射公式

对 `domain = block_space`，每个物理面的 `(u, v)` MUST 映射为：

```text
top    : (x, y, z) = (u,     1,     v)
bottom : (x, y, z) = (u,     0, 1 - v)
north  : (x, y, z) = (u, 1 - v,     0)
south  : (x, y, z) = (1-u, 1 - v,   1)
east   : (x, y, z) = (1, 1 - v, 1-u)
west   : (x, y, z) = (0, 1 - v,   u)
```

这是本 profile 下**唯一允许**的 cube 面映射定义。

### 25.4 跨面连续性承诺

实现只有在以下条件同时成立时，才应承诺跨边连续：

1. 相邻两个面绑定到同一个 surface
2. 该 surface 的 `domain = block_space`
3. 该 binding 没有非法变换
4. 相关 signal / mask / field 全部使用同一 block-space 语义

若面绑定到不同 surface，即使参数一致，也不保证跨边连续。

---

## 26. 运行时逻辑输出

对每个 texture 的每个物理面，conforming runtime/exporter MUST 生成以下逻辑结果：

- `albedo`
- `normal`
- `height`
- `roughness`
- `ao`
- `metallic`
- `emissive`
- `opacity`

逻辑语义如下：

- `albedo`：基础反照率颜色，sRGB authored，线性混合
- `normal`：切线空间 normal
- `height`：`0..1` 相对高度，`0.5` 为中性平面
- `roughness`：`0..1`
- `ao`：`0..1`
- `metallic`：`0..1`
- `emissive`：线性发光颜色
- `opacity`：`0` 全透明，`1` 全不透明

最终 alpha 只由 `opacity` 提供：

```text
final_alpha = opacity
```

---

## 27. 物理存储与导出契约

core 规范定义的是**逻辑通道语义**，不强制某一种物理存储后端。  
实现 MAY 使用：

- `texture_2d_array`
- atlas
- 独立纹理
- 离线 PNG
- 离线 EXR
- KTX2 / BC 压缩纹理
- 上述方式的组合

但无论采用哪种方式，对外语义都 MUST 与本文一致。

### 27.1 推荐打包契约

推荐但非强制：

- `albedo`：单独纹理
- `normal`：单独纹理
- `orm`：`R=ao, G=roughness, B=metallic, A=opacity`
- `emissive`：单独纹理
- `height`：单独单通道纹理

### 27.2 色彩空间

实现 MUST 遵循：

- `albedo`：作者输入为 sRGB；混合与 mip 生成在线性空间
- `emissive`：作者输入为 sRGB；参与照明前转换到线性并乘以 `intensity`
- `normal / height / roughness / ao / metallic / opacity`：始终线性

### 27.3 Mipmap 规则

若 `output.mipmaps = full`，实现 MUST：

- 对 `albedo`：在线性空间下采样，再编码为目标格式
- 对 `emissive`：在线性空间下采样
- 对 `normal`：
  1. 先解码到 `[-1,1]`
  2. 在线性向量空间平均
  3. 重新归一化
  4. 再编码回纹理格式
- 对 `height / roughness / ao / metallic / opacity`：在线性空间逐通道下采样

### 27.4 Mask 透明度的 coverage 保持

若：

- `alpha_mode = mask`
- 且生成 mip

则实现 SHOULD 在 mip 过程中做 **coverage-preserving alpha adjustment**，以尽量保持相对于 `cutout_threshold` 的可见面积一致。  
若未实现该策略，至少必须记录该限制，并保证结果可预期。

### 27.5 Atlas 边缘处理

若存储后端使用 atlas，则 MUST 对每个 tile 做 padding / extrusion：

- 推荐：2–4 像素
- 对 tileable 内容，padding SHOULD 使用 wrap 采样生成

若后端使用 array，则无需 atlas padding。

### 27.6 推荐压缩格式

推荐但非强制：

- `albedo`：RGBA8 sRGB / BC7 sRGB
- `normal`：RG8 / RGBA8 / BC5
- `orm`：RGBA8 linear / BC7
- `emissive`：RGBA8 / BC7
- `height`：R16_UNORM / R16F

---

## 28. 编译、规范化与缓存指纹

### 28.1 编译阶段

compiler MUST 至少执行以下阶段：

1. **Parse**
2. **Strict validate**
3. **Default expansion**
4. **Shorthand expansion**
5. **Reference resolution**
6. **Signal DAG validation**
7. **Face resolution**
8. **Canonicalization**
9. **Fingerprint generation**

### 28.2 简写展开

compiler MUST 至少展开以下简写：

- face binding 字符串 -> `FaceBinding`
- palette 简写颜色数组 -> 显式 palette stops
- inline noise field -> 等价的规范化 field/signal 语义
- 省略的 default field -> 显式继承结果

### 28.3 Canonical Form

compiler MUST 生成 canonical form，并保证：

- 对象键顺序固定
- face binding 全部展开
- 所有默认值显式化
- 所有 palette 停靠点显式化
- `all/sides` 最终能解析到六个物理面
- canonical form 是缓存与增量构建的唯一输入

### 28.4 Fingerprint

compiler SHOULD 计算稳定缓存指纹，至少应覆盖：

- canonical form
- `spec`
- `spec_version`
- `profile`
- compiler 算法版本
- 目标导出/打包 profile（若存在）

规则：

- 任意语义变化 MUST 改变指纹
- 非语义字段如 `meta` 是否计入指纹，应由构建系统显式决定，而不是隐式漂移

---

## 29. 确定性与数值容差

### 29.1 同后端确定性

对同一 canonical 资产、同一 compiler 版本、同一 backend、同一平台：

- 结果 MUST bit-exact

### 29.2 跨后端容差

对不同 conforming backend，允许存在微小数值误差，但必须满足：

- 8 位颜色/标量通道：最大绝对误差 ≤ `1 / 255`
- 16 位 height：最大绝对误差 ≤ `1 / 65535`
- normal：解码并归一化后，角度误差 ≤ `0.5°`

### 29.3 抖动与噪声稳定性

- `bayer4` 阈值矩阵 MUST 固定
- warp 种子偏移规则 MUST 固定
- 同一 canonical 资产不得因对象键顺序、map 迭代顺序或 hash 随机化而改变输出

---

## 30. 限制与资源安全门槛

为保证实现稳定性与防止恶意/异常资产拉爆编译器，core 规范定义以下上限：

- 每个 texture set 的 `surfaces`：最多 4096
- 每个 texture set 的 `textures`：最多 4096
- 每个 surface 的 `signals`：最多 64
- 每个 surface 的 `layers`：最多 16
- mask 递归深度：最多 8
- palette stops：最多 64
- `output.size`：最多 1024
- `octaves`：最多 8
- `anisotropy`：最多 16

实现可以收紧这些上限，但 MUST 在文档或日志中明确说明。

---

## 31. 校验规则

以下规则 MUST 在加载/编译阶段完成，并采用 fail-fast 处理。

### 31.1 顶层校验

- `spec` 必须正确
- `spec_version` 必须正确
- `profile` 必须正确
- `output` 必填
- `surfaces` 非空
- `textures` 非空
- 不允许未知顶层字段

### 31.2 输出校验

- `output.size` 必须为 `16..1024` 且是 2 的幂
- `output.mipmaps` 必须合法
- `output.normal_format` 必须合法

### 31.3 名称校验

- 所有 surface / texture / signal 名称必须满足正则
- 同类名称在同一作用域内必须唯一
- texture 与 surface 可同名但 SHOULD NOT，这会降低可读性

### 31.4 Surface 校验

- `base.albedo` 必填
- `logical_size <= output.size`
- `pixel_snap = true` 时，`output.size % logical_size == 0`
- `layers` 数量合法
- `domain`、`tile_mode` 合法
- `seed` 合法

### 31.5 Signal 校验

- signal 图无环
- `source` / `inputs` 引用存在
- `combine` 的输入数量与 `op` 匹配
- `clamp`、`curve`、`remap` 合法

### 31.6 Field 校验

- `palette` 至少 2 个停靠点
- `mapping.levels = 0` 或 `2..64`
- `contrast > 0`
- `intensity >= 0`
- scalar `remap.range`、`clamp` 合法
- `albedo` 上禁止 `intensity != 1`
- `constant` / `signal` / `noise` 的互斥关系必须成立

### 31.7 Noise 校验

- `scale > 0`
- `stretch` 每项 `> 0`
- `octaves` 在 `1..8`
- `lacunarity > 1`
- `gain` 在 `0..1`
- `cellular` 仅允许 `fractal = none`
- `fractal = none` 时 `octaves == 1`
- `warp.amplitude >= 0`

### 31.8 Mask 校验

- `axis_band.to > axis_band.from`
- `falloff >= 0`
- `softness >= 0`
- `threshold` 在 `0..1`
- 组合 mask 的输入长度合法
- `edge_distance` 只能在 face UV 语义上求值

### 31.9 Texture 校验

- `faces` 必须能解析出六个物理面
- 引用的 surface 必须存在
- `rotate` 只允许 `0/90/180/270`
- `block_space` surface 不允许 face 旋转/翻转
- `anisotropy` 在 `1..16`
- `cutout_threshold` 在 `0..1`

---

## 32. 失败语义与运行时状态机

runtime generator MUST 暴露至少以下状态：

- `Loading`
- `Ready`
- `Failed(reason)`

规则：

1. 任意 parse / validate / compile / generate / upload 错误，都 MUST 进入 `Failed(reason)`。
2. MUST NOT 因错误而永远停留在 `Loading`。
3. MUST NOT 进入“部分 Ready、部分 Failed”的残缺状态。
4. registry 只有在整套 texture system 完整成功后才可见。

推荐错误格式：

```text
assets/texture_data/blocks.texture.json:textures.grass_block.faces.sides.surface: unknown surface "grass_sidee"
```

错误消息 SHOULD 至少包含：

- 文件名
- JSON 路径
- 人类可读原因

---

## 33. Registry 契约

实现 MUST 提供一个与声明顺序无关的 registry。  
推荐逻辑形态：

```text
TextureRegistry
  texture_name -> ResolvedTexture

ResolvedTexture
  top    -> ResolvedFace
  bottom -> ResolvedFace
  north  -> ResolvedFace
  south  -> ResolvedFace
  east   -> ResolvedFace
  west   -> ResolvedFace
  sampler
  alpha_mode
  cutout_threshold

ResolvedFace
  albedo_handle
  normal_handle
  orm_handle or individual scalar handles
  emissive_handle
  height_handle
```

必须满足：

1. 通过 texture 名称查找，不依赖 JSON 声明顺序。
2. 任何缺失 texture/surface/face/channel 都必须在加载期报错。
3. 不允许 name -> layer_index 这种失去面语义和通道语义的过窄 API 作为唯一公开契约。
4. runtime 可以内部使用 atlas/array/layer index，但外部逻辑 API 必须是面与通道语义。

---

## 34. CI 与质量门槛

生产环境至少应包含以下测试：

1. **Schema lint**
   - 所有 `*.texture.json` 必须通过严格解析与字段校验。

2. **Canonicalization test**
   - 等价输入的 canonical form 必须一致。

3. **Fingerprint stability test**
   - 非语义对象键顺序变化不得改变指纹。

4. **Registry test**
   - 验证 `all / sides / exact face` precedence、字符串简写、旋转/翻转与 surface 引用。

5. **Golden image test**
   - 对关键 surface 生成快照，防止算法漂移。

6. **Seam test**
   - 对 `block_space` surface 验证共享边缘连续性。

7. **Color-space test**
   - 验证 albedo mip 在线性空间处理，不出现明显偏暗/偏灰。

8. **Normal mip test**
   - 验证 normal 下采样后仍被正确归一化。

9. **Failure-path test**
   - 非法资产必须进入 `Failed(reason)`，而不是卡在 `Loading`。

---

## 35. 完整示例

下面是一份完整、单文件、可直接落地实现的示例。  
它展示了：

- surface 复用
- signal 图复用
- `top/bottom/sides` 绑定
- `axis_band` 草皮覆盖
- `threshold` 矿点遮罩
- palette 映射
- signal 驱动的 height/roughness
- 从最终 height 推导 normal

```json
{
  "spec": "cruft.procedural_texture",
  "spec_version": "1.0.0",
  "profile": "voxel_cube_pbr",
  "meta": {
    "author": "cruft",
    "license": "MIT",
    "description": "Example voxel cube texture set"
  },
  "output": {
    "size": 64,
    "mipmaps": "full",
    "normal_format": "opengl"
  },
  "defaults": {
    "surface": {
      "logical_size": 16,
      "pixel_snap": true,
      "domain": "face_uv",
      "tile_mode": "repeat",
      "seed": 0,
      "normal": {
        "mode": "flat",
        "strength": 1.0
      }
    },
    "texture": {
      "sampler": {
        "mag_filter": "nearest",
        "min_filter": "nearest",
        "mipmap_filter": "nearest",
        "anisotropy": 1,
        "address_u": "clamp_to_edge",
        "address_v": "clamp_to_edge"
      },
      "alpha_mode": "opaque",
      "cutout_threshold": 0.5
    }
  },
  "surfaces": {
    "dirt": {
      "seed": 1101,
      "signals": {
        "base_noise": {
          "kind": "noise",
          "noise": {
            "basis": "value",
            "fractal": "fbm",
            "scale": 3.0,
            "stretch": [1.0, 1.0, 1.0],
            "octaves": 3,
            "lacunarity": 2.0,
            "gain": 0.5,
            "offset": [0.0, 0.0, 0.0],
            "seed_offset": 0,
            "warp": {
              "amplitude": 0.08,
              "basis": "gradient",
              "fractal": "fbm",
              "scale_multiplier": 1.0,
              "octaves": 2,
              "lacunarity": 2.0,
              "gain": 0.5,
              "seed_offset": 4096
            }
          }
        }
      },
      "base": {
        "albedo": {
          "mode": "palette",
          "source": "base_noise",
          "palette": ["#4A321F", "#5C3D26", "#6E4B30", "#81583A"],
          "mapping": {
            "mode": "quantized",
            "levels": 0,
            "contrast": 1.0,
            "bias": 0.0,
            "invert": false,
            "dither": "none"
          }
        },
        "height": {
          "mode": "signal",
          "source": "base_noise",
          "remap": {
            "range": [0.42, 0.58],
            "contrast": 1.0,
            "bias": 0.0,
            "invert": false,
            "curve": {
              "interp": "linear",
              "stops": [
                [0.0, 0.0],
                [1.0, 1.0]
              ]
            },
            "clamp": [0.0, 1.0]
          }
        },
        "roughness": {
          "mode": "constant",
          "value": 0.95
        },
        "ao": {
          "mode": "constant",
          "value": 1.0
        },
        "metallic": {
          "mode": "constant",
          "value": 0.0
        },
        "opacity": {
          "mode": "constant",
          "value": 1.0
        }
      },
      "normal": {
        "mode": "derive_from_height",
        "strength": 1.0
      }
    },
    "grass_top": {
      "seed": 9001,
      "signals": {
        "base_noise": {
          "kind": "noise",
          "noise": {
            "basis": "value",
            "fractal": "fbm",
            "scale": 2.8,
            "stretch": [1.0, 1.0, 1.0],
            "octaves": 3,
            "lacunarity": 2.0,
            "gain": 0.5,
            "offset": [0.0, 0.0, 0.0],
            "seed_offset": 0,
            "warp": {
              "amplitude": 0.10,
              "basis": "gradient",
              "fractal": "fbm",
              "scale_multiplier": 1.0,
              "octaves": 2,
              "lacunarity": 2.0,
              "gain": 0.5,
              "seed_offset": 4096
            }
          }
        }
      },
      "base": {
        "albedo": {
          "mode": "palette",
          "source": "base_noise",
          "palette": ["#305E24", "#3E742C", "#558C3A", "#78AA4E"],
          "mapping": {
            "mode": "quantized",
            "levels": 0,
            "contrast": 1.05,
            "bias": 0.0,
            "invert": false,
            "dither": "none"
          }
        },
        "height": {
          "mode": "signal",
          "source": "base_noise",
          "remap": {
            "range": [0.45, 0.64],
            "contrast": 1.0,
            "bias": 0.0,
            "invert": false,
            "curve": {
              "interp": "linear",
              "stops": [
                [0.0, 0.0],
                [1.0, 1.0]
              ]
            },
            "clamp": [0.0, 1.0]
          }
        },
        "roughness": {
          "mode": "constant",
          "value": 0.92
        },
        "ao": {
          "mode": "constant",
          "value": 1.0
        },
        "metallic": {
          "mode": "constant",
          "value": 0.0
        },
        "opacity": {
          "mode": "constant",
          "value": 1.0
        }
      },
      "normal": {
        "mode": "derive_from_height",
        "strength": 1.2
      }
    },
    "grass_side": {
      "seed": 9101,
      "signals": {
        "dirt_noise": {
          "kind": "noise",
          "noise": {
            "basis": "value",
            "fractal": "fbm",
            "scale": 3.0,
            "stretch": [1.0, 1.0, 1.0],
            "octaves": 3,
            "lacunarity": 2.0,
            "gain": 0.5,
            "offset": [0.0, 0.0, 0.0],
            "seed_offset": 0,
            "warp": {
              "amplitude": 0.08,
              "basis": "gradient",
              "fractal": "fbm",
              "scale_multiplier": 1.0,
              "octaves": 2,
              "lacunarity": 2.0,
              "gain": 0.5,
              "seed_offset": 4096
            }
          }
        },
        "grass_noise": {
          "kind": "noise",
          "noise": {
            "basis": "value",
            "fractal": "fbm",
            "scale": 2.8,
            "stretch": [1.0, 1.0, 1.0],
            "octaves": 3,
            "lacunarity": 2.0,
            "gain": 0.5,
            "offset": [0.0, 0.0, 0.0],
            "seed_offset": 23,
            "warp": {
              "amplitude": 0.10,
              "basis": "gradient",
              "fractal": "fbm",
              "scale_multiplier": 1.0,
              "octaves": 2,
              "lacunarity": 2.0,
              "gain": 0.5,
              "seed_offset": 4120
            }
          }
        },
        "cap_band_jitter": {
          "kind": "noise",
          "noise": {
            "basis": "value",
            "fractal": "fbm",
            "scale": 4.0,
            "stretch": [1.0, 1.0, 1.0],
            "octaves": 2,
            "lacunarity": 2.0,
            "gain": 0.5,
            "offset": [0.0, 0.0, 0.0],
            "seed_offset": 97
          }
        }
      },
      "base": {
        "albedo": {
          "mode": "palette",
          "source": "dirt_noise",
          "palette": ["#4A321F", "#5C3D26", "#6E4B30", "#81583A"]
        },
        "height": {
          "mode": "signal",
          "source": "dirt_noise",
          "remap": {
            "range": [0.42, 0.58],
            "contrast": 1.0,
            "bias": 0.0,
            "invert": false,
            "curve": {
              "interp": "linear",
              "stops": [
                [0.0, 0.0],
                [1.0, 1.0]
              ]
            },
            "clamp": [0.0, 1.0]
          }
        },
        "roughness": {
          "mode": "constant",
          "value": 0.95
        },
        "ao": {
          "mode": "constant",
          "value": 1.0
        },
        "metallic": {
          "mode": "constant",
          "value": 0.0
        },
        "opacity": {
          "mode": "constant",
          "value": 1.0
        }
      },
      "layers": [
        {
          "name": "grass_cap",
          "mask": {
            "mode": "axis_band",
            "axis": "v",
            "from": 0.0,
            "to": 0.30,
            "falloff": 0.05,
            "invert": false,
            "jitter": {
              "amount": 0.03,
              "source": "cap_band_jitter"
            }
          },
          "strength": 1.0,
          "albedo": {
            "mode": "palette",
            "source": "grass_noise",
            "palette": ["#305E24", "#3E742C", "#558C3A", "#78AA4E"],
            "mapping": {
              "mode": "quantized",
              "levels": 0,
              "contrast": 1.05,
              "bias": 0.0,
              "invert": false,
              "dither": "none"
            }
          },
          "height": {
            "mode": "signal",
            "source": "grass_noise",
            "remap": {
              "range": [0.50, 0.66],
              "contrast": 1.0,
              "bias": 0.0,
              "invert": false,
              "curve": {
                "interp": "linear",
                "stops": [
                  [0.0, 0.0],
                  [1.0, 1.0]
                ]
              },
              "clamp": [0.0, 1.0]
            }
          },
          "blend": {
            "albedo": "mix",
            "height": "max"
          }
        }
      ],
      "normal": {
        "mode": "derive_from_height",
        "strength": 1.0
      }
    },
    "iron_ore_surface": {
      "seed": 3001,
      "signals": {
        "stone_noise": {
          "kind": "noise",
          "noise": {
            "basis": "gradient",
            "fractal": "ridged",
            "scale": 3.2,
            "stretch": [1.0, 1.0, 1.0],
            "octaves": 4,
            "lacunarity": 2.0,
            "gain": 0.5,
            "offset": [0.0, 0.0, 0.0],
            "seed_offset": 0,
            "warp": {
              "amplitude": 0.06,
              "basis": "gradient",
              "fractal": "fbm",
              "scale_multiplier": 1.0,
              "octaves": 2,
              "lacunarity": 2.0,
              "gain": 0.5,
              "seed_offset": 4096
            }
          }
        },
        "ore_cells": {
          "kind": "noise",
          "noise": {
            "basis": "cellular",
            "fractal": "none",
            "cellular_return": "f1",
            "scale": 5.5,
            "stretch": [1.0, 1.0, 1.0],
            "octaves": 1,
            "lacunarity": 2.0,
            "gain": 0.5,
            "offset": [0.0, 0.0, 0.0],
            "seed_offset": 9
          }
        },
        "ore_mask": {
          "kind": "mask",
          "mask": {
            "mode": "threshold",
            "source": "ore_cells",
            "threshold": 0.72,
            "softness": 0.08,
            "invert": false
          }
        }
      },
      "base": {
        "albedo": {
          "mode": "palette",
          "source": "stone_noise",
          "palette": ["#61656C", "#737A84", "#878F9B", "#A0A7B3"],
          "mapping": {
            "mode": "quantized",
            "levels": 0,
            "contrast": 1.0,
            "bias": 0.0,
            "invert": false,
            "dither": "none"
          }
        },
        "height": {
          "mode": "signal",
          "source": "stone_noise",
          "remap": {
            "range": [0.40, 0.60],
            "contrast": 1.0,
            "bias": 0.0,
            "invert": false,
            "curve": {
              "interp": "linear",
              "stops": [
                [0.0, 0.0],
                [1.0, 1.0]
              ]
            },
            "clamp": [0.0, 1.0]
          }
        },
        "roughness": {
          "mode": "constant",
          "value": 0.88
        },
        "ao": {
          "mode": "constant",
          "value": 1.0
        },
        "metallic": {
          "mode": "constant",
          "value": 0.0
        },
        "opacity": {
          "mode": "constant",
          "value": 1.0
        }
      },
      "layers": [
        {
          "name": "ore_deposits",
          "mask": {
            "mode": "signal",
            "source": "ore_mask"
          },
          "strength": 1.0,
          "albedo": {
            "mode": "palette",
            "noise": {
              "basis": "value",
              "fractal": "fbm",
              "scale": 6.0,
              "stretch": [1.0, 1.0, 1.0],
              "octaves": 2,
              "lacunarity": 2.0,
              "gain": 0.5,
              "offset": [0.0, 0.0, 0.0],
              "seed_offset": 19
            },
            "palette": ["#905A2B", "#B76C33", "#D98B46", "#F0B166"],
            "mapping": {
              "mode": "quantized",
              "levels": 0,
              "contrast": 1.1,
              "bias": 0.0,
              "invert": false,
              "dither": "none"
            }
          },
          "height": {
            "mode": "noise",
            "noise": {
              "basis": "value",
              "fractal": "none",
              "scale": 6.0,
              "stretch": [1.0, 1.0, 1.0],
              "octaves": 1,
              "lacunarity": 2.0,
              "gain": 0.5,
              "offset": [0.0, 0.0, 0.0],
              "seed_offset": 21
            },
            "remap": {
              "range": [0.56, 0.76],
              "contrast": 1.0,
              "bias": 0.0,
              "invert": false,
              "curve": {
                "interp": "linear",
                "stops": [
                  [0.0, 0.0],
                  [1.0, 1.0]
                ]
              },
              "clamp": [0.0, 1.0]
            }
          },
          "roughness": {
            "mode": "constant",
            "value": 0.60
          },
          "blend": {
            "albedo": "mix",
            "height": "max",
            "roughness": "mix"
          }
        }
      ],
      "normal": {
        "mode": "derive_from_height",
        "strength": 1.4
      }
    }
  },
  "textures": {
    "grass_block": {
      "faces": {
        "top": "grass_top",
        "bottom": "dirt",
        "sides": "grass_side"
      }
    },
    "iron_ore": {
      "faces": {
        "all": "iron_ore_surface"
      }
    }
  },
  "extensions_used": [],
  "extensions": {}
}
```

---

## 36. 明确的实现原则

1. **core 规范禁止模糊风格 fallback。**
   - 不允许 `style = xxx` 写错了却自动退回另一个风格。
   - 任何未知枚举值都必须直接失败。

2. **core 规范要求面语义与通道语义一等公民。**
   - 不允许仅暴露 `name -> layer_index` 作为唯一逻辑 API。

3. **core 规范要求失败可感知。**
   - 非法资产不得永远停在 `Loading`。
   - 必须进入 `Failed(reason)`。

4. **core 规范要求 canonicalization。**
   - 只有 canonical form 才能作为缓存、热重载、增量构建与导出的一致依据。

5. **core 规范要求颜色空间正确。**
   - 颜色 authored in sRGB，混合与 mip 在线性空间。
   - normal 与标量通道始终线性。

6. **core 规范要求 determinism。**
   - 同一 canonical 资产必须在可控容差内得到一致结果。

---

## 37. 推荐落地顺序

若按本文实现代码，最稳妥的工程顺序是：

1. 严格 parser 与顶层 schema 校验
2. defaults / shorthand / canonicalization
3. signal DAG 与 face 解析
4. base + layers 合成
5. normal derive
6. registry 与 `Loading / Ready / Failed`
7. 物理打包、mip、导出、缓存指纹

按这个顺序最容易得到一套真正不漂移、可长期维护、可 CI 化、可生产部署的程序化纹理系统。
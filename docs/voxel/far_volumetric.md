# Cruft 远景雾化地平线（Occupancy Clipmap + Raymarch）

本文档定义 Cruft 的远景方案：远处不渲染细节几何，只提供雾化地平线的遮蔽轮廓与体积雾感。该方案采用固定规格的 occupancy clipmap，并以固定步数的 raymarch 输出遮蔽与雾透过率。

---

## 1. 强约束（写死）

- clipmap 必须来自 CPU 侧占据缓存；禁止 GPU 反读。
- clipmap 只表达“是否占据”（occupancy），不表达材质与细节。
- raymarch 只输出遮蔽与雾透过率，不输出可交互几何。

---

## 2. 数据表示：Occupancy Clipmap（写死）

### 2.1 Clipmap 级别

3 级（从近到远）：

- L0：细（最小 cell size）
- L1：中（2× cell size）
- L2：粗（4× cell size）

每级覆盖一个固定尺寸的 3D 体积窗口（围绕相机/参考点），并随相机滚动。

### 2.2 存储格式（写死）

occupancy 使用 1 byte：

- `R8Uint` 3D texture 或 2D array（实现选其一，但对外语义相同）
- cell 值：
  - `0`：空
  - `255`：有占据（solid）

分辨率（写死）：

- 每级：`128 × 64 × 128`（X,Y,Z）
  - Y 维度较小：因为“雾化地平线”主要关心中低高度轮廓
 
cell 尺寸（写死）：

- `BASE_CELL_SIZE = 4`（单位：world voxels）
- `cell_size[L] = BASE_CELL_SIZE * 2^L`

### 2.3 世界→clipmap 映射（写死）

对每级 L：

- clipmap 的世界对齐原点 `origin[L]` 必须对齐到 `cell_size[L]` 网格。
- 当相机在世界空间移动跨越一个 `cell_size[L]` 的边界时，只更新新进入窗口的 slab。

---

## 3. Clipmap 更新：CPU 侧占据缓存（写死）

### 3.1 数据来源（写死）

clipmap 的 occupancy 来自 CPU 侧的世界存储：

- 通过 `World Storage` 提供的只读查询：
  - “某个世界空间 AABB 内是否存在 solid voxel”

为了避免每次更新做大量 voxel 扫描，必须维护 chunk 内的粗粒度占据缓存：

- `OCC_BRICK_SIZE = 8`
- 每个 chunk 维护 `4×4×4` 个 occupancy brick（每 brick 覆盖 `8×8×8` voxels）

更新 clipmap cell 时：

- 先用 coarse occupancy 快速判空
- coarse 判定为占据则将该 cell 置为占据

### 3.2 更新触发与一致性

触发条件：

- 相机滚动导致的 slab 更新
- 世界编辑影响到 clipmap 覆盖区域（可按“编辑事件”低频合并后刷新）

一致性要求：

- clipmap 更新允许最多延迟 1 帧。
- 每级 clipmap 必须使用双缓冲，提交时一次性切换。

---

## 4. Raymarch 合成（极低预算）

### 4.1 输出语义

远景 pass 输出两项即可：

- `horizon_occlusion`：0..1（轮廓遮蔽强度）
- `fog_transmittance`：0..1（雾透过率）

最终与近景合成（例如：远景先做雾，再叠加轮廓暗化）。

### 4.2 采样策略（写死）

- 最大步数：`MAX_STEPS = 24`
- 级别切换（写死）：
  - 距离 `< 256`：使用 L0
  - 距离 `< 512`：使用 L1
  - 距离 `>= 512`：使用 L2
- 步长：每步前进一个 cell（按当前 L 的 `cell_size[L]`），并在命中后固定输出遮蔽。

### 4.3 命中与雾累积（写死公式）

雾累积采用指数衰减（示例）：

```
transmittance *= exp(-density * step_length)
```

轮廓遮蔽：

- 当 ray 首次命中 occupancy 时，输出一个遮蔽因子（可与深度/距离相关）
- 遮蔽不追求精确碰撞点，只要轮廓稳定即可

---

## 5. 与近景网格的交界（写死）

- 远景雾/遮蔽从距离阈值 `FAR_START = 512` 开始叠加到最终画面。
- 远景雾/遮蔽在距离 `FAR_END = 640` 达到全强度。
- 交界过渡使用 `smoothstep(FAR_START, FAR_END, distance)` 进行渐变。

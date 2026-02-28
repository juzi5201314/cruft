# Cruft 体素引擎方案总览（单一路线 / 最高性能）

本文档定义 Cruft 的体素（voxel）引擎**唯一且确定**的主方案：近处体素使用 **整 chunk 的 Binary Greedy Meshing（bitmask）** 生成面片，渲染端使用 **Packed-Quad + Vertex Pulling + Multi-Draw Indirect**，并启用 **HZB Occlusion Culling**；远处使用固定规格的 **occupancy clipmap 体积雾化地平线**。

约束：本文档只描述唯一主方案；这里写到的内容都属于必须实现的范围。

---

## 1. 目标

- **最高性能**：将 meshing 与渲染两端的常数项压到最低，并保证可大规模并行。
- **高频编辑稳定**：编辑导致的重建成本可预测，不出现“某些编辑极慢”的最坏情况。
- **低内存 / 低显存**：chunk 数据压缩常驻；渲染侧以紧凑 quad 数据驱动，减少顶点带宽。

---

## 2. 输入约束（写死）

- 表面：**方块面**（Minecraft 风格）。
- 编辑频率：**高频**（持续挖放 + 可能大范围修改）。
- 世界维度：**无限高度**（真正 3D 稀疏流式，不是列式 32×32×256）。
- 平台目标：**PC 核显/低配优先**。
- 渲染：
  - 近处：网格光栅化（整 chunk meshing 输出 quads）
  - 远处：雾化地平线体积/遮蔽（occupancy clipmap + raymarch）
- chunk 尺寸：**32×32×32**。
- 方块状态：需要 **metadata**；复杂对象（容器/机器等）不强塞进每 voxel 常驻结构。
- 光照：不实现体素 GI（渲染按常规光栅化 + 雾合成）。

---

## 3. 术语与坐标约定（强约束）

### 术语

- **Voxel**：单位体素格。
- **Chunk**：固定尺寸的体素块，为 `32×32×32`。
- **BlockKind**：方块“种类”（草、石、木头…）。
- **BlockStateId**：可渲染/可碰撞的方块静态状态（含少量 metadata，如朝向/变体）。
- **BlockEntity**：复杂/稀疏/高频变化的对象状态（库存、机器、作物等），与 voxel 存储分离。

### 坐标

- 世界 voxel 坐标：`(wx, wy, wz)`，整数格点。
- chunk 坐标：`(cx, cy, cz)`，每轴 `cx = floor_div(wx, 32)`。
- chunk 内局部坐标：`(lx, ly, lz)`，范围 `[0, 31]`。

> 线性索引顺序（存储与网格必须一致）在 `storage.md` / `storage_blocks.md` 明确写死。

---

## 4. 系统分层与数据流

### 模块边界（分卷对应）

1) **World Storage**（`storage.md` / `storage_blocks.md` / `storage_layers.md` / `persistence.md`）  
负责：chunk 流式、blocks 热路径存储、分层数据组织、持久化 IO、变更通知（dirty chunk 标记）。

2) **Meshing**（`meshing.md`）  
负责：从 chunk 数据生成网格；跨 chunk 邻居采样；任务并行与版本一致性。

3) **Rendering**（`rendering.md`）  
负责：quad buffer、vertex pulling、MDI、HZB 遮挡剔除、材质与纹理数组绑定。

4) **Far Volumetric Horizon**（`far_volumetric.md`）  
负责：远处雾化地平线；低分辨率 clipmap 的维护；极低成本 raymarch 合成。

5) **Lighting**（`lighting.md`）  
负责：近景光栅化光照与远景雾化合成的固定规则。

### 主数据流（运行时）

```
[玩家/系统编辑 voxel]
        |
        v
[World Storage 写入 + 标记 dirty chunk]
        |
        v
[Meshing 异步任务队列：重建 chunk mesh（Binary Greedy）]
        |
        v
[主线程合并结果：替换/更新 chunk quad buffer]
        |
        +--------------------+
        |                    |
        v                    v
[近处网格光栅化]      [远景 clipmap 增量更新]
                             |
                             v
                    [远景雾/遮蔽 fullscreen 合成]
```

---

## 5. 参数（写死）

- `CHUNK_SIZE = 32`
- 渲染层（render layers）：
  - `Opaque`：完全不透明，参与遮挡与合并
  - `Cutout`：alpha test（如树叶），不与 `Opaque` 合并
  - `Transparent`：真正半透明
- 存储策略（Storage v2）：
  - chunk 的权威热表示为 `Chunk(32³) → Brick(8³)`（每 chunk 64 个 brick）（见 `storage.md` / `storage_blocks.md`）
  - brick 的热路径采用 `Single | Paletted(u16 indices) | Direct(u32 values)` 三态（bit-pack 只允许存在于落盘/冷存）
- 高频编辑：mesh 重建粒度为 **整 chunk**（Binary Greedy 的常数项足够低，以吞吐换稳定性）。

---

## 6. 粗略预算（便于后续对齐）

> 下列数字用于“量级对齐”，不作为最终性能承诺。

- 一个 chunk voxel 数：`32^3 = 32768`
- blocks 热路径内存量级（仅用于对齐）：
  - 每 brick：`Paletted.indices = 512 * 2 = 1024B`；`Direct.values = 512 * 4 = 2048B`
  - 每 chunk 最多 64 个 brick：上限约 `64KB`（全 Paletted indices）到 `128KB`（全 Direct values），大量 `Single(air)` 时远低于此
- chunk mesh 的典型开销取决于地形噪声与方块类型；本方案用二值占据 + face masks + greedy 合并降低常数项。
- 远景体积 clipmap（3 级、R8 occupancy）：
  - 例如每级 `128×64×128`：约 1MB/级，合计约 3MB（不含 mip）

---

## 7. 风险约束（写死）

- **透明渲染**：`Transparent` 必须单独渲染队列并排序；`Cutout` 必须使用 alpha test。
- **无限高度流式**：meshing 与渲染必须接受 chunk 生命周期变化（加载/卸载）且不持有悬空引用。
- **远景 clipmap 数据源**：clipmap 的 occupancy 必须来自 CPU 侧的粗粒度占据缓存；禁止 GPU 反读。

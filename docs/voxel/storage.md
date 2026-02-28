# Cruft 体素存储（Storage v2：Chunk / Brick / Hot+Cold / Layers）

本文档定义 Cruft 的 voxel 世界存储**唯一且确定**的主方案（Storage v2）：

- 寻址与流式单位：`Chunk = 32×32×32`
- 热路径（运行时权威表示）：`Brick = 8×8×8`（每 chunk 64 个 brick）
- 冷存/落盘表示：brick 级的 `palette + bit-pack`，再做段级压缩
- 复杂状态（BlockEntity/Overlay/Entities/WorldState）与 blocks 分层，禁止混入 blocks 热路径

约束：Storage v2 是硬切换；不做任何向后兼容与双写。

---

## 1. 常量（写死）

- `CHUNK_SIZE = 32`
- `BRICK_SIZE = 8`
- `BRICKS_PER_CHUNK_AXIS = 4`
- `BRICKS_PER_CHUNK = 64`
- chunk 体素数：`32^3 = 32768`
- brick 体素数：`8^3 = 512`

---

## 2. 坐标与索引（写死）

### 2.1 坐标转换

给定世界 voxel 坐标 `(wx, wy, wz)`：

- chunk 坐标：
  - `cx = floor_div(wx, 32)`
  - `cy = floor_div(wy, 32)`
  - `cz = floor_div(wz, 32)`
- chunk 内局部：
  - `lx = wx - cx*32`（范围 `[0, 31]`）
  - `ly = wy - cy*32`
  - `lz = wz - cz*32`

约束：`floor_div` 必须对负数正确（向下取整），否则无限世界会在 0 轴附近出现断层 bug。

### 2.2 线性索引顺序（块存储与网格必须一致）

chunk 内 voxel 的线性索引顺序写死为：

```
index(lx, ly, lz) = lx + lz*32 + ly*32*32
```

即：`x` 最快、其次 `z`、最后 `y`。

brick 内 voxel 的线性索引顺序同样写死为：

```
sub_index(sx, sy, sz) = sx + sz*8 + sy*8*8
```

其中 `(sx, sy, sz)` 是 brick 内局部坐标，范围 `0..7`。

---

## 3. Blocks 数据模型（写死）

### 3.1 BlockStateKey 与动态注册表

Storage v2 使用两级标识：

- `BlockStateKey`：稳定键（例如 `namespace:name` + 排序后的属性表），用于持久化与注册表一致性
- `BlockStateId`：运行时 dense 数字 ID，用于所有热路径（meshing/碰撞/采样/编辑）

约束：

- `BlockStateId` 在 v2 中写死为 `u32`。
- 世界文件必须携带 `StateRegistry`（见 `persistence.md`），用于 `BlockStateKey ↔ BlockStateId` 的一致重建。
- 若某世界需要超过 `u32` 容量：必须通过世界格式版本变更硬切换解决（v2 不提供兼容退路）。

### 3.2 复杂状态严禁进入 blocks

下列数据严禁进入 blocks 的常驻结构（否则热路径成本失控）：

- 容器库存、机器能量/配方、脚本状态等复杂对象（BlockEntity）
- ECS 实体与世界全局状态（Entities/WorldState）

这些内容的存储边界在 `storage_layers.md` 与 `persistence.md` 中写死。

---

## 4. 热路径（运行时权威表示）：Chunk → Brick

一个 chunk 的 blocks 由 64 个 brick 组成（`4×4×4`）。

每个 brick 必须实现三态（并允许在运行时互转）：

1) `Single(value: BlockStateId)`
   - brick 全部 512 voxel 同一值（典型：空气）
   - 读写最省分支与内存

2) `Paletted { palette: Vec<BlockStateId>, indices: [u16; 512], reverse_map }`
   - `indices` 固定 `u16`（热路径性能优先，避免 bit-pack 的位运算常数）
   - `reverse_map` 用于 `BlockStateId -> palette_index(u16)` 的 O(1) 写入加速（实现可用哈希/开地址等）

3) `Direct { values: [BlockStateId; 512] }`
   - 直接存值，读写最快但内存更高
   - 当 palette 退化（palette_len 过大）时硬切换

brick 的阈值写死为：

- 当 `palette_len > 384`：从 `Paletted` 硬切到 `Direct`
- `Direct` 不在热路径自动降级为 `Paletted/Single`；降级只允许发生在 chunk 卸载/后台压缩任务中（避免写入时扫 512 项）

brick 的详细布局与读写契约见 `storage_blocks.md`。

---

## 5. 冷存/落盘表示（必须实现，但不进入热路径）

blocks 的冷存目标是“更小 + 可压缩 + 可流式 IO”，因此落盘时必须使用 brick 级编码：

- `Single`：仅写入一个 `BlockStateId`
- `PalettedPacked`：写入 palette + bit-packed indices（`bits = ceil(log2(palette_len))`，最多 9 bits）
- `DirectRaw`：写入 512 个 `BlockStateId`（仅当它比 `PalettedPacked` 更小/更快，或作为实现兜底）

chunk record 的分段压缩与校验在 `persistence.md` 写死。

---

## 6. Dirty 与一致性（与 meshing 的契约）

dirty 粒度写死为 chunk（与 `meshing.md` 一致）：

- 任意 voxel 写入导致所属 chunk dirty
- 写入发生在 chunk 边界时，相邻 chunk 同步标记 dirty

同时，存储层必须维护 brick 级粗粒度占据缓存（与 `far_volumetric.md` 一致）：

- 每 chunk 维护 `4×4×4` 的 occupancy brick（每个覆盖 `8×8×8` voxel）
- 对外提供只读查询：“某个 AABB 是否可能包含 solid voxel”

---

## 7. 分层存储（Blocks + 复杂状态 + 实体 + 世界状态）

Storage v2 将世界数据强制拆为独立层，且每层都必须可独立加载/卸载/压缩/版本化：

- Blocks（本文件 + `storage_blocks.md`）
- BlockEntities / Overlay layers / Entities / WorldState（见 `storage_layers.md`）
- 持久化格式与 IO（见 `persistence.md`）

---

## 8. 不变量（写死）

- blocks 热路径只处理 `BlockStateId`（纯值、可比较、可拷贝）。
- blocks 的写入必须只产生“局部变化”，禁止隐式地触发全局重编码或跨 chunk 重排。
- Storage v2 的热表示与落盘表示必须解耦；禁止把 bit-pack 带回热路径。

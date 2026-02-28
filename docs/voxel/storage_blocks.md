# Storage v2：Blocks（Chunk → Brick）规范

本文档定义 Storage v2 的 **Blocks 层**：其目标是作为运行时权威数据结构，提供最高吞吐的随机读写，并为 `meshing.md` 提供可批量构造的 `PaddedChunk` 输入。

约束：本文件是规范级；实现必须逐条满足。

---

## 1. 术语（写死）

- `Chunk`：`32×32×32` voxel 的寻址/流式单位
- `Brick`：`8×8×8` voxel 的存储原子（每 chunk 64 个）
- `BlockStateId`：热路径 ID（写死为 `u32`），用于所有 blocks 采样/比较
- `BlockStateKey`：稳定键（用于持久化），不进入热路径

---

## 2. 坐标分解（写死）

给定 chunk 内局部 voxel 坐标 `(lx, ly, lz)`，范围 `0..31`：

- brick 坐标：
  - `bx = lx >> 3`（范围 `0..3`）
  - `by = ly >> 3`
  - `bz = lz >> 3`
- brick 内局部：
  - `sx = lx & 7`（范围 `0..7`）
  - `sy = ly & 7`
  - `sz = lz & 7`

brick 的线性索引顺序写死为（与 `storage.md` 的 `x/z/y` 约定一致）：

```
brick_index(bx, by, bz) = bx + bz*4 + by*16
```

brick 内 voxel 的线性索引顺序写死为：

```
sub_index(sx, sy, sz) = sx + sz*8 + sy*8*8
```

因此 chunk 内 voxel 的读取路径必须等价于：

1) 计算 `brick_index(bx,by,bz)` 找到 brick
2) 计算 `sub_index(sx,sy,sz)` 在该 brick 内取值

---

## 3. Brick 三态（热路径权威表示）

每个 brick 必须实现下列三态之一：

### 3.1 `Single(value: BlockStateId)`

- 表示该 brick 的 512 个 voxel 全为同一 `value`
- 典型用途：空气、完全填充的大体积区域

### 3.2 `Paletted { palette, indices, reverse_map }`

- `palette: Vec<BlockStateId>`
  - `palette_len` 范围写死为 `1..=512`
  - `palette` 的顺序对热路径无语义要求，但落盘编码必须按当前顺序写入
- `indices: [u16; 512]`
  - `indices[i]` 必须满足 `indices[i] < palette_len`
  - `indices` 的索引 `i` 使用 `sub_index` 顺序（见第 2 节）
- `reverse_map`
  - 必须提供 `BlockStateId -> u16` 的 O(1) 查询能力（禁止对 palette 做线性扫描）
  - 必须与 `palette` 保持一致；若发生不一致属于数据损坏

### 3.3 `Direct { values: [BlockStateId; 512] }`

- 直接存值，读写最快，内存更高
- `values[i]` 的索引 `i` 使用 `sub_index` 顺序（见第 2 节）

---

## 4. 状态互转与阈值（写死）

### 4.1 Paletted → Direct 阈值

为保证最坏情况写入吞吐稳定，阈值写死为：

- 当 `palette_len > 384` 时，该 brick 必须硬切换为 `Direct`

说明：

- `palette_len` 的理论上限是 512，但当接近该上限时，`Paletted` 的写入（维护 palette/reverse_map）会退化；因此选择在 384 硬切换。
- 该阈值是规范的一部分；不允许以“更省内存”为理由延迟切换。

### 4.2 Direct/Paletted/Single 的降级策略

为了避免“每次写入后扫 512 项”的最坏情况，降级规则写死为：

- `Direct` 在热路径禁止自动降级为 `Paletted/Single`
- `Paletted` 在热路径禁止自动降级为 `Single`

降级只允许在以下时机发生：

- chunk 卸载前的后台压缩任务
- 明确的“重打包/整理”任务（非编辑热路径）

---

## 5. 读写契约（语义写死）

### 5.1 读取 `get_voxel(lx,ly,lz) -> BlockStateId`

- 必须是纯读操作；不得触发隐式转换或重排
- 并发模型写死为：每个 chunk 使用独立 `RwLock` 保护其 64 个 brick；读取必须只获取该 chunk 的共享锁，禁止获取全局锁

### 5.2 写入 `set_voxel(lx,ly,lz,new_state) -> changed`

语义写死：

- 若旧值与 `new_state` 相同：必须是无副作用（不标 dirty、不重打包）
- 若发生变化：必须更新该 voxel，并触发 dirty（见第 7 节）

写入的结构转换规则写死为：

1) 当前为 `Single(old)`：
   - 若 `old == new_state`：返回 `changed=false`
   - 否则转为 `Paletted`：
     - `palette = [old, new_state]`
     - `indices` 初始化为全 0，然后把目标 `sub_index` 写为 1
2) 当前为 `Paletted`：
   - 若 `new_state` 在 `reverse_map` 中：仅更新 `indices`
   - 否则：
     - 若 `palette_len >= 384`：必须先转为 `Direct`，再写入（避免继续膨胀 palette）
     - 否则把 `new_state` 追加到 `palette` 并更新 `reverse_map`，再写入 `indices`
3) 当前为 `Direct`：直接写 `values[sub_index]`

---

## 6. 为 Meshing 构造 `PaddedChunk`（必须支持）

`meshing.md` 规定 mesher 必须只读取 `PaddedChunk(34³)`，因此存储层必须提供高效的“批量拉取接口”：

- 输入：chunk 坐标 `(cx,cy,cz)`
- 输出：`PaddedChunk`，尺寸固定为 `34×34×34`
  - 内部有效区间 `x,y,z ∈ [1..32]` 对应本 chunk
  - 6 个边界来自邻居 chunk，缺失邻居视为空气

硬约束：

- 构造 `PaddedChunk` 必须只读 blocks 热路径结构，不得触发任何结构互转或重打包。
- 构造 `PaddedChunk` 必须能在并行任务中调用，并且必须按固定锁顺序获取读锁以避免死锁：
  - 锁集合写死为：中心 chunk + 6 个轴向邻居 chunk（共 7 个）
  - 锁顺序写死为：按 `(cx,cy,cz)` 字典序升序获取
  - 获取到所有读锁后完成 `34×34×34` 拷贝，随后立即释放全部锁

---

## 7. Dirty 与占据缓存（与其他系统的契约）

### 7.1 Dirty（与 `meshing.md` 对齐）

dirty 粒度写死为 chunk：

- 任意 `set_voxel` 成功（值变化）必须标记所属 chunk dirty
- 若写入发生在 chunk 边界 voxel（`lx==0/31`、`ly==0/31`、`lz==0/31`），必须同步标记相邻 chunk dirty

### 7.2 Occupancy Brick（与 `far_volumetric.md` 对齐）

每个 chunk 必须维护 `4×4×4` 的 occupancy brick：

- 每个 occupancy brick 覆盖一个 `8×8×8` 区域
- 对外提供只读查询：“某个 AABB 是否可能包含 solid voxel”

该缓存的写入更新必须由 `set_voxel` 驱动（或等价事件驱动），禁止在查询时回扫体素数组。

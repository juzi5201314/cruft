# Cruft 网格生成（Binary Greedy Meshing / Bitmask）

本文档定义 Cruft 的**唯一**网格生成方案：对每个 dirty chunk，构造带 1-voxel padding 的输入块，在 CPU 上生成 **occupancy bitmask** 与 **face bitmasks**，然后对可见面做 **greedy 合并**，输出紧凑的 **Packed-Quad** 流供渲染端使用。

---

## 1. 强约束（写死）

- chunk：`32×32×32`
- padding：每个 chunk 输入必须包含 1-voxel 邻居边界（形成 `34×34×34` 的 padded 区域）
- 输出：只输出 quad（不输出三角形顶点）；渲染端使用 vertex pulling 还原角点
- 渲染层（render layer）：`Opaque`、`Cutout`、`Transparent`

---

## 2. 输入/输出契约（文档层 API）

### 2.1 输入

输入必须是 padded 数据块 `PaddedChunk`（概念）：

- `PaddedChunk` 尺寸：`(CHUNK_SIZE + 2)³ = 34³`
- 内部有效区间：`x,y,z ∈ [1..32]` 对应本 chunk
- 边界区间：`x==0/33`、`y==0/33`、`z==0/33` 来自 6 邻居 chunk（缺失邻居视为空气）

约束：

- mesher 必须只读取 `PaddedChunk`，禁止在 meshing 过程中再次访问世界存储（避免随机 IO 与锁竞争）。
- mesher 必须是纯函数式输出（同输入必然同输出），用于任务并行与版本一致性。

### 2.2 输出

输出为按渲染层分桶的 Packed-Quad 流：

- `OpaqueQuads: Vec<PackedQuad>`
- `CutoutQuads: Vec<PackedQuad>`
- `TransparentQuads: Vec<PackedQuad>`

其中 `PackedQuad` 是一个 `u64`（写死布局）：

- 低 32bit（从低到高位）：`x(6) | y(6) | z(6) | w(5) | h(5) | face(3) | reserved(1)`
- 高 32bit（从低到高位）：`material_key(8) | flags(8) | reserved(16)`

位布局（写死）：

```
low32  = x | (y<<6) | (z<<12) | (w_minus1<<18) | (h_minus1<<23) | (face<<28) | (reserved1<<31)
high32 = material_key | (flags<<8) | (reserved<<16)
packed = low32 | (high32<<32)
```

字段含义（写死）：

- `x,y,z`：quad 起点（chunk 局部坐标，范围 `0..32`，用于表达正向面的 `+1` 边界）
- `w,h`：quad 尺寸（范围 `1..32`），编码为 `w_minus1/h_minus1 ∈ 0..31`
- `face`：6 个方向编码（`0..5`）
- `material_key`：纹理数组 layer（`0..255`）
- `flags`：渲染层/alpha 模式等（与 `rendering.md` 对齐）

---

## 3. 遮挡与层规则（写死）

每个 `BlockStateId` 必须映射到：

- `render_layer`: `Opaque | Cutout | Transparent`
- `is_occluder`: bool（是否遮挡邻居面）
- `material_key: u8`

规则（写死）：

- `Opaque`：`is_occluder = true`
- `Cutout`：`is_occluder = false`
- `Transparent`：`is_occluder = false`

面生成规则（写死）：

对每个 voxel `A` 的每个方向面，取邻居 voxel `B`：

- 若 `A` 是空气：不生成面
- 否则若 `B.is_occluder == true`：不生成面
- 否则：生成面并归入 `A.render_layer`

---

## 4. Binary Greedy Meshing（算法规格）

算法固定为三步：

1) **Occupancy mask**：从 `PaddedChunk` 生成 `opaque_col[x,y]`，每个元素是一个 `u64` bitset（bit 方向为 `z`）。
2) **Face masks**：从 `opaque_col` 生成 6 个方向的可见面 bitmask（每次位运算处理一整列面）。
3) **Greedy merge**：对 face masks 扫描并合并成矩形 quad，生成 `PackedQuad`。

### 4.1 Occupancy mask（写死布局）

- `opaque_col` 维度：`(CHUNK_SIZE + 2) × (CHUNK_SIZE + 2) = 34×34`
- `opaque_col[x,y]` 的 bit `k` 表示 padded 块在 `(x,y,z=k)` 是否为 occluder
- bit `0` 与 bit `33` 属于 padding 边界

### 4.2 Face masks（写死生成规则）

对内部有效区间 `x,y,z ∈ [1..32]`，可见面 mask 的生成规则写死为：

- 对跨列邻居（±X / ±Y）：
  - `visible = occ_here & ~occ_neighbor`
- 对同列邻居（±Z）：
  - `visible_pos = occ_here & ~(occ_here >> 1)`
  - `visible_neg = occ_here & ~(occ_here << 1)`

所有可见面 mask 必须裁掉 padding bit（仅保留内部 `z ∈ [1..32]`）。

### 4.3 Greedy merge（写死合并键）

合并键（必须完全相等才合并）：

- `render_layer`
- `face`
- `material_key`

合并扫描规则写死为：

- 对每个 face、每个切片，读取 `bits_here`
- 用 `ctz(bits_here)` 找到下一个候选 bit
- 扩展矩形时，要求扩展范围内所有候选 bit 的 `material_key` 完全一致
- 扩展完成后生成 1 个 `PackedQuad` 并清除覆盖范围内的 bit

---

## 5. 面朝向与 quad 展开（写死）

face 编码写死为：

- `0`: `+X`
- `1`: `-X`
- `2`: `+Y`
- `3`: `-Y`
- `4`: `+Z`
- `5`: `-Z`

`w/h` 的轴向（写死）：

- `±X` 面：`w` 沿 `+Z`，`h` 沿 `+Y`
- `±Y` 面：`w` 沿 `+X`，`h` 沿 `+Z`
- `±Z` 面：`w` 沿 `+X`，`h` 沿 `+Y`

quad 平面坐标（写死）：

- `+X` 面的平面坐标为 `x`，并位于体素单元的 `x+1` 边界
- `-X` 面的平面坐标为 `x`，并位于体素单元的 `x` 边界
- 其他轴类推：正向面在 `+1` 边界，负向面在 `0` 边界

---

## 6. Dirty chunk 重建（并行与一致性）

dirty 粒度写死为 chunk：

- 任意 voxel 写入导致所属 chunk dirty
- 写入发生在 chunk 边界时，相邻 chunk 同步标记 dirty

任务一致性写死为 generation：

- 每个 chunk 维护单调递增 `generation`
- meshing 任务记录提交时的 `generation`
- 任务完成回到主线程时，若 `generation` 不匹配则丢弃结果

线程与内存（写死）：

- meshing 必须使用 work-stealing 线程池（chunk 粒度任务）。
- worker 数量写死为：`MESH_WORKERS = max(1, logical_cpu_count - 1)`。
- 同一 chunk 在任意时刻最多允许 1 个 in-flight 任务；再次 dirty 只提升 `generation`，不创建并发任务。
- 每个 worker 必须持有复用的 scratch buffer，禁止在 meshing 热路径反复分配/释放内存：
  - `opaque_col[34×34]`（`u64`）
  - `face_masks[6][32×32]`（每格一个 `u64`）
  - 合并阶段的行/列状态（用于扩展矩形）
- meshing 输入必须是 `PaddedChunk` 的快照；快照完成后 worker 任务内禁止读取世界存储。

---

## 7. 边界与裂缝（必须解释清楚）

### 7.1 Chunk 边界

mesher 必须读取邻居 chunk 数据以裁掉边界内部面，否则 chunk 间会出现“重叠双面”或缝隙。

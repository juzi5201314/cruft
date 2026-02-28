# Storage v2：持久化与 IO（WorldHeader / Region / ChunkRecord）

本文档规定 Storage v2 的世界持久化格式与 IO 约束。目标是：高吞吐流式加载/卸载、段级压缩、固定 schema、可校验一致性。

约束：本文件是规范级；实现必须逐条满足。Storage v2 是硬切换，不提供向后兼容读取。

---

## 1. 版本与文件组织（写死）

- 世界格式版本写死为：`WORLD_FORMAT_VERSION = 2`
- 世界由两类文件组成：
  1) `WorldHeader`（全局状态，单文件）
  2) `Region` 文件集合（chunk 数据，按空间分片）

目录布局（写死）：

```
world/
  header.cruft
  regions/
    r.<rx>.<ry>.<rz>.cruft
```

---

## 2. Region 分片（写死）

### 2.1 Region 尺寸

Region 的 chunk 维度写死为：

- `REGION_CHUNKS = 16`（每轴 16 个 chunk）
- 每个 region 覆盖 `16×16×16 = 4096` 个 chunk 槽位

### 2.2 Region 坐标与槽位

- region 坐标：`(rx, ry, rz) = floor_div((cx,cy,cz), REGION_CHUNKS)`
- chunk 在 region 内的局部坐标：
  - `lcx = cx - rx*REGION_CHUNKS`（范围 `0..15`）
  - `lcy = cy - ry*REGION_CHUNKS`
  - `lcz = cz - rz*REGION_CHUNKS`
- 槽位线性索引顺序写死为（`x/z/y`）：

```
slot(lcx,lcy,lcz) = lcx + lcz*16 + lcy*16*16
```

---

## 3. WorldHeader（写死）

WorldHeader 必须至少包含：

- `WORLD_FORMAT_VERSION`（=2）
- 世界 UUID / 创建时间
- 全局配置（seed、规则、时间/日夜等）
- `StateRegistry`（必须）

### 3.1 StateRegistry（写死）

StateRegistry 用于把稳定键映射到热路径 ID：

- `BlockStateKey`：稳定键（例如 `namespace:name` + 排序后的属性表）
- `BlockStateId`：`u32`

序列化约束（写死）：

- `BlockStateId` 必须是 dense 的 `0..N-1`（便于落盘压缩与网络传输）
- `0` 必须保留为 `air`（或等价的“空”状态）
- `BlockStateKey` 的属性表必须按 key 排序后序列化（保证规范化与可复现）

---

## 4. Region 文件结构（写死）

每个 Region 文件必须包含：

1) Region 文件头（magic + 版本 + region 坐标 + 约束常量）
2) Chunk 索引表（固定长度，4096 槽位）
3) ChunkRecord 数据区（变长，按需追加/重写）

### 4.1 Chunk 索引表（写死）

每个槽位必须有一个索引条目：

- `offset`：`u64`（0 表示该槽位没有 chunk）
- `length`：`u32`
- `flags`：`u32`（压缩/校验/保留位）

约束：

- 索引表必须允许 O(1) 定位任意 chunk record
- 更新单个 chunk record 不得要求重写整个 region（允许碎片化，但必须提供后台整理/compact）

---

## 5. ChunkRecord（写死）

ChunkRecord 是一个“分段容器”，必须按 layer 分段存储：

- `BlocksSection`（必须存在）
- `BlockEntitiesSection`（可选）
- `OverlaysSection`（可选，允许出现 0..N 段）
- `EntitiesSection`（可选）
- `ChunkMetaSection`（可选）

### 5.1 段头（写死）

每个段必须带以下元数据：

- `section_kind`（枚举）
- `schema_version`
- `compression`（写死为 `Lz4`）
- `uncompressed_len`
- `compressed_len`
- `checksum`（写死为 `XXH3_64`）

约束：

- 不同段必须允许独立压缩（blocks 与 entities 的可压缩性不同）
- 遇到未知 `section_kind` 必须作为“opaque section”跳过解析，但必须原样保留并在保存时写回（用于扩展与工具链）

---

## 6. BlocksSection 编码（写死）

BlocksSection 必须按 brick 顺序存储（见 `storage_blocks.md` 的 `brick_index`），并对每个 brick 使用以下编码之一：

### 6.1 BrickTag（写死）

- `0`：`Single`
- `1`：`PalettedPacked`
- `2`：`DirectRaw`

保留：

- `3..255` 保留（遇到必须报错）

### 6.2 `Single`（写死）

- 写入 `value: BlockStateId(u32)`

### 6.3 `PalettedPacked`（写死）

- 写入 `palette_len: u16`（范围 `1..=512`）
- 写入 `palette: [BlockStateId; palette_len]`
- 写入 `bits_per_entry: u8`（`bits = ceil(log2(palette_len))`，最小为 1，最大为 9）
- 写入 bit-packed `indices` 流（按 `sub_index` 顺序，长度 `ceil(512 * bits / 8)` bytes）

bit-pack 的位序规则写死为：

- little-endian bit order（低位在前）
- 连续 bitstream，从 `bit_offset = i * bits` 开始

### 6.4 `DirectRaw`（写死）

- 写入 `values: [BlockStateId; 512]`（按 `sub_index` 顺序）

### 6.5 编码选择（写死）

保存时对每个 brick 的编码选择写死为：

- 若 hot brick 为 `Single`：落盘必须为 `Single`
- 否则若 hot brick 为 `Paletted`：落盘必须为 `PalettedPacked`
- 否则（hot brick 为 `Direct`）：落盘必须为 `DirectRaw`

---

## 7. BlockEntities / Overlays / Entities 的持久化（写死约束）

- 每一层必须带 `schema_version`
- 遇到未知类型记录必须原样保留并写回（见 `storage_layers.md`）
- 每段必须可独立压缩与校验
- 禁止把 `StateRegistry`、世界规则等全局信息写进 chunk record

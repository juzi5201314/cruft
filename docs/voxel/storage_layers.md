# Storage v2：分层存储（BlockEntities / Entities / WorldState）

本文档定义 Storage v2 的“非 blocks”数据层。目标是：在不污染 blocks 热路径的前提下，提供可版本化、可按需加载/卸载、可压缩的世界状态存储。

约束：本文件是规范级；实现必须逐条满足。

---

## 1. 总原则（写死）

1) blocks 热路径只处理 `BlockStateId`（见 `storage.md` / `storage_blocks.md`）。
2) 任何“复杂/稀疏/高频变化”的数据必须进入独立层，禁止塞进每 voxel 常驻结构。
3) 每一层必须独立：
   - 序列化格式与 schema version
   - 压缩策略
   - 加载/卸载生命周期

---

## 2. Layers 一览（写死）

Storage v2 的世界数据由下列层构成：

1) `BlocksLayer`：体素方块（`BlockStateId`），brick 三态（热路径权威）
2) `BlockEntityLayer`：方块复杂状态（容器/机器/脚本等），稀疏
3) `EntitiesLayer`：运行时 ECS 实体；持久化按空间分桶
4) `WorldState`：全局状态（seed、规则、注册表等）

---

## 3. BlockEntityLayer（写死）

### 3.1 定义

BlockEntity 表示“绑定到某个 voxel 位置”的复杂对象状态。它与 blocks 的关系必须满足：

- blocks 中的某些 `BlockStateId` **需要**一个 BlockEntity 才能完整表达玩法/交互
- blocks 的渲染/碰撞等热路径不得依赖 BlockEntity 才能成立（例如：mesh 不应遍历 BlockEntity）

### 3.2 索引键（写死）

BlockEntity 必须以 chunk 为分桶，并以“chunk 内 voxel 线性索引”作为键：

```
voxel_index(lx,ly,lz) = lx + lz*32 + ly*32*32
```

约束：

- 键空间写死为 `0..32767`
- 禁止使用 world 坐标字符串或哈希作为热路径键

### 3.3 记录格式（写死）

每条 BlockEntity 记录写死为：

- `type_id`：`u32`（稳定类型 ID）
- `schema_version`：`u16`
- `payload`：`bytes`（类型私有二进制数据）

约束：

- 遇到未知 `type_id` 的记录必须报错并拒绝加载该 ChunkRecord。
- 记录解析失败必须报错并拒绝加载该 ChunkRecord。

### 3.4 生命周期规则（写死）

- 当某 voxel 的 `BlockStateId` 被写入为“不再需要 BlockEntity”的状态时：
  - 必须删除该位置的 BlockEntity 记录（或标记为 tombstone 并在保存时清理）
- 当写入为“需要 BlockEntity”的状态时：
  - 必须创建默认 BlockEntity（或在首次交互时惰性创建，但必须在语义上等价）

---

## 4. OverlayLayers（写死）

Storage v2 不包含 OverlayLayers。

---

## 5. EntitiesLayer（写死）

### 5.1 运行时权威

运行时实体由 ECS 权威管理；存储层负责“按空间加载/卸载 + 持久化”。

### 5.2 空间分桶（写死）

实体必须按 chunk 分桶持久化，桶的选择规则写死为：

- `home_chunk = floor_div(entity_position, CHUNK_SIZE)`
- 若实体拥有 AABB 并跨越多个 chunk：仍使用 `home_chunk` 作为归属桶，但必须同时存储其 AABB（用于流式加载判定）

### 5.3 持久化记录（写死）

每个实体持久化记录至少包含：

- `entity_id`：稳定 ID（世界内唯一）
- `schema_version`
- `components`：一组 `(component_type_id, component_payload)`

约束：

- 遇到未知 component 必须报错并拒绝加载该实体记录。
- 实体的临时运行时状态（如渲染缓存句柄）不得进入持久化记录。

---

## 6. WorldState（写死）

WorldState 是全局数据，必须存放在 `WorldHeader` 中（见 `persistence.md`），至少包含：

- 世界格式版本（v2）
- `StateRegistry`（`BlockStateKey ↔ BlockStateId`）
- 随机种子、时间/日夜、规则/难度等全局配置
- 校验信息（用于检测损坏/不一致）

约束：

- 禁止把 WorldState 混入 chunk record（否则变更会导致全区域重写）。

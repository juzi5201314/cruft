# Cruft 渲染管线（Packed-Quad + Vertex Pulling + MDI + Temporal HZB）

本文档定义 Cruft 体素渲染的**唯一**管线：CPU meshing 输出 `PackedQuad` 流，GPU 端使用 **vertex pulling** 还原顶点，并通过 **Multi-Draw Indirect** 绘制所有可见 chunk；可见性由 **Frustum + Temporal HZB Occlusion Culling** 在 GPU 上生成。

---

## Wave-1 冻结决策（规范级）

当前实现阶段冻结为 **temporal HZB variant**，关键决策如下：

- **Per-view ownership**：每个 extracted view 拥有独立的 voxel culling 状态（HZB 纹理、previous-depth、visible list、indirect/count buffers），禁止 `views.single()` 全局状态假设。
- **Previous-frame HZB**：HZB 构建来源于**上一帧**的 full opaque+cutout depth（从 `ViewDepthTexture` copy level-0，然后 build mip chain），而非 same-frame depth prepass。
- **Opaque + Cutout only**：wave-1 仅覆盖 `Opaque` 与 `Cutout` 渲染层；`Transparent` 明确排除在 wave-1 之外（后续阶段处理）。
- **First-frame fallback**：第一帧或 previous depth 缺失时，降级为 frustum-only culling，禁止未定义 HZB 读取。
- **Temporal HZB 语义**：当前帧的 cutout depth 会写入 depth buffer，但只在**下一帧**的 HZB snapshot 中成为 occluder 输入（same-frame refine 不在 wave-1 范围内）。

Wave-1 排除项（明确不承诺）：

- 同帧 occlusion refine（same-frame depth prepass + re-cull）
- Transparent 排序 / OIT / per-chunk distance sort
- Per-quad / meshlet 粒度剔除
- Far clipmap / volumetric 地平线整合
- MSAA 支持（wave-1 政策：voxel culling views 要求 `Msaa::Off`）

---

## 1. 强约束（写死）

- 近处渲染：光栅化
- 渲染输入：`PackedQuad`（来自 `meshing.md`）
- 绘制方式：MDI（一个 pass 内绘制所有可见 chunk）
- 剔除：Frustum + HZB（GPU 生成可见 draw list）
- 纹理：texture array（与 `docs/textures.md` 的程序化贴图一致）

---

## 2. ECS 实体与数据归属（规范级）

每个 chunk 在主世界中对应一个实体：

- `ChunkEntity(cx, cy, cz)`

该实体必须包含（概念）：

- `ChunkKey(cx, cy, cz)`
- `ChunkBoundsAabb`（世界空间 AABB，用于剔除）
- `ChunkDrawRange`（该 chunk 在 GPU quad buffer 中的 `offset/len`）
- `ChunkGeneration`（与 meshing generation 对齐）

约束：

- 禁止在渲染侧依赖 `Mesh` 顶点资产；体素几何完全由 quad buffer 驱动。
- `ChunkDrawRange` 只能在 meshing 任务完成时更新。

---

## 3. 资产与材质（与程序化贴图管线对齐）

Cruft 的方块贴图使用 texture array（见 `docs/textures.md`）。体素材质固定为：

- 方块贴图：使用 texture array（每个方块/面对应 layer）
- quad：携带 `material_id`（纹理数组 layer，16-bit）
- shader：按 `material_id` 从 array 采样

alpha 模式固定为：

- `Opaque`：写深度，参与遮挡
- `Cutout`：alpha test（discard），写深度
- `Transparent`：alpha blend，不写深度（或按材质策略写死），单独 pass

---

## 4. 数据上传与绘制（固定流水线）

### 4.1 CPU → GPU 数据流

meshing 任务产出（固定）：

- `ChunkKey`
- `generation`
- `OpaqueQuads/CutoutQuads/TransparentQuads`（`Vec<PackedQuad>`）

主线程合并产出（固定）：

- 把 quad 追加/写入全局 `VoxelQuadBuffer`（GPU storage buffer）
- 更新对应 chunk 实体的 `ChunkDrawRange`
- 更新对应 chunk 实体的 `ChunkGeneration`

### 4.2 GPU 侧数据结构（固定）

GPU 必须维护以下 buffer/texture（per-view ownership）：

- `QuadBuffer`：`PackedQuad[]`（storage，全局共享）
- `ChunkMetaBuffer`：每 chunk 的 `offset/len/aabb`（storage，全局共享）
- `IndirectDrawBuffer`：MDI draw commands（indirect，per-view）
- `VisibleChunkList`：可见 chunk 索引（storage，per-view）
- `HZB`：depth pyramid 纹理（texture，per-view，来源于 previous-frame depth）
- `PreviousDepth`：level-0 depth copy 纹理（per-view，从 `ViewDepthTexture` capture）

`IndirectDrawBuffer` 的 draw 模型写死为 "indexed quad + instancing"：

- 固定 index buffer：`[0,1,2, 2,1,3]`（6 indices）
- 每个 instance 对应一个 `PackedQuad`
- 每条 indirect command 对应一个 chunk 的一个渲染层：
  - `index_count = 6`
  - `instance_count = quad_len`
  - `first_index = 0`
  - `base_vertex = 0`
  - `first_instance = quad_offset`

vertex shader 必须用 `first_instance + instance_index` 作为 `PackedQuad` 索引读取 `QuadBuffer`。

---

## 5. 每帧执行顺序（固定）

1) **Previous-depth capture**：从 `ViewDepthTexture` copy level-0 到 per-view `PreviousDepth` 纹理。  
2) **Build HZB**：对 `PreviousDepth` 生成多级 mip（depth pyramid，2×2 min-reduction）。  
3) **GPU Culling**：compute 读取 `ChunkMetaBuffer` 与 HZB，执行 frustum + temporal HZB 测试，写入 per-view 的 `VisibleChunkList` 与 `IndirectDrawBuffer`（dense compaction，append order 无保证）。  
4) **Voxel Opaque Pass**：vertex pulling + MDI 绘制所有可见 `Opaque` quads（写深度）。  
5) **Voxel Cutout Pass**：vertex pulling + MDI 绘制所有可见 `Cutout` quads（写深度，alpha test discard）。  
6) **Voxel Transparent Pass**：wave-1 排除；后续阶段按 chunk 距离排序后绘制（chunk 内不排序）。

第一帧 / 无 previous depth 时：步骤 1-2 跳过，步骤 3 降级为 frustum-only。  

---

## 6. 不变量（写死）

- 所有体素几何必须从 `QuadBuffer` 驱动，禁止走 `Mesh` 顶点路径。
- `Opaque/Cutout` 必须写深度并参与 HZB 的遮挡剔除输入（current-frame cutout depth → next-frame HZB）。
- `Transparent` 明确排除在 wave-1 外；后续阶段的排序粒度固定为 chunk（chunk 内不排序）。
- Per-view ownership：HZB、previous-depth、visible list、indirect/count buffers 均为 per-view 资源，禁止全局单例假设。
- Temporal HZB 语义：previous-frame depth 是 occlusion 的唯一来源；first-frame 必须使用 frustum-only fallback。


## PackedQuad material field update

当前实现已将 `PackedQuad` 的材质位宽从 `u8` 提升为 `u16`（high32: material_id(16) + flags(8) + reserved(8)），用于承载更大规模材质库，并为后续 `material_id -> layer/face binding` 的间接寻址预留空间。

## Wave-1 Scope Summary（冻结）

- ✅ Per-view `VoxelViewState` 生命周期管理
- ✅ Previous-frame depth capture + HZB build（自持有，不依赖 `ViewDepthPyramid`）
- ✅ GPU culling：frustum + temporal HZB，dense visible compaction
- ✅ Per-layer indirect submission（`Opaque + Cutout`）
- ✅ First-frame frustum-only fallback
- ❌ Transparent（后续阶段）
- ❌ Same-frame occlusion refine（后续阶段）
- ❌ Per-quad / meshlet culling（后续阶段）
- ❌ MSAA support（wave-1 政策：禁用）

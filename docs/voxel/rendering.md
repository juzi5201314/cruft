# Cruft 渲染管线（Packed-Quad + Vertex Pulling + MDI + HZB）

本文档定义 Cruft 体素渲染的**唯一**管线：CPU meshing 输出 `PackedQuad` 流，GPU 端使用 **vertex pulling** 还原顶点，并通过 **Multi-Draw Indirect** 绘制所有可见 chunk；可见性由 **Frustum + HZB Occlusion Culling** 在 GPU 上生成 indirect draw list。

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
- quad：携带 `material_key`（纹理数组 layer）
- shader：按 `material_key` 从 array 采样

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

GPU 必须维护以下 buffer/texture：

- `QuadBuffer`：`PackedQuad[]`（storage）
- `ChunkMetaBuffer`：每 chunk 的 `offset/len/aabb`（storage）
- `IndirectDrawBuffer`：MDI draw commands（indirect）
- `VisibleChunkList`：可见 chunk 索引（storage）
- `HZB`：深度金字塔纹理（texture）

`IndirectDrawBuffer` 的 draw 模型写死为 “indexed quad + instancing”：

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

1) **Depth prepass（近景 Opaque/Cutout）**：输出深度缓冲（用于构建 HZB）。  
2) **Build HZB**：从深度缓冲生成多级 mip（depth pyramid）。  
3) **GPU Culling**：compute 读取 `ChunkMetaBuffer`，执行 frustum + HZB 测试，写入 `VisibleChunkList` 与 `IndirectDrawBuffer`。  
4) **Voxel Opaque Pass**：vertex pulling + MDI 绘制所有可见 `Opaque` quads。  
5) **Voxel Cutout Pass**：vertex pulling + MDI 绘制所有可见 `Cutout` quads。  
6) **Voxel Transparent Pass**：按 chunk 距离排序后绘制（排序结果写死为 chunk 粒度；每 chunk 内 quads 不再排序）。  

---

## 6. 不变量（写死）

- 所有体素几何必须从 `QuadBuffer` 驱动，禁止走 `Mesh` 顶点路径。
- `Opaque/Cutout` 必须写深度并参与 HZB 的遮挡剔除输入。
- `Transparent` 的排序粒度固定为 chunk（chunk 内不排序）。

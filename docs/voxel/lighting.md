# Cruft 光照（无 GI / 性能优先）

本文档定义 Cruft 的光照主方案：体素场景不实现体素 GI；近景采用常规光栅化光照，远景雾化由 `far_volumetric.md` 的遮蔽与透过率参与合成。

---

## 1. 光照组成（写死）

- **Direct Light**：方向光（太阳）+ 阴影
- **Ambient**：环境光（常量或天空盒/环境贴图）
- **Fog Compose**：将 `far_volumetric.md` 输出的 `horizon_occlusion` 与 `fog_transmittance` 合成到最终画面

---

## 2. 约束（写死）

- 禁止体素 GI、cone tracing、SVOGI 等体积间接光。
- 近景 shading 必须与 texture array 的 `material_key` 采样一致。
- 远景合成只使用遮蔽与雾透过率，不引入额外体积照明。

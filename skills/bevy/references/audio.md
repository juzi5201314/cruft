# Audio（Bevy v0.18）：AudioPlayer / AudioSource / Spatial audio

Bevy 音频系统以实体组件的方式播放音频：

- 在实体上插入 `AudioPlayer`（引用某个 `AudioSource` 资产）
- 通过 `PlaybackSettings` 控制循环/音量/速度等

## 0) 入口与 examples

- crate 入口：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_audio/src/lib.rs`
- examples：
  - `examples/audio/audio.rs`（最小播放）
  - `examples/audio/audio_control.rs`（控制播放，视文件存在）
  - `examples/audio/spatial_audio_3d.rs`（空间音频）

## 1) 最小播放例子

```rust
fn setup(asset_server: Res<AssetServer>, mut commands: Commands) {
    commands.spawn(AudioPlayer::new(asset_server.load("sounds/example.ogg")));
}
```

示例：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/audio/audio.rs`

## 2) 循环与播放设置

常见是搭配 `PlaybackSettings::LOOP`：

- crate 文档里有完整示例：
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_audio/src/lib.rs`（顶部 no_run 代码块）

## 3) 空间音频（Spatial）

空间音频通常需要：

- listener（监听器）
- emitter（音源）的位置（Transform）
- `DefaultSpatialScale` 等配置

建议直接跑示例对照：

- `examples/audio/spatial_audio_3d.rs`

并注意 AudioPlugin 的系统集是放在 PostUpdate 且在 Transform propagation 之后执行：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_audio/src/lib.rs`（`after(TransformSystems::Propagate)`）

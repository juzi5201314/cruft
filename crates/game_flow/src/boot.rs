use bevy::prelude::*;
use bitflags::bitflags;

use crate::request::FlowRequest;
use crate::state::AppState;

bitflags! {
    /// BootLoading 期间必须完成的任务集合（固定 bitflags，零注册表）。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BootReady: u32 {
        const PROC_TEXTURES = 1 << 0;
        const SAVE_INDEX    = 1 << 1;
    }
}

/// Boot readiness 位图：由各子系统（贴图/存档）写入。
#[derive(Resource, Debug, Clone, Copy)]
pub struct BootReadiness(pub BootReady);

impl Default for BootReadiness {
    fn default() -> Self {
        Self(BootReady::empty())
    }
}

/// BootLoading UI 使用的聚合进度（变更驱动）。
#[derive(Resource, Debug, Clone)]
pub struct BootProgress {
    pub value: f32,
    pub label: String,
}

impl Default for BootProgress {
    fn default() -> Self {
        Self {
            value: 0.0,
            label: "Initializing…".to_string(),
        }
    }
}

const WEIGHT_PROC_TEXTURES: f32 = 0.6;
const WEIGHT_SAVE_INDEX: f32 = 0.4;

fn compute_progress(bits: BootReady) -> (f32, &'static str) {
    let mut value = 0.0;
    let mut missing = 0;

    if bits.contains(BootReady::PROC_TEXTURES) {
        value += WEIGHT_PROC_TEXTURES;
    } else {
        missing += 1;
    }

    if bits.contains(BootReady::SAVE_INDEX) {
        value += WEIGHT_SAVE_INDEX;
    } else {
        missing += 1;
    }

    let label = match missing {
        0 => "Ready",
        _ if !bits.contains(BootReady::PROC_TEXTURES) => "Generating textures",
        _ => "Scanning saves",
    };

    (value.clamp(0.0, 1.0), label)
}

/// BootLoading 阶段聚合 readiness，并在全部完成后切换到 FrontEnd。
pub fn update_boot_progress(
    readiness: Option<Res<BootReadiness>>,
    mut progress: ResMut<BootProgress>,
    mut requests: MessageWriter<FlowRequest>,
    app: Res<State<AppState>>,
) {
    if *app.get() != AppState::BootLoading {
        return;
    }

    let bits = readiness.map(|r| r.0).unwrap_or(BootReady::empty());
    let (value, label) = compute_progress(bits);

    if progress.value != value || progress.label != label {
        progress.value = value;
        progress.label = label.to_string();
    }

    if value >= 1.0 {
        requests.write(FlowRequest::EnterFrontEnd);
    }
}

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::tasks::{futures_lite::future, AsyncComputeTaskPool, Task};

use cruft_game_flow::{BootReadiness, BootReady};

use crate::error::TextureDataError;
use crate::generator::{build_runtime_texture_assets, RuntimeTextureBuild};
use crate::TextureRuntimePacks;

const TEXTURE_DATA_PATH: &str = "assets/texture_data/blocks.texture.json";

#[derive(Resource, Debug, Clone, Default)]
pub enum ProcTexturesStatus {
    #[default]
    Loading,
    Ready,
    Failed(String),
}

#[derive(Resource)]
struct ProcTexturesTask(Task<Result<RuntimeTextureBuild, TextureDataError>>);

pub struct ProcTexturesPlugin;

impl Plugin for ProcTexturesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProcTexturesStatus>()
            .add_systems(Startup, spawn_proc_texture_task)
            .add_systems(Update, poll_proc_texture_task);
    }
}

fn spawn_proc_texture_task(mut commands: Commands) {
    let path = PathBuf::from(TEXTURE_DATA_PATH);
    let pool = AsyncComputeTaskPool::get();
    let task = pool.spawn(async move {
        let compiled = crate::compiler::load_and_compile_texture_set(&path)?;
        build_runtime_texture_assets(&compiled)
    });
    commands.insert_resource(ProcTexturesTask(task));
}

fn poll_proc_texture_task(
    mut commands: Commands,
    task: Option<ResMut<ProcTexturesTask>>,
    mut status: ResMut<ProcTexturesStatus>,
    mut images: ResMut<Assets<Image>>,
    mut boot: ResMut<BootReadiness>,
) {
    let Some(mut task) = task else {
        return;
    };
    let Some(result) = future::block_on(future::poll_once(&mut task.0)) else {
        return;
    };
    match result {
        Ok(runtime) => {
            let pack = runtime.pack_data.create_runtime_pack(&mut images);
            commands.insert_resource(runtime.registry);
            commands.insert_resource(TextureRuntimePacks { packs: vec![pack] });
            *status = ProcTexturesStatus::Ready;
            boot.0.insert(BootReady::PROC_TEXTURES);
        }
        Err(error) => {
            log::error!("Procedural textures initialization failed: {error}");
            *status = ProcTexturesStatus::Failed(error.to_string());
        }
    }
    commands.remove_resource::<ProcTexturesTask>();
}

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use cruft_voxel::{ChunkKey, VoxelWorld};

use crate::voxel_collision::{move_with_voxel_aabb_collision, VoxelAabbCollider};

#[derive(Component, Debug)]
pub(crate) struct FpsPlayer;

#[derive(Component, Debug)]
pub(crate) struct FpsCamera;

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct FpsController {
    pub(crate) mouse_sensitivity: f32,
    pub(crate) move_speed: f32,
    pub(crate) gravity: f32,
    pub(crate) jump_speed: f32,
    pub(crate) max_pitch_radians: f32,
    pub(crate) eye_height: f32,
    pub(crate) collider: VoxelAabbCollider,
}

impl Default for FpsController {
    fn default() -> Self {
        let height = 1.8;
        let radius = 0.3;
        Self {
            mouse_sensitivity: 0.0025,
            move_speed: 6.5,
            gravity: -24.0,
            jump_speed: 8.5,
            max_pitch_radians: 89_f32.to_radians(),
            eye_height: 1.6,
            collider: VoxelAabbCollider {
                half_extents: Vec3::new(radius, height * 0.5, radius),
                center_offset: Vec3::new(0.0, height * 0.5, 0.0),
            },
        }
    }
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub(crate) struct FpsVelocity(pub(crate) Vec3);

#[derive(Component, Debug, Default, Clone, Copy)]
pub(crate) struct FpsGrounded(pub(crate) bool);

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct FpsLook {
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
}

impl Default for FpsLook {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

pub(crate) fn lock_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    let Ok(mut cursor) = cursors.single_mut() else {
        return;
    };
    cursor.visible = false;
    cursor.grab_mode = CursorGrabMode::Locked;
}

pub(crate) fn unlock_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    let Ok(mut cursor) = cursors.single_mut() else {
        return;
    };
    cursor.visible = true;
    cursor.grab_mode = CursorGrabMode::None;
}

pub(crate) fn apply_mouse_look(
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut players: Query<
        (&FpsController, &mut FpsLook, &mut Transform, &Children),
        (With<FpsPlayer>, Without<FpsCamera>),
    >,
    mut cameras: Query<&mut Transform, (With<FpsCamera>, Without<FpsPlayer>)>,
) {
    let delta = mouse_motion.delta;
    if delta.length_squared() <= f32::EPSILON {
        return;
    }

    for (controller, mut look, mut player_xform, children) in &mut players {
        look.yaw -= delta.x * controller.mouse_sensitivity;
        look.pitch -= delta.y * controller.mouse_sensitivity;
        look.pitch = look
            .pitch
            .clamp(-controller.max_pitch_radians, controller.max_pitch_radians);

        player_xform.rotation = Quat::from_rotation_y(look.yaw);

        for &child in children {
            if let Ok(mut cam_xform) = cameras.get_mut(child) {
                cam_xform.rotation = Quat::from_rotation_x(look.pitch);
                break;
            }
        }
    }
}

pub(crate) fn apply_movement_and_physics(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    world: Option<Res<VoxelWorld>>,
    mut players: Query<
        (
            &FpsController,
            &FpsLook,
            &mut Transform,
            &mut FpsVelocity,
            &mut FpsGrounded,
        ),
        With<FpsPlayer>,
    >,
) {
    let dt = time.delta_secs().min(0.05);
    let world = world.as_deref();

    for (controller, look, mut xform, mut vel, mut grounded) in &mut players {
        // 水平速度：由 WASD 直接决定（先简单可用，后续可换成加速度/摩擦模型）。
        let yaw_rot = Quat::from_rotation_y(look.yaw);
        let forward = yaw_rot * Vec3::NEG_Z;
        let right = yaw_rot * Vec3::X;

        let mut wish = Vec3::ZERO;
        if keys.pressed(KeyCode::KeyW) {
            wish += forward;
        }
        if keys.pressed(KeyCode::KeyS) {
            wish -= forward;
        }
        if keys.pressed(KeyCode::KeyD) {
            wish += right;
        }
        if keys.pressed(KeyCode::KeyA) {
            wish -= right;
        }
        wish.y = 0.0;
        if wish.length_squared() > 0.0 {
            wish = wish.normalize();
        }

        vel.0.x = wish.x * controller.move_speed;
        vel.0.z = wish.z * controller.move_speed;

        // 跳跃
        if grounded.0 && keys.just_pressed(KeyCode::Space) {
            vel.0.y = controller.jump_speed;
            grounded.0 = false;
        }

        // 重力
        vel.0.y += controller.gravity * dt;

        let out = if let Some(world) = world {
            let is_solid = |wx: i32, wy: i32, wz: i32| is_world_voxel_solid(world, wx, wy, wz);
            move_with_voxel_aabb_collision(
                xform.translation,
                vel.0,
                dt,
                controller.collider,
                Some(&is_solid),
            )
        } else {
            move_with_voxel_aabb_collision(xform.translation, vel.0, dt, controller.collider, None)
        };

        xform.translation = out.feet_position;
        vel.0 = out.velocity;
        grounded.0 = out.grounded;
    }
}

fn is_world_voxel_solid(world: &VoxelWorld, wx: i32, wy: i32, wz: i32) -> bool {
    let (chunk, local) = ChunkKey::from_world_voxel(IVec3::new(wx, wy, wz));
    let id = world.storage.get_voxel(chunk, local.x, local.y, local.z);
    !world.defs.is_air(id)
}

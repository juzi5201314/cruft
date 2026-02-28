use bevy::prelude::*;

const EPS: f32 = 0.001;
const MAX_AXIS_STEP: f32 = 0.25;

#[derive(Debug, Clone, Copy)]
pub(crate) struct VoxelAabbCollider {
    /// 以“碰撞盒中心”为参考的半尺寸。
    pub(crate) half_extents: Vec3,
    /// 从 entity 的 `Transform.translation`（这里约定为“脚底点”）到碰撞盒中心的偏移。
    pub(crate) center_offset: Vec3,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MoveResult {
    pub(crate) feet_position: Vec3,
    pub(crate) velocity: Vec3,
    pub(crate) grounded: bool,
}

pub(crate) fn move_with_voxel_aabb_collision(
    feet_position: Vec3,
    velocity: Vec3,
    dt: f32,
    collider: VoxelAabbCollider,
    is_solid_at: Option<&dyn Fn(i32, i32, i32) -> bool>,
) -> MoveResult {
    let Some(is_solid_at) = is_solid_at else {
        return MoveResult {
            feet_position: feet_position + velocity * dt,
            velocity,
            grounded: false,
        };
    };

    let mut pos = feet_position;
    let mut vel = velocity;
    let mut grounded = false;

    // 先处理 Y：落地后再做水平移动更直观（避免“空中穿洞后落入”）。
    move_axis(
        &mut pos,
        &mut vel,
        dt,
        collider,
        is_solid_at,
        Axis::Y,
        &mut grounded,
    );
    move_axis(
        &mut pos,
        &mut vel,
        dt,
        collider,
        is_solid_at,
        Axis::X,
        &mut grounded,
    );
    move_axis(
        &mut pos,
        &mut vel,
        dt,
        collider,
        is_solid_at,
        Axis::Z,
        &mut grounded,
    );

    MoveResult {
        feet_position: pos,
        velocity: vel,
        grounded,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
    Z,
}

fn move_axis(
    pos: &mut Vec3,
    vel: &mut Vec3,
    dt: f32,
    collider: VoxelAabbCollider,
    is_solid_at: &dyn Fn(i32, i32, i32) -> bool,
    axis: Axis,
    grounded: &mut bool,
) {
    let delta = axis_value(*vel, axis) * dt;
    if delta.abs() <= f32::EPSILON {
        return;
    }

    let sign = delta.signum();
    let mut remaining = delta;

    while remaining.abs() > f32::EPSILON {
        let step = sign * remaining.abs().min(MAX_AXIS_STEP);
        remaining -= step;

        let mut next_pos = *pos;
        *axis_value_mut(&mut next_pos, axis) += step;

        if !collides_on_axis_face(next_pos, collider, is_solid_at, axis, step) {
            *pos = next_pos;
            continue;
        }

        // 发生碰撞：贴住面并清零该轴速度。
        let corrected = correct_position_against_face(next_pos, collider, axis, step);
        *axis_value_mut(pos, axis) = corrected;
        *axis_value_mut(vel, axis) = 0.0;

        if axis == Axis::Y && step < 0.0 {
            *grounded = true;
        }

        break;
    }
}

fn collides_on_axis_face(
    feet_position: Vec3,
    collider: VoxelAabbCollider,
    is_solid_at: &dyn Fn(i32, i32, i32) -> bool,
    axis: Axis,
    step: f32,
) -> bool {
    let (aabb_min, aabb_max) = aabb_min_max(feet_position, collider);

    let Some((min_x, max_x)) = voxel_range_inclusive(aabb_min.x, aabb_max.x) else {
        return false;
    };
    let Some((min_y, max_y)) = voxel_range_inclusive(aabb_min.y, aabb_max.y) else {
        return false;
    };
    let Some((min_z, max_z)) = voxel_range_inclusive(aabb_min.z, aabb_max.z) else {
        return false;
    };

    match axis {
        Axis::X => {
            let face_x = if step > 0.0 {
                (aabb_max.x - EPS).floor() as i32
            } else {
                (aabb_min.x + EPS).floor() as i32
            };
            for y in min_y..=max_y {
                for z in min_z..=max_z {
                    if is_solid_at(face_x, y, z) {
                        return true;
                    }
                }
            }
            false
        }
        Axis::Y => {
            let face_y = if step > 0.0 {
                (aabb_max.y - EPS).floor() as i32
            } else {
                (aabb_min.y + EPS).floor() as i32
            };
            for x in min_x..=max_x {
                for z in min_z..=max_z {
                    if is_solid_at(x, face_y, z) {
                        return true;
                    }
                }
            }
            false
        }
        Axis::Z => {
            let face_z = if step > 0.0 {
                (aabb_max.z - EPS).floor() as i32
            } else {
                (aabb_min.z + EPS).floor() as i32
            };
            for x in min_x..=max_x {
                for y in min_y..=max_y {
                    if is_solid_at(x, y, face_z) {
                        return true;
                    }
                }
            }
            false
        }
    }
}

fn correct_position_against_face(
    next_feet: Vec3,
    collider: VoxelAabbCollider,
    axis: Axis,
    step: f32,
) -> f32 {
    let next_center = next_feet + collider.center_offset;
    let half = collider.half_extents;

    match axis {
        Axis::X => {
            let next_aabb_max = next_center.x + half.x;
            let next_aabb_min = next_center.x - half.x;
            if step > 0.0 {
                let voxel_x = (next_aabb_max - EPS).floor() as f32;
                let corrected_center_x = voxel_x - EPS - half.x;
                corrected_center_x - collider.center_offset.x
            } else {
                let voxel_x = (next_aabb_min + EPS).floor() as f32;
                let corrected_center_x = voxel_x + 1.0 + EPS + half.x;
                corrected_center_x - collider.center_offset.x
            }
        }
        Axis::Y => {
            let next_aabb_max = next_center.y + half.y;
            let next_aabb_min = next_center.y - half.y;
            if step > 0.0 {
                let voxel_y = (next_aabb_max - EPS).floor() as f32;
                let corrected_center_y = voxel_y - EPS - half.y;
                corrected_center_y - collider.center_offset.y
            } else {
                let voxel_y = (next_aabb_min + EPS).floor() as f32;
                let corrected_center_y = voxel_y + 1.0 + EPS + half.y;
                corrected_center_y - collider.center_offset.y
            }
        }
        Axis::Z => {
            let next_aabb_max = next_center.z + half.z;
            let next_aabb_min = next_center.z - half.z;
            if step > 0.0 {
                let voxel_z = (next_aabb_max - EPS).floor() as f32;
                let corrected_center_z = voxel_z - EPS - half.z;
                corrected_center_z - collider.center_offset.z
            } else {
                let voxel_z = (next_aabb_min + EPS).floor() as f32;
                let corrected_center_z = voxel_z + 1.0 + EPS + half.z;
                corrected_center_z - collider.center_offset.z
            }
        }
    }
}

fn aabb_min_max(feet_position: Vec3, collider: VoxelAabbCollider) -> (Vec3, Vec3) {
    let center = feet_position + collider.center_offset;
    let min = center - collider.half_extents;
    let max = center + collider.half_extents;
    (min, max)
}

fn voxel_range_inclusive(min: f32, max: f32) -> Option<(i32, i32)> {
    if max <= min {
        return None;
    }
    let min_i = min.floor() as i32;
    let max_i = (max - EPS).floor() as i32;
    if max_i < min_i {
        None
    } else {
        Some((min_i, max_i))
    }
}

fn axis_value(v: Vec3, axis: Axis) -> f32 {
    match axis {
        Axis::X => v.x,
        Axis::Y => v.y,
        Axis::Z => v.z,
    }
}

fn axis_value_mut(v: &mut Vec3, axis: Axis) -> &mut f32 {
    match axis {
        Axis::X => &mut v.x,
        Axis::Y => &mut v.y,
        Axis::Z => &mut v.z,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falling_stops_on_ground_and_sets_grounded() {
        let is_solid = |_: i32, wy: i32, _: i32| wy < 0;
        let collider = VoxelAabbCollider {
            half_extents: Vec3::new(0.3, 0.9, 0.3),
            center_offset: Vec3::new(0.0, 0.9, 0.0),
        };

        let feet = Vec3::new(0.0, 0.2, 0.0);
        let vel = Vec3::new(0.0, -10.0, 0.0);
        let out = move_with_voxel_aabb_collision(feet, vel, 0.1, collider, Some(&is_solid));

        // 地面是 y < 0 的实心体素，玩家脚底应被夹在 y=0 之上。
        assert!(out.grounded);
        assert!(out.feet_position.y >= 0.0);
        assert_eq!(out.velocity.y, 0.0);
    }
}

use bevy::{
    camera::Camera2d,
    ecs::{
        query::With,
        system::{Res, Single},
    },
    input::{ButtonInput, keyboard::KeyCode},
    math::Vec2,
    time::Time,
    transform::components::Transform,
};

const CAMERA_SPEED: f32 = 500.0;

pub fn move_camera(
    mut camera: Single<&mut Transform, With<Camera2d>>,
    time: Res<Time>,
    kb_input: Res<ButtonInput<KeyCode>>,
) {
    let mut direction = Vec2::ZERO;

    if kb_input.pressed(KeyCode::KeyW) {
        direction.y += 1.;
    }

    if kb_input.pressed(KeyCode::KeyS) {
        direction.y -= 1.;
    }

    if kb_input.pressed(KeyCode::KeyA) {
        direction.x -= 1.;
    }

    if kb_input.pressed(KeyCode::KeyD) {
        direction.x += 1.;
    }

    // Progressively update the player's position over time. Normalize the
    // direction vector to prevent it from exceeding a magnitude of 1 when
    // moving diagonally.
    let move_delta = direction.normalize_or_zero() * CAMERA_SPEED * time.delta_secs();
    //player.translation += move_delta.extend(0.);

    camera.translation += move_delta.extend(0.);
}

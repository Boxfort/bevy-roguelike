use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        query::{Has, With, Without},
        system::{Commands, Query, Res, Single},
    },
    input::{ButtonInput, keyboard::KeyCode},
    math::{IVec2, Vec2},
    prelude::Deref,
    sprite_render::TilemapChunkTileData,
    transform::components::Transform,
};
use bevy_text_mode::TextModeSprite;

use crate::{
    components::{position::Position, renderable::Renderable},
    map::renderer::{CHUNK_SIZE, ChunkPosition, TILE_PIXEL_DISPLAY_SIZE},
};

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct WantsToMove {
    direction: IVec2,
}

pub fn player_input(
    mut commands: Commands,
    kb_input: Res<ButtonInput<KeyCode>>,
    player: Single<Entity, With<Player>>,
) {
    let mut direction = IVec2::ZERO;

    if kb_input.pressed(KeyCode::Numpad7)
        || kb_input.pressed(KeyCode::Numpad8)
        || kb_input.pressed(KeyCode::Numpad9)
    {
        direction.y += 1;
    }

    if kb_input.pressed(KeyCode::Numpad1)
        || kb_input.pressed(KeyCode::Numpad2)
        || kb_input.pressed(KeyCode::Numpad3)
    {
        direction.y -= 1;
    }

    if kb_input.pressed(KeyCode::Numpad1)
        || kb_input.pressed(KeyCode::Numpad4)
        || kb_input.pressed(KeyCode::Numpad7)
    {
        direction.x -= 1;
    }

    if kb_input.pressed(KeyCode::Numpad3)
        || kb_input.pressed(KeyCode::Numpad6)
        || kb_input.pressed(KeyCode::Numpad9)
    {
        direction.x += 1;
    }

    commands.entity(*player).insert(WantsToMove { direction });
}

pub fn try_move_player(
    mut player: Single<(&mut Position, &WantsToMove), With<Player>>,
    mut entities_with_sprites: Query<
        (&Renderable, &mut Transform, &Position),
        (
            With<TextModeSprite>,
            Without<Player>,
            Without<TilemapChunkTileData>,
        ),
    >,
    mut tilemap_chunks: Query<(&ChunkPosition, &mut Transform), With<TilemapChunkTileData>>,
) {
    let chunk_offset = Vec2::splat(((CHUNK_SIZE / 2) * TILE_PIXEL_DISPLAY_SIZE) as f32);

    player.0.0.x += player.1.direction.x;
    player.0.0.y += player.1.direction.y;

    for (chunk_pos, mut transform) in tilemap_chunks.iter_mut() {
        transform.translation.x = (((chunk_pos.pos.x * CHUNK_SIZE) - player.0.0.x)
            * TILE_PIXEL_DISPLAY_SIZE) as f32
            + chunk_offset.x;
        transform.translation.y = (((chunk_pos.pos.y * CHUNK_SIZE) - player.0.0.y)
            * TILE_PIXEL_DISPLAY_SIZE) as f32
            + chunk_offset.x;
    }

    for (_, mut transform, position) in entities_with_sprites.iter_mut() {
        transform.translation.x = ((position.0.x - player.0.0.x) * TILE_PIXEL_DISPLAY_SIZE) as f32;
        transform.translation.y = ((position.0.y - player.0.0.y) * TILE_PIXEL_DISPLAY_SIZE) as f32;
    }
}

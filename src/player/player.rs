use bevy::{ecs::{component::Component, query::{Has, With, Without}, system::{Commands, Query, Res, Single}}, input::{ButtonInput, keyboard::KeyCode}, math::{IVec2, Vec2}, sprite_render::TilemapChunkTileData, transform::components::Transform};
use bevy_text_mode::TextModeSprite;

use crate::{components::{position::Position, renderable::Renderable}, map::renderer::{CHUNK_SIZE, ChunkPosition, TILE_PIXEL_DISPLAY_SIZE}};

#[derive(Component)]
pub struct Player;

pub fn player_input(
    mut commands: Commands,
    kb_input: Res<ButtonInput<KeyCode>>,
    mut player: Single<&mut Position, With<Player>>,
    mut entities_with_sprites: Query<(&Renderable, &mut Transform, &Position), (With<TextModeSprite>, Without<Player>, Without<TilemapChunkTileData>)>,
    mut tilemap_chunks: Query<(&ChunkPosition, &mut Transform), With<TilemapChunkTileData>>,
) {
    let mut direction = IVec2::ZERO;

    if kb_input.just_pressed(KeyCode::Numpad7) || kb_input.just_pressed(KeyCode::Numpad8) || kb_input.just_pressed(KeyCode::Numpad9){
        direction.y += 1;
    }

    if kb_input.just_pressed(KeyCode::Numpad1) || kb_input.just_pressed(KeyCode::Numpad2) || kb_input.just_pressed(KeyCode::Numpad3){
        direction.y -= 1;
    }

    if kb_input.just_pressed(KeyCode::Numpad1) || kb_input.just_pressed(KeyCode::Numpad4) || kb_input.just_pressed(KeyCode::Numpad7){
        direction.x -= 1;
    }

    if kb_input.just_pressed(KeyCode::Numpad3) || kb_input.just_pressed(KeyCode::Numpad6) || kb_input.just_pressed(KeyCode::Numpad9){
        direction.x += 1;
    }

    player.x += direction.x;
    player.y += direction.y;

    let chunk_offset= Vec2::splat(((CHUNK_SIZE/2)*TILE_PIXEL_DISPLAY_SIZE) as f32);


    // Tilemapchunk pos = ((chunk_coord * chunk_size) - player_pos) * TILE_DISPLAY_SIZE

    for (chunk_pos, mut transform) in tilemap_chunks.iter_mut() {
        transform.translation.x = (((chunk_pos.pos.x * CHUNK_SIZE) - player.x) * TILE_PIXEL_DISPLAY_SIZE) as f32 + chunk_offset.x;
        transform.translation.y = (((chunk_pos.pos.y * CHUNK_SIZE) - player.y) * TILE_PIXEL_DISPLAY_SIZE) as f32 + chunk_offset.x;
    }

    for (_, mut transform, position) in entities_with_sprites.iter_mut()  {
        transform.translation.x = ((position.x - player.x) * TILE_PIXEL_DISPLAY_SIZE) as f32;
        transform.translation.y = ((position.y - player.y) * TILE_PIXEL_DISPLAY_SIZE) as f32;
    }

}

// PLAYER MOVES
// ====
// POSITION IS UPDATED
// ALL TILEMAPS MUST SHIFT VISUALLY
// ALL ENTITIES MUST SHIFT VISUALLY
// PLAYER STAYS PUT
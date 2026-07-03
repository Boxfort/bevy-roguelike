use bevy::{
    asset::{Assets, Handle},
    ecs::{
        entity::Entity,
        system::{Commands, Query, Res, ResMut},
    },
    image::{Image, TextureAtlas, TextureAtlasLayout},
    math::{UVec2, Vec3},
    sprite::Anchor,
    transform::components::Transform,
    utils::default,
};
use bevy_text_mode::{TextModeSprite, TextModeSpriteBundle};

use crate::{
    Tilesets, components::{position::Position, renderable::{NeedsSprite, Renderable}}, map::renderer::{TILE_PIXEL_DISPLAY_SIZE, TILE_PIXEL_SIZE},
};

pub fn spawn_entity_sprites(
    mut commands: Commands,
    tilesets: Res<Tilesets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut query: Query<(Entity, &Renderable, &Position, &NeedsSprite)>,
) {
    let tileset: Handle<Image> = tilesets.sprite.clone();
    let layout =
        TextureAtlasLayout::from_grid(UVec2::splat(TILE_PIXEL_SIZE as u32), 16, 16, None, None);
    let handle = texture_atlas_layouts.add(layout);
    let sprite_scale = (TILE_PIXEL_DISPLAY_SIZE / TILE_PIXEL_SIZE) as f32;

    for (entity, renderable, position, _) in query.iter_mut() {
        println!("ADDING SPRITE !");
        commands
            .entity(entity)
            .insert(TextModeSpriteBundle {
                sprite: TextModeSprite {
                    bg: renderable.bg.clone().into(),
                    fg: renderable.fg.clone().into(),
                    anchor: Anchor::BOTTOM_LEFT,
                    image: tileset.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: handle.clone(),
                        index: renderable.tilemap_index as usize,
                    }),
                    ..default()
                },
                transform: Transform {
                    translation: Vec3 {
                        x: (position.0.x * TILE_PIXEL_DISPLAY_SIZE) as f32,
                        y: (position.0.y * TILE_PIXEL_DISPLAY_SIZE) as f32,
                        z: 1.0,
                    },
                    scale: Vec3 {
                        x: sprite_scale,
                        y: sprite_scale,
                        z: 1.0,
                    },
                    ..default()
                },
                ..default()
            })
            .remove::<NeedsSprite>();
    }
}

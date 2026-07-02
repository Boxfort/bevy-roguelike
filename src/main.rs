use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    platform::collections::HashMap,
    prelude::*,
};
use bevy_text_mode::TextModePlugin;
use chacha20::ChaCha8Rng;
use rand::SeedableRng;

use crate::{
    components::{
        position::Position,
        renderable::{GlyphColor, NeedsSprite, Renderable},
    }, map::renderer::{MapData, generate_chunk_data, load_chunks, render_map_chunks}, player::player::{Player, player_input, try_move_player}, systems::{camera::{CameraTarget, move_camera_to_target}, entity_renderer::spawn_entity_sprites},
};

mod components;
mod map;
mod player;
mod systems;

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(TextModePlugin)
        .insert_resource(ClearColor(Color::BLACK))
        .add_systems(Startup, (load_tileset, setup, generate_chunk_data).chain())
        .add_systems(
            Update,
            (
                load_chunks,
                render_map_chunks,
                spawn_entity_sprites,
                player_input,
                move_camera_to_target,
                try_move_player
            )
                .chain(),
        );

    app.run();
}

#[derive(Resource, Deref, DerefMut)]
struct SeededRng(ChaCha8Rng);

#[derive(Resource)]
struct Tilesets {
    sprite: Handle<Image>,
    tilemap: Handle<Image>,
}

fn load_tileset(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // decoded once, synchronously; no runtime file needed
    let bytes = include_bytes!("../assets/Anikki_square_8x8.png");
    let base = Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .unwrap();

    let sprite = images.add(base.clone()); // D2
    let array = base.create_stacked_array_from_2d_grid(16, 16).unwrap(); // (rows, columns)
    let tilemap = images.add(array); // D2Array

    commands.insert_resource(Tilesets { sprite, tilemap });
}

fn setup(mut commands: Commands) {
    // We're seeding the PRNG here to make this example deterministic for testing purposes.
    // This isn't strictly required in practical use unless you need your app to be deterministic.
    let rng = ChaCha8Rng::seed_from_u64(42);

    commands.spawn(Camera2d);
    commands.insert_resource(CameraTarget(Vec2 {x: 0.0, y: 0.0}));

    commands.insert_resource(SeededRng(rng));
    commands.insert_resource(MapData {
        loaded_chunks: vec![],
        chunk_data: HashMap::new(),
    });

    commands.spawn((
        Player {},
        Position { x: 1, y: 1 },
        Renderable {
            tilemap_index: (16 * 5) + 7,
            fg: GlyphColor::GREEN,
            bg: GlyphColor::BLACK,
        },
        NeedsSprite {},
    ));

    commands.spawn((
        Position { x: 5, y: 5 },
        Renderable {
            tilemap_index: (16 * 6) + 7,
            fg: GlyphColor::GREEN,
            bg: GlyphColor::BLACK,
        },
        NeedsSprite {},
    ));
}

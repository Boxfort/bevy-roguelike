use bevy::{asset::RenderAssetUsages, image::{CompressedImageFormats, ImageSampler, ImageType}, platform::collections::HashMap, prelude::*, sprite::Anchor};
use bevy_text_mode::{TextModePlugin, TextModeSprite, TextModeSpriteBundle};
use chacha20::ChaCha8Rng;
use rand::SeedableRng;

use crate::{
    map::renderer::{MapData, TILE_PIXEL_DISPLAY_SIZE, TILE_PIXEL_SIZE, generate_chunk_data, load_chunks, render_map_chunks}, systems::camera::move_camera,
};

mod components;
mod map;
mod systems;

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(TextModePlugin)
        .insert_resource(ClearColor(Color::BLACK))
        .add_systems(Startup, (load_tileset, setup, generate_chunk_data).chain())
        .add_systems(
            Update,
            (move_camera, load_chunks, render_map_chunks).chain(),
        );

    app.run();
}

#[derive(Resource, Deref, DerefMut)]
struct SeededRng(ChaCha8Rng);


#[derive(Resource)]
struct Tilesets {
    sprite: Handle<Image>,
    tilemap: Handle<Image>
}

fn load_tileset(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    server: Res<AssetServer>,
)
{
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

    let sprite= images.add(base.clone());                                 // D2
    let array = base.create_stacked_array_from_2d_grid(16, 16).unwrap();   // (rows, columns)
    let tilemap = images.add(array);                                       // D2Array

    commands.insert_resource(Tilesets {  sprite, tilemap });    
}

fn setup(
    mut commands: Commands,
    tilesets: Res<Tilesets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    // We're seeding the PRNG here to make this example deterministic for testing purposes.
    // This isn't strictly required in practical use unless you need your app to be deterministic.
    let rng = ChaCha8Rng::seed_from_u64(42);

    commands.spawn(Camera2d);

    commands.insert_resource(SeededRng(rng));
    commands.insert_resource(MapData {
        loaded_chunks: vec![],
        chunk_data: HashMap::new(),
    });

    let tileset: Handle<Image> = tilesets.sprite.clone();
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(TILE_PIXEL_SIZE as u32), 16, 16, None, None);
    let handle = texture_atlas_layouts.add(layout);
    let sprite_scale = (TILE_PIXEL_DISPLAY_SIZE/TILE_PIXEL_SIZE) as f32;

    commands.spawn(TextModeSpriteBundle {
        sprite: TextModeSprite {
            bg: GlyphColor::WHITE.into(),
            fg: GlyphColor::BLACK.into(),
            anchor: Anchor::TOP_LEFT,
            image: tileset.clone(),
            texture_atlas: Some(TextureAtlas {
                layout: handle.clone(),
                index: 1,
            }),
            ..default()
        },
        transform: Transform {
            translation: Vec3{x: 40.0, y: 25.0, z: 1.0},
            scale: Vec3 {x: sprite_scale, y: sprite_scale, z: 1.0},
            ..default()
        },
        ..default()
    });
}

enum GlyphColor {
    BLACK,
    WHITE,
    BLUE,
    GREEN,
    ORANGE,
    PINK,
}

impl Into<LinearRgba> for GlyphColor {
    fn into(self) -> LinearRgba {
        match self {
            GlyphColor::WHITE => LinearRgba::from(Srgba::WHITE),
            GlyphColor::BLACK => LinearRgba::from(Srgba::BLACK),
            GlyphColor::BLUE => LinearRgba::from(Srgba::hex("a2fff3").unwrap()),
            GlyphColor::GREEN => LinearRgba::from(Srgba::hex("cbf382").unwrap()),
            GlyphColor::ORANGE => LinearRgba::from(Srgba::hex("ffcbba").unwrap()),
            GlyphColor::PINK => LinearRgba::from(Srgba::hex("e3b2ff").unwrap()),
        }
    }
}

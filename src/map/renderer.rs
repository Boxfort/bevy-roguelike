use std::ops::Div;

use bevy::{
    asset::{AssetServer, Handle},
    camera::Camera2d,
    color::Color,
    ecs::{
        component::Component,
        entity::Entity,
        query::With,
        resource::Resource,
        system::{Commands, Query, Res, ResMut, Single},
    },
    image::{Image, ImageArrayLayout, ImageLoaderSettings},
    math::{IVec2, UVec2, Vec3Swizzles},
    platform::collections::HashMap,
    sprite_render::{TileData, TilemapChunk, TilemapChunkTileData},
    transform::components::Transform,
    utils::default,
};
use chacha20::ChaCha8Rng;
use rand::RngExt;

use crate::{SeededRng, Tilesets};

#[derive(Component)]
pub struct ChunkPosition {
    pub pos: IVec2,
}

#[derive(Resource)]
pub struct MapData {
    pub loaded_chunks: Vec<IVec2>,
    pub chunk_data: HashMap<IVec2, ChunkData>,
}

#[derive(Clone)]
pub struct ChunkData {
    tiles: Vec<TileType>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum TileType {
    Wall,
    Floor,
}

pub const TILE_PIXEL_SIZE: i32 = 8;
pub const TILE_PIXEL_DISPLAY_SIZE: i32 = 16;
const CHUNK_SIZE: i32 = 20;
const MAP_SIZE_X: i32 = 160;
const MAP_SIZE_Y: i32 = 100;

pub fn xy_idx(x: i32, y: i32) -> usize {
    (y as usize * MAP_SIZE_X as usize) + x as usize
}

fn dummy_chunk_data(rng: &mut ChaCha8Rng) -> HashMap<IVec2, ChunkData> {
    let mut map = vec![TileType::Floor; (MAP_SIZE_X * MAP_SIZE_Y) as usize];

    // Make the boundaries walls
    for x in 0..MAP_SIZE_X {
        map[xy_idx(x, 0)] = TileType::Wall;
        map[xy_idx(x, MAP_SIZE_Y - 1)] = TileType::Wall;
    }
    for y in 0..MAP_SIZE_Y {
        map[xy_idx(0, y)] = TileType::Wall;
        map[xy_idx(MAP_SIZE_X - 1, y)] = TileType::Wall;
    }

    // Now we'll randomly splat 400 walls. It won't be pretty, but it's a decent illustration.
    for _i in 0..400 {
        let x = rng.random_range(1..(MAP_SIZE_X - 1));
        let y = rng.random_range(1..(MAP_SIZE_Y - 1));
        let idx = xy_idx(x, y);
        if idx != xy_idx(40, 25) {
            map[idx] = TileType::Wall;
        }
    }

    let mut chunks: HashMap<IVec2, ChunkData> = HashMap::new();

    for chunk_x in 0..(MAP_SIZE_X / CHUNK_SIZE) {
        for chunk_y in 0..(MAP_SIZE_Y / CHUNK_SIZE) {
            let mut tiles_in_chunk: Vec<TileType> = vec![];

            for curr_y in 0..CHUNK_SIZE {
                for curr_x in 0..CHUNK_SIZE {
                    let world_x = (chunk_x * CHUNK_SIZE) + curr_x;
                    let world_y = (chunk_y * CHUNK_SIZE) + curr_y;
                    let idx = (world_y * MAP_SIZE_X) + world_x;
                    tiles_in_chunk.push(map[idx as usize]);
                }
            }

            let chunk_data = ChunkData {
                tiles: tiles_in_chunk,
            };
            chunks.insert(
                IVec2 {
                    x: chunk_x,
                    y: chunk_y,
                },
                chunk_data,
            );
        }
    }

    chunks
}

pub fn generate_chunk_data(mut map_data: ResMut<MapData>, mut rng: ResMut<SeededRng>) {
    map_data.chunk_data = dummy_chunk_data(&mut rng);
}

pub fn load_chunks(
    mut cmd: Commands,
    mut map_data: ResMut<MapData>,
    assets: Res<AssetServer>,
    mut query: Query<(Entity, &TilemapChunkTileData, &ChunkPosition)>,
    tilesets: Res<Tilesets>,
    camera: Single<&mut Transform, With<Camera2d>>, // camera currently used as a standin for player pos
) {
    let camera_pos: IVec2 = camera.translation.xy().as_ivec2() / TILE_PIXEL_DISPLAY_SIZE;

    // Clamped to the bounds of our chunks (TODO: this would need to change)
    // TODO: generate chunks when necessary
    let chunk_coordinate = camera_pos.div(CHUNK_SIZE).clamp(
        IVec2 { x: 0, y: 0 },
        IVec2 {
            x: (MAP_SIZE_X / CHUNK_SIZE) - 1,
            y: (MAP_SIZE_Y / CHUNK_SIZE) - 1,
        },
    );

    println!("CAM POS: {} CHUNK COORD {}", camera_pos, chunk_coordinate);

    let mut chunks_to_load: Vec<IVec2> = get_adjacent_chunk_positions(chunk_coordinate, 1);

    println!("TO LOAD {:?}", chunks_to_load);

    for (entity, _tile_data, chunk_position) in query.iter_mut() {
        match chunks_to_load.iter().position(|x| *x == chunk_position.pos) {
            Some(idx) => {
                chunks_to_load.remove(idx);
            }
            None => {
                if chunk_coordinate.chebyshev_distance(chunk_position.pos) > 2 {
                    match map_data
                        .loaded_chunks
                        .iter()
                        .position(|x| *x == chunk_position.pos)
                    {
                        Some(idx) => {
                            map_data.loaded_chunks.remove(idx);
                        }
                        None => {}
                    }
                    cmd.entity(entity).despawn(); // TODO: mark for despawn instead?
                }
            }
        }
    }

    println!("ACTUAL TO LOAD {:?}", chunks_to_load);

    for chunk_pos in chunks_to_load {
        if chunk_pos.x >= (MAP_SIZE_X / CHUNK_SIZE) {
            continue;
        };
        if chunk_pos.x < 0 {
            continue;
        };
        if chunk_pos.y >= (MAP_SIZE_Y / CHUNK_SIZE) {
            continue;
        };
        if chunk_pos.y < 0 {
            continue;
        };

        map_data.loaded_chunks.push(chunk_pos);
        create_tilemap_chunk(&mut cmd, chunk_pos, &assets, &tilesets);
    }
}

fn get_adjacent_chunk_positions(chunk_coordinate: IVec2, distance: i32) -> Vec<IVec2> {
    let mut adjacent_positions: Vec<IVec2> = vec![];

    for x in -distance..=distance {
        for y in -distance..=distance {
            adjacent_positions.push(chunk_coordinate + IVec2 { x, y });
        }
    }

    adjacent_positions
}

/// Spawn a completely blank tilemap chunk
fn create_tilemap_chunk(
    cmd: &mut Commands,
    chunk_pos: IVec2,
    assets: &Res<AssetServer>,
    tilesets: &Res<Tilesets>,
) {
    let chunk_size = UVec2::splat(CHUNK_SIZE as u32);
    let tile_display_size = UVec2::splat(TILE_PIXEL_DISPLAY_SIZE as u32);
    let mut tile_data = vec![None; (CHUNK_SIZE * CHUNK_SIZE) as usize];

    let tileset: Handle<Image> = tilesets.tilemap.clone();

    cmd.spawn((
        TilemapChunk {
            chunk_size,
            tile_display_size,
            tileset: tileset,
            ..default()
        },
        TilemapChunkTileData(tile_data),
        ChunkPosition { pos: chunk_pos },
        Transform::from_xyz(
            (chunk_pos.x * CHUNK_SIZE * TILE_PIXEL_SIZE) as f32,
            (chunk_pos.y * CHUNK_SIZE * TILE_PIXEL_SIZE) as f32,
            0.0,
        ),
    ));
}

/*

This is how the chunks are oriented.

y
|______ ______ (CHUNK_SIZE*2, CHUNK_SIZE*2)
|      |      |
| 0, 1 | 1, 1 |
|______|______|
|      |      |
| 0, 0 | 1, 0 |
|______|______|_ x
^
|
origin (0,0)

*/
pub fn render_map_chunks(
    map_data: Res<MapData>,
    mut query: Query<(&mut TilemapChunkTileData, &ChunkPosition)>,
) {
    for (mut tile_data, chunk_position) in query.iter_mut() {
        let tiles_in_chunk = map_data.chunk_data.get(&chunk_position.pos);

        match tiles_in_chunk {
            Some(tiles) => {
                for idx in 0..(CHUNK_SIZE * CHUNK_SIZE) {
                    let tileset_idx = match (chunk_position.pos, idx, tiles.tiles[idx as usize]) {
                        (IVec2 { x: 0, y: 0 }, 0, _) => 4,
                        (_, _, TileType::Wall) => 1,
                        (_, _, TileType::Floor) => 0,
                    };

                    tile_data[idx as usize] = Some(TileData {
                        tileset_index: tileset_idx,
                        color: Color::linear_rgb(1.0, 1.0, 1.0),
                        ..default()
                    });
                }
            }
            None => (),
        }
    }
}

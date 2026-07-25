use bevy::{
    math::IVec2,
    platform::collections::{HashMap, HashSet},
    sprite_render::TileData,
};
use chacha20::ChaCha8Rng;
use rand::{Rng, RngExt, SeedableRng};

use crate::map::renderer::CHUNK_SIZE;

const OVERMAP_CHUNK_SIZE: i32 = 128;

struct GameMap {
    overmap_chunks: HashMap<OvermapChunkCoords, OvermapChunk>,
}

#[derive(Eq, Hash, PartialEq, Clone, Copy)]
struct OvermapChunkCoords(IVec2);

#[derive(Clone, Copy, PartialEq)]
pub enum OvermapTileType {
    Road,
    House,
}

struct OvermapChunk {
    coordinates: OvermapChunkCoords,
    cities: HashMap<CityId, City>,
    overmap_tiles: Vec<Option<OvermapTileType>>,
}

impl OvermapChunk {
    pub fn new(coordinates: OvermapChunkCoords) -> OvermapChunk {
        OvermapChunk {
            coordinates,
            cities: HashMap::new(),
            overmap_tiles: vec![None; (OVERMAP_CHUNK_SIZE * OVERMAP_CHUNK_SIZE) as usize],
        }
    }
}

#[derive(Eq, Hash, PartialEq, Clone)]
struct CityId(i32);

#[derive(Eq, Hash, PartialEq, Clone)]
struct City {
    id: CityId,
    position: IVec2,
    overmap_chunk_coordinate: OvermapChunkCoords,
    size: i32, // The estimated size of the city, used for influencing the procgen
    connected_to: Vec<CityId>,
}

struct Building {
    position: IVec2,
    bounds: IVec2,
}

pub fn generate_map(current_overmap_coordinate: IVec2) {
    let mut game_map = GameMap {
        overmap_chunks: HashMap::new(),
    };

    let neighbouring_chunk_positions =
        get_neighbouring_overmap_chunk_positions(current_overmap_coordinate);
    let mut rng = ChaCha8Rng::seed_from_u64(42); // TODO: remove and use the resource

    let cities: Vec<City> = vec![];

    for chunk_coordinate in neighbouring_chunk_positions {
        let mut overmap_chunk = OvermapChunk::new(chunk_coordinate);

        if rng.random_range(0..=1) == 1 {
            // Add a city somewhere
            let new_city_position = IVec2 {
                x: rng.random_range(0..OVERMAP_CHUNK_SIZE),
                y: rng.random_range(0..OVERMAP_CHUNK_SIZE),
            };

            let mut city = City {
                id: CityId(cities.len() as i32),
                position: new_city_position,
                overmap_chunk_coordinate: chunk_coordinate,
                size: 64,
                connected_to: vec![],
            };

            // Add connecting road to adjacent cities.
            let adjacent_chunks =
                get_potential_neighbouring_city_overmap_chunks(chunk_coordinate.0);

            for adjacent_chunk_coords in adjacent_chunks {
                let neighbouring_cities = game_map.overmap_chunks[&adjacent_chunk_coords]
                    .cities
                    .clone();

                if !neighbouring_cities.is_empty() {
                    for neighbouring_city in neighbouring_cities {
                        if !city.connected_to.contains(&neighbouring_city.0) {
                            connect_cities_with_road(
                                &mut game_map.overmap_chunks,
                                new_city_position,
                                overmap_chunk.coordinates,
                                neighbouring_city.1.position,
                                adjacent_chunk_coords,
                            );
                            city.connected_to.push(neighbouring_city.0.clone());
                            let a = game_map
                                .overmap_chunks
                                .get_mut(&adjacent_chunk_coords)
                                .unwrap();
                            a.cities
                                .get_mut(&neighbouring_city.0)
                                .unwrap()
                                .connected_to
                                .push(city.id.clone());
                        }
                    }
                }
            }

            overmap_chunk.cities.insert(city.id.clone(), city);
        }

        game_map
            .overmap_chunks
            .insert(chunk_coordinate, overmap_chunk);
    }
}

fn connect_cities_with_road(
    overmap_chunks: &mut HashMap<OvermapChunkCoords, OvermapChunk>,
    start_city_pos: IVec2,
    start_city_chunk: OvermapChunkCoords,
    end_city_pos: IVec2,
    end_city_chunk: OvermapChunkCoords,
) {
    draw_horizontal_road_between_cities(
        overmap_chunks,
        start_city_pos,
        start_city_chunk,
        end_city_pos,
        end_city_chunk,
    );
    draw_vertical_road_between_cities(
        overmap_chunks,
        start_city_pos,
        start_city_chunk,
        end_city_pos,
        end_city_chunk,
    );
}

struct ChunkCursor {
    chunk_coord: OvermapChunkCoords,
    local_pos: IVec2,
}

impl ChunkCursor {
    fn step_x(&mut self) {
        self.local_pos.x += 1;
        if self.local_pos.x >= OVERMAP_CHUNK_SIZE {
            self.local_pos.x = 0;
            self.chunk_coord.0.x += 1;
        }
    }

    fn step_y(&mut self) {
        self.local_pos.y += 1;
        if self.local_pos.y >= OVERMAP_CHUNK_SIZE {
            self.local_pos.y = 0;
            self.chunk_coord.0.y += 1;
        }
    }

    fn set_tile(&self, overmap_chunk: &mut OvermapChunk, tile_type: OvermapTileType) {
        overmap_chunk.overmap_tiles[xy_idx(self.local_pos.x, self.local_pos.y)] = Some(tile_type);
    }
}

fn draw_vertical_road_between_cities(
    overmap_chunks: &mut HashMap<OvermapChunkCoords, OvermapChunk>,
    start_city_pos: IVec2,
    start_city_chunk: OvermapChunkCoords,
    end_city_pos: IVec2,
    end_city_chunk: OvermapChunkCoords,
) {
    let chunk_coord_y_diff = start_city_chunk.0.y - end_city_chunk.0.y;
    let distance_between_cities = (start_city_pos - (end_city_pos * chunk_coord_y_diff)).abs();

    // Line is always drawn bottom to top, starting from the bottom-most chunk
    let mut current_chunk_coords = if chunk_coord_y_diff < 0 {
        end_city_chunk
    } else {
        start_city_chunk
    };
    let y_start = if chunk_coord_y_diff < 0 {
        end_city_pos.y
    } else {
        start_city_pos.y
    };

    let mut cursor = ChunkCursor {
        chunk_coord: current_chunk_coords,
        local_pos: IVec2 {
            x: start_city_pos.x,
            y: y_start,
        },
    };

    for _ in 0..distance_between_cities.y {
        let overmap_chunk = overmap_chunks.get_mut(&current_chunk_coords).unwrap();
        cursor.set_tile(overmap_chunk, OvermapTileType::Road);
        cursor.step_y();
    }
}

fn draw_horizontal_road_between_cities(
    overmap_chunks: &mut HashMap<OvermapChunkCoords, OvermapChunk>,
    start_city_pos: IVec2,
    start_city_chunk: OvermapChunkCoords,
    end_city_pos: IVec2,
    end_city_chunk: OvermapChunkCoords,
) {
    let chunk_coord_x_diff = start_city_chunk.0.x - end_city_chunk.0.x;
    let distance_between_cities = (start_city_pos - (end_city_pos * chunk_coord_x_diff)).abs();

    // Line is always drawn left to right, starting from the leftmost chunk
    let mut current_chunk_coords = if chunk_coord_x_diff < 0 {
        end_city_chunk
    } else {
        start_city_chunk
    };
    let x_start = if chunk_coord_x_diff < 0 {
        end_city_pos.x
    } else {
        start_city_pos.x
    };

    let mut cursor = ChunkCursor {
        chunk_coord: current_chunk_coords,
        local_pos: IVec2 {
            x: x_start,
            y: start_city_pos.x,
        },
    };

    for _ in 0..distance_between_cities.x {
        let overmap_chunk = overmap_chunks.get_mut(&current_chunk_coords).unwrap();
        cursor.set_tile(overmap_chunk, OvermapTileType::Road);
        cursor.step_y();
    }
}

pub fn xy_idx(x: i32, y: i32) -> usize {
    (y as usize * OVERMAP_CHUNK_SIZE as usize) + x as usize
}

fn get_potential_neighbouring_city_overmap_chunks(
    chunk_coordinate: IVec2,
) -> Vec<OvermapChunkCoords> {
    let mut adjacent_positions: Vec<OvermapChunkCoords> = vec![];

    adjacent_positions.push(OvermapChunkCoords(chunk_coordinate + IVec2 { x: 1, y: 0 }));
    adjacent_positions.push(OvermapChunkCoords(chunk_coordinate + IVec2 { x: -1, y: 0 }));
    adjacent_positions.push(OvermapChunkCoords(chunk_coordinate + IVec2 { x: 0, y: 1 }));
    adjacent_positions.push(OvermapChunkCoords(chunk_coordinate + IVec2 { x: 0, y: -1 }));

    adjacent_positions
}

fn get_neighbouring_overmap_chunk_positions(chunk_coordinate: IVec2) -> Vec<OvermapChunkCoords> {
    let mut adjacent_positions: Vec<OvermapChunkCoords> = vec![];

    for x in -2..=2 {
        for y in -2..=2 {
            adjacent_positions.push(OvermapChunkCoords(chunk_coordinate + IVec2 { x, y }));
        }
    }

    adjacent_positions
}

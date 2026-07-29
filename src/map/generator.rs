use std::time::{SystemTime, UNIX_EPOCH};

use bevy::{
    math::IVec2,
    platform::collections::{HashMap, HashSet},
    sprite_render::TileData,
};
use chacha20::ChaCha8Rng;
use rand::{Rng, RngExt, SeedableRng};

use crate::map::renderer::CHUNK_SIZE;

const OVERMAP_CHUNK_SIZE: i32 = 128;

#[derive(Debug)]
pub struct GameMap {
    pub overmap_chunks: HashMap<OvermapChunkCoords, OvermapChunk>,
}

#[derive(Debug, Eq, Hash, PartialEq, Clone, Copy)]
pub struct OvermapChunkCoords(pub IVec2);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OvermapTileType {
    Road,
    House,
}

#[derive(Debug)]
pub struct OvermapChunk {
    pub coordinates: OvermapChunkCoords,
    pub cities: HashMap<CityId, City>,
    pub overmap_tiles: Vec<Option<OvermapTileType>>,
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

#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub struct CityId(i32);

#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub struct City {
    id: CityId,
    position: IVec2,
    overmap_chunk_coordinate: OvermapChunkCoords,
    size: i32, // The estimated size of the city, used for influencing the procgen
    connected_to: Vec<(OvermapChunkCoords, CityId)>,
}

pub struct Building {
    position: IVec2,
    bounds: IVec2,
}

pub fn generate_overmap_chunk(
    game_map: &mut GameMap,
    current_overmap_coordinate: IVec2,
    rng: &mut ChaCha8Rng,
) {
    // Create any surrounding chunks which don't already exist
    for overmap_coord in get_coords_in_square_radius(current_overmap_coordinate, 2, false) {
        let chunk_coordinate = OvermapChunkCoords(overmap_coord);
        if !game_map.overmap_chunks.contains_key(&chunk_coordinate) {
            let mut overmap_chunk = OvermapChunk::new(chunk_coordinate);
            add_cities_to_chunk(&mut overmap_chunk, rng);
            game_map
                .overmap_chunks
                .insert(chunk_coordinate, overmap_chunk);
        }
    }

    for overmap_coord in get_coords_in_square_radius(current_overmap_coordinate, 1, false) {
        // Get adjacent tiles for city connection
        let current_chunk_coordinate = OvermapChunkCoords(overmap_coord);
        let current_chunk_cities: HashMap<CityId, City> = game_map
            .overmap_chunks
            .get(&current_chunk_coordinate)
            .unwrap()
            .cities
            .clone();

        let coordinates_to_check = get_neighbouring_city_connection_coordinates(overmap_coord);
        for neighbouring_coord in coordinates_to_check {
            let neighbouring_chunk_coordinate = OvermapChunkCoords(neighbouring_coord);
            let neighbouring_chunk_cities: HashMap<CityId, City> = game_map
                .overmap_chunks
                .get(&neighbouring_chunk_coordinate)
                .unwrap()
                .cities
                .clone();

            for city_a in &current_chunk_cities {
                for city_b in &neighbouring_chunk_cities {
                    if !city_a
                        .1
                        .connected_to
                        .contains(&(neighbouring_chunk_coordinate, city_b.0.clone()))
                    {
                        connect_cities_with_road(
                            &mut game_map.overmap_chunks,
                            city_a.1.position,
                            city_a.1.overmap_chunk_coordinate,
                            city_b.1.position,
                            city_b.1.overmap_chunk_coordinate,
                        );

                        let [current_chunk, neighbouring_chunk] = game_map
                            .overmap_chunks
                            .get_disjoint_mut([
                                &current_chunk_coordinate,
                                &neighbouring_chunk_coordinate,
                            ])
                            .map(|x| x.unwrap());

                        // Connect city_a to city_b
                        current_chunk
                            .cities
                            .get_mut(&city_a.0.clone())
                            .unwrap()
                            .connected_to
                            .push((neighbouring_chunk_coordinate, city_b.0.clone()));

                        // Connect city_b to city_a
                        neighbouring_chunk
                            .cities
                            .get_mut(&city_b.0.clone())
                            .unwrap()
                            .connected_to
                            .push((current_chunk_coordinate, city_a.0.clone()));
                    }
                }
            }
        }
    }
}

fn add_cities_to_chunk(overmap_chunk: &mut OvermapChunk, rng: &mut ChaCha8Rng) {
    if rng.random_range(0..=1) == 1 {
        // Add a city somewhere
        let new_city_position = IVec2 {
            x: rng.random_range(0..OVERMAP_CHUNK_SIZE),
            y: rng.random_range(0..OVERMAP_CHUNK_SIZE),
        };

        let city = City {
            id: CityId(0), // TODO: when/if we decide to add multiple cities then this will need to be set
            position: new_city_position,
            overmap_chunk_coordinate: overmap_chunk.coordinates,
            size: 64,
            connected_to: vec![],
        };

        overmap_chunk.cities.insert(CityId(0), city);
    }
}

fn get_coords_in_square_radius(from_coord: IVec2, radius: i32, border_only: bool) -> Vec<IVec2> {
    let mut coords: Vec<IVec2> = vec![];

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if border_only && (dx.abs() != radius && dy.abs() != radius) {
                continue;
            }

            let nx = from_coord.x + dx;
            let ny = from_coord.y + dy;

            coords.push(IVec2 { x: nx, y: ny })
        }
    }

    coords
}

fn connect_cities_with_road(
    overmap_chunks: &mut HashMap<OvermapChunkCoords, OvermapChunk>,
    start_city_pos: IVec2,
    start_city_chunk: OvermapChunkCoords,
    end_city_pos: IVec2,
    end_city_chunk: OvermapChunkCoords,
) {
    let mut cursor = ChunkCursor {
        chunk_coord: start_city_chunk,
        local_pos: IVec2 {
            x: start_city_pos.x,
            y: start_city_pos.y,
        },
    };

    let distance_between_cities = ((start_city_pos + (start_city_chunk.0 * OVERMAP_CHUNK_SIZE))
        - (end_city_pos + (end_city_chunk.0 * OVERMAP_CHUNK_SIZE)))
        .abs();

    let chunk_dir =
        (end_city_chunk.0 - start_city_chunk.0).clamp(IVec2::splat(-1), IVec2::splat(1));
    let x_direction = if chunk_dir.x != 0 {
        chunk_dir.x
    } else {
        (end_city_pos.x - start_city_pos.x).clamp(-1, 1)
    };
    let y_direction = if chunk_dir.y != 0 {
        chunk_dir.y
    } else {
        (end_city_pos.y - start_city_pos.y).clamp(-1, 1)
    };
    let direction = IVec2 {
        x: x_direction,
        y: y_direction,
    };

    let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();
    cursor.set_tile(overmap_chunk, OvermapTileType::House);

    println!("DISTANCE {:?} DIR {:?}", distance_between_cities, direction);

    println!(
        "FROM {:?} {:?} TO {:?} {:?}",
        start_city_chunk, start_city_pos, end_city_chunk, end_city_pos
    );

    println!("CURSOR {:?} {:?}", cursor.chunk_coord, cursor.local_pos);

    for _ in 0..distance_between_cities.x {
        cursor.step_x(direction.x);
        let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();
        cursor.set_tile(overmap_chunk, OvermapTileType::Road);
    }

    println!("TURNING AT {:?} {:?}", cursor.chunk_coord, cursor.local_pos);

    for _ in 0..distance_between_cities.y {
        cursor.step_y(direction.y);
        let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();
        cursor.set_tile(overmap_chunk, OvermapTileType::Road);
    }

    let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();
    println!("ENDING AT {:?} {:?}", cursor.chunk_coord, cursor.local_pos);

    cursor.set_tile(overmap_chunk, OvermapTileType::House);
}

struct ChunkCursor {
    chunk_coord: OvermapChunkCoords,
    local_pos: IVec2,
}

impl ChunkCursor {
    fn step_x(&mut self, delta: i32) {
        self.local_pos.x += delta;
        if delta > 0 {
            if self.local_pos.x >= OVERMAP_CHUNK_SIZE {
                self.local_pos.x = 0;
                self.chunk_coord.0.x += 1;
            }
        } else {
            if self.local_pos.x < 0 {
                self.local_pos.x = OVERMAP_CHUNK_SIZE - 1;
                self.chunk_coord.0.x -= 1;
            }
        }
    }

    fn step_y(&mut self, delta: i32) {
        self.local_pos.y += delta;
        if delta > 0 {
            if self.local_pos.y >= OVERMAP_CHUNK_SIZE {
                self.local_pos.y = 0;
                self.chunk_coord.0.y += 1;
            }
        } else {
            if self.local_pos.y < 0 {
                self.local_pos.y = OVERMAP_CHUNK_SIZE - 1;
                self.chunk_coord.0.y -= 1;
            }
        }
    }

    fn set_tile(&self, overmap_chunk: &mut OvermapChunk, tile_type: OvermapTileType) {
        if overmap_chunk.overmap_tiles[xy_idx(self.local_pos.x, self.local_pos.y)]
            != Some(OvermapTileType::House)
        {
            overmap_chunk.overmap_tiles[xy_idx(self.local_pos.x, self.local_pos.y)] =
                Some(tile_type);
        }
    }
}

pub fn xy_idx(x: i32, y: i32) -> usize {
    (y as usize * OVERMAP_CHUNK_SIZE as usize) + x as usize
}

fn get_neighbouring_city_connection_coordinates(from_coord: IVec2) -> Vec<IVec2> {
    let mut coordinates: Vec<IVec2> = vec![];

    coordinates.push(from_coord + IVec2 { x: 1, y: 0 });
    coordinates.push(from_coord + IVec2 { x: -1, y: 0 });
    coordinates.push(from_coord + IVec2 { x: 0, y: 1 });
    coordinates.push(from_coord + IVec2 { x: 0, y: -1 });
    coordinates
}

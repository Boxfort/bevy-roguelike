use bevy::{
    math::{IVec2, Vec2Swizzles},
    platform::collections::HashMap,
};
use chacha20::ChaCha8Rng;
use rand::RngExt;

use crate::{
    map::generator::{OvermapTileType::Road, RoadBuilderAction::*},
    utils::{get_coords_in_square_radius, get_neighbouring_cardinal_coordinates, xy_idx},
};

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
    pub generation_complete: bool,
}

impl OvermapChunk {
    pub fn new(coordinates: OvermapChunkCoords) -> OvermapChunk {
        OvermapChunk {
            coordinates,
            cities: HashMap::new(),
            overmap_tiles: vec![None; (OVERMAP_CHUNK_SIZE * OVERMAP_CHUNK_SIZE) as usize],
            generation_complete: false,
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

// NOTES
// Road placement:
// Kick off 4 'Builders' from the center of the city
// each tick they have a chance to: stop and/or split left, right, or both.
// as they get further away from the center of the city relative to the city size
//   the chance for the builder to stop increases, and the split chance decreases.
// Building placement:
// When placing roads place a marker beside the road to show a potential building location
// After all roads are placed try expanding each building location
// If placing a building then remove other markers it crosses

pub fn generate_overmap_chunk(
    game_map: &mut GameMap,
    current_overmap_coordinate: IVec2,
    rng: &mut ChaCha8Rng,
) {
    // Create any surrounding chunks which don't already exist, add add cities to them.
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

    // Connect up cities between neighbouring chunks
    for overmap_coord in get_coords_in_square_radius(current_overmap_coordinate, 1, false) {
        let current_chunk_coordinate = OvermapChunkCoords(overmap_coord);
        let current_chunk_cities: HashMap<CityId, City> = game_map
            .overmap_chunks
            .get(&current_chunk_coordinate)
            .unwrap()
            .cities
            .clone();

        let coordinates_to_check = get_neighbouring_cardinal_coordinates(overmap_coord);
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
                        connect_cities_with_road(game_map, city_a.1, city_b.1);
                    }
                }
            }
        }
    }

    // Do city road network generation
    for overmap_coord in get_coords_in_square_radius(current_overmap_coordinate, 1, false) {
        let current_overmap_chunk_coord = OvermapChunkCoords(overmap_coord);
        let current_chunk_cities: Vec<City> = game_map
            .overmap_chunks
            .get(&current_overmap_chunk_coord)
            .unwrap()
            .cities
            .iter()
            .map(|c| c.1.clone())
            .collect();

        for city in current_chunk_cities {
            // Kick off 4 'Builders' from the center of the city
            // each tick they have a chance to: stop and/or split left, right, or both.
            // as they get further away from the center of the city relative to the city size
            //   the chance for the builder to stop increases, and the split chance decreases.
            let mut road_builders: Vec<RoadBuilder> = vec![
                RoadBuilder::new(
                    current_overmap_chunk_coord,
                    city.position,
                    IVec2 { x: 0, y: 1 },
                    0,
                    0.0,
                    city.size,
                ),
                RoadBuilder::new(
                    current_overmap_chunk_coord,
                    city.position,
                    IVec2 { x: 0, y: -1 },
                    0,
                    0.0,
                    city.size,
                ),
                RoadBuilder::new(
                    current_overmap_chunk_coord,
                    city.position,
                    IVec2 { x: 1, y: 0 },
                    0,
                    0.0,
                    city.size,
                ),
                RoadBuilder::new(
                    current_overmap_chunk_coord,
                    city.position,
                    IVec2 { x: -1, y: 0 },
                    0,
                    0.0,
                    city.size,
                ),
            ];

            while road_builders.len() > 0 {
                run_road_builder_iteration(
                    &mut road_builders,
                    &city,
                    &mut game_map.overmap_chunks,
                    rng,
                )
            }
        }
    }
}

#[derive(Clone, Debug)]
enum RoadBuilderAction {
    Continue,
    SplitOnce,
    SplitOnceAndContinue,
    SplitTwice,
    SplitTwiceAndContinue,
}

struct RoadBuilder {
    chunk_cursor: ChunkCursor,
    direction: IVec2,
    generation: i32,
    stop_probability: i32,
    action_probabilities: Vec<(RoadBuilderAction, i32)>,
    cumulative_action_weights: i32,
    moves_since_split: i32,
}

// https://github.com/Boxfort/GodotProject/blob/master/Assets/Scenes/City/RoadBuilder.gd
impl RoadBuilder {
    const MOVES_BEFORE_SPLIT: i32 = 2;

    pub fn new(
        chunk_coordinate: OvermapChunkCoords,
        start_position: IVec2,
        direction: IVec2,
        generation: i32,
        distance_from_center: f32,
        city_size: i32,
    ) -> RoadBuilder {
        let t = distance_from_center / city_size as f32;
        RoadBuilder {
            chunk_cursor: ChunkCursor {
                chunk_coord: chunk_coordinate,
                local_pos: start_position,
            },
            direction: direction,
            generation: generation,
            stop_probability: (100.0 * t * t) as i32,
            /*
            stop_probability: (100.0
                * ((distance_from_center) / (city_size as f32)))
                as i32,
            */
            action_probabilities: vec![
                (Continue, 4),
                (SplitOnce, 1),
                (SplitOnceAndContinue, 2),
                (SplitTwice, 1),
                (SplitTwiceAndContinue, 2),
            ],
            cumulative_action_weights: 10,
            moves_since_split: 0,
        }
    }
}

fn run_road_builder_iteration(
    road_builders: &mut Vec<RoadBuilder>,
    city: &City,
    overmap_chunks: &mut HashMap<OvermapChunkCoords, OvermapChunk>,
    rng: &mut ChaCha8Rng,
) {
    let mut i = 0;
    'outer: while i < road_builders.len() {
        // move
        match road_builders[i].direction {
            IVec2 { x, y: _ } if x != 0 => road_builders[i].chunk_cursor.step_x(x),
            IVec2 { x: _, y } if y != 0 => road_builders[i].chunk_cursor.step_y(y),
            _ => (),
        }

        let current_tile = overmap_chunks
            .get_mut(&road_builders[i].chunk_cursor.chunk_coord)
            .unwrap()
            .overmap_tiles[xy_idx(
            road_builders[i].chunk_cursor.local_pos.x,
            road_builders[i].chunk_cursor.local_pos.y,
            OVERMAP_CHUNK_SIZE as usize,
        )];

        // Kill builder if moved onto another road
        if road_builders[i].generation > 0
            && let Some(tile) = current_tile
            && tile == Road
        {
            println!("KILLED BUILDER DUE TO TOUCHING ROAD, GEN: {}", road_builders[i].generation);
            road_builders.remove(i);
            continue 'outer;
        } else if road_builders[i].generation > 0 {
            // Kill builder if next to a road
            let perpendicular_directions = vec![
                road_builders[i].direction.yx(),
                road_builders[i].direction.yx() * -1,
            ];
            for dir in perpendicular_directions {
                if let Some(tile_type) =
                    road_builders[i].chunk_cursor.peek_tile(overmap_chunks, dir)
                    && tile_type == Road
                {
                println!("KILLED BUILDER DUE TO BEING BESIDE ROAD, GEN: {}", road_builders[i].generation);
                    road_builders.remove(i);
                    continue 'outer;
                }
            }
        }

        // set tile
        road_builders[i].chunk_cursor.set_tile(
            overmap_chunks
                .get_mut(&road_builders[i].chunk_cursor.chunk_coord)
                .unwrap(),
            Road,
        );

        // Kill builder if required
        let stop_roll = rng.random_range(0..100);
        if stop_roll < road_builders[i].stop_probability {
            println!("KILLED BUILDER DUE TO STOP ROLL");
            road_builders.remove(i);
            continue;
        }

        // calculate action
        if road_builders[i].moves_since_split < RoadBuilder::MOVES_BEFORE_SPLIT {
            road_builders[i].moves_since_split += 1;
            i += 1;
        } else {
            let action_roll = rng.random_range(0..road_builders[i].cumulative_action_weights);
            let mut cumulative = 0;
            println!("action roll {}", action_roll);
            for (action, weight) in road_builders[i].action_probabilities.clone() {
                cumulative += weight;
                println!("weight {}", weight);
                println!("cum {}", cumulative);
                println!("action {:?}", action);
                if action_roll < cumulative {
                    println!("SELECTED!");
                    match action {
                        Continue => {
                            println!("continue");
                            i += 1;
                        },
                        SplitOnce => {
                            println!("Split one");
                            let dir = rng.random_range(0..=1);
                            spawn_child_road_builder(road_builders, city, dir, i);
                            road_builders.remove(i);
                        }
                        SplitOnceAndContinue => {
                            println!("Split one and continue");
                            let dir = rng.random_range(0..=1);
                            spawn_child_road_builder(road_builders, city, dir, i);
                            road_builders[i].moves_since_split = 0;
                            i += 1
                        }
                        SplitTwice => {
                            println!("Split twice");
                            spawn_child_road_builder(road_builders, city, 0, i);
                            spawn_child_road_builder(road_builders, city, 1, i);
                            road_builders.remove(i);
                        }
                        SplitTwiceAndContinue => {
                            println!("Split twice and continue");
                            spawn_child_road_builder(road_builders, city, 0, i);
                            spawn_child_road_builder(road_builders, city, 1, i);
                            road_builders[i].moves_since_split = 0;
                            i += 1
                        }
                    }
                    break;
                }
            }
        }
    }
}

fn spawn_child_road_builder(
    road_builders: &mut Vec<RoadBuilder>,
    city: &City,
    dir: i32,
    idx: usize,
) {
    let road_builder = &road_builders[idx];
    road_builders.push(RoadBuilder::new(
        road_builder.chunk_cursor.chunk_coord,
        road_builder.chunk_cursor.local_pos,
        road_builder.direction.yx() * if dir > 0 { 1 } else { -1 },
        road_builder.generation + 1,
        ((OVERMAP_CHUNK_SIZE * road_builder.chunk_cursor.chunk_coord.0)
            + road_builder.chunk_cursor.local_pos)
            .manhattan_distance(
                (OVERMAP_CHUNK_SIZE * city.overmap_chunk_coordinate.0) + city.position,
            ) as f32,
        city.size,
    ));
}

fn connect_cities_with_road(game_map: &mut GameMap, city_a: &City, city_b: &City) {
    insert_road_tiles_between_cities(
        &mut game_map.overmap_chunks,
        city_a.position,
        city_a.overmap_chunk_coordinate,
        city_b.position,
        city_b.overmap_chunk_coordinate,
    );

    let [current_chunk, neighbouring_chunk] = game_map
        .overmap_chunks
        .get_disjoint_mut([
            &city_a.overmap_chunk_coordinate,
            &city_b.overmap_chunk_coordinate,
        ])
        .map(|x| x.unwrap());

    // Connect city_a to city_b
    current_chunk
        .cities
        .get_mut(&city_a.id)
        .unwrap()
        .connected_to
        .push((city_b.overmap_chunk_coordinate, city_b.id.clone()));

    // Connect city_b to city_a
    neighbouring_chunk
        .cities
        .get_mut(&city_b.id)
        .unwrap()
        .connected_to
        .push((city_a.overmap_chunk_coordinate, city_a.id.clone()));
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
            size: 128,
            connected_to: vec![],
        };

        overmap_chunk.cities.insert(CityId(0), city);
    }
}

fn insert_road_tiles_between_cities(
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

    for _ in 0..distance_between_cities.x {
        cursor.step_x(direction.x);
        let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();
        cursor.set_tile(overmap_chunk, OvermapTileType::Road);
    }

    for _ in 0..distance_between_cities.y {
        cursor.step_y(direction.y);
        let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();
        cursor.set_tile(overmap_chunk, OvermapTileType::Road);
    }

    let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();

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

    fn peek_tile(
        &self,
        overmap_chunks: &HashMap<OvermapChunkCoords, OvermapChunk>,
        delta: IVec2,
    ) -> Option<OvermapTileType> {
        let chunk_delta = (delta + self.local_pos).div_euclid(IVec2::splat(OVERMAP_CHUNK_SIZE));
        let new_pos = (delta + self.local_pos).rem_euclid(IVec2::splat(OVERMAP_CHUNK_SIZE));

        let chunk_coord = OvermapChunkCoords(&self.chunk_coord.0 + chunk_delta);
        overmap_chunks
            .get(&chunk_coord)
            .map(|x| x.overmap_tiles[xy_idx(new_pos.x, new_pos.y, OVERMAP_CHUNK_SIZE as usize)])
            .flatten()
    }

    fn set_tile(&self, overmap_chunk: &mut OvermapChunk, tile_type: OvermapTileType) {
        if overmap_chunk.overmap_tiles[xy_idx(
            self.local_pos.x,
            self.local_pos.y,
            OVERMAP_CHUNK_SIZE as usize,
        )] != Some(OvermapTileType::House)
        {
            overmap_chunk.overmap_tiles[xy_idx(
                self.local_pos.x,
                self.local_pos.y,
                OVERMAP_CHUNK_SIZE as usize,
            )] = Some(tile_type);
        }
    }
}

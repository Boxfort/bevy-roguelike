use bevy::{
    math::{IVec2, Vec2Swizzles},
    platform::collections::HashMap,
};
use chacha20::ChaCha8Rng;
use rand::{Rng, RngExt};

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

#[derive(Clone, Debug)]
struct BuildingCandidates {
    candidates: HashMap<usize, BuildingCandidate>,
    occupancy_map: HashMap<(OvermapChunkCoords, IVec2), usize>,
}

#[derive(Clone, Debug)]
/// Buildings are always anchored from the bottom left (relative to orientation)
/// ```
/// XX  RoX  RR
/// oX  RXX  Xo
/// RR       XX
/// ```
struct BuildingCandidate {
    position: IVec2,
    chunk_coord: OvermapChunkCoords,
    direction_to_road: IVec2,
    shape: BuildingShape,
}

#[derive(Clone, Debug)]
enum BuildingShape {
    OneByOne,
    OneByTwo,
    TwoByOne,
    TwoByTwo,
}

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
                        connect_cities_with_road(game_map, city_a.1, city_b.1, rng);
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
                    current_overmap_chunk_coord,
                    IVec2 { x: 0, y: 1 },
                    0,
                ),
                RoadBuilder::new(
                    current_overmap_chunk_coord,
                    city.position,
                    current_overmap_chunk_coord,
                    IVec2 { x: 0, y: -1 },
                    0,
                ),
                RoadBuilder::new(
                    current_overmap_chunk_coord,
                    city.position,
                    current_overmap_chunk_coord,
                    IVec2 { x: 1, y: 0 },
                    0,
                ),
                RoadBuilder::new(
                    current_overmap_chunk_coord,
                    city.position,
                    current_overmap_chunk_coord,
                    IVec2 { x: -1, y: 0 },
                    0,
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
    origin_chunk: OvermapChunkCoords,
    direction: IVec2,
    generation: i32,
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
        origin_chunk: OvermapChunkCoords,
        direction: IVec2,
        generation: i32,
    ) -> RoadBuilder {
        let actions = match generation {
            0 => vec![(Continue, 8), (SplitTwice, 2), (SplitTwiceAndContinue, 2)],
            _ => vec![
                (Continue, 4),
                (SplitOnce, 1),
                (SplitOnceAndContinue, 2),
                (SplitTwice, 1),
                (SplitTwiceAndContinue, 2),
            ],
        };
        RoadBuilder {
            chunk_cursor: ChunkCursor {
                chunk_coord: chunk_coordinate,
                local_pos: start_position,
            },
            origin_chunk: origin_chunk,
            direction: direction,
            generation: generation,
            cumulative_action_weights: actions.iter().map(|x| x.1).sum(),
            action_probabilities: actions,
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

        if (road_builders[i].chunk_cursor.chunk_coord.0 - road_builders[i].origin_chunk.0)
            .abs()
            .max_element()
            > 1
        {
            road_builders.remove(i);
            continue 'outer;
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

        // Kill builder based on distance from city center
        let stop_roll = rng.random_range(0..100);

        let distance_from_center: f32 = ((OVERMAP_CHUNK_SIZE
            * road_builders[i].chunk_cursor.chunk_coord.0)
            + road_builders[i].chunk_cursor.local_pos)
            .manhattan_distance(
                (OVERMAP_CHUNK_SIZE * city.overmap_chunk_coordinate.0) + city.position,
            ) as f32;

        let t = distance_from_center / city.size as f32;
        let stop_probability = (100.0 * t * t * t) as i32; // Quadratic

        if stop_roll < stop_probability {
            if road_builders[i].generation == 0 {
                println!("KILLED BUILDER DUE TO STOP ROLL");
            }
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
            for (action, weight) in road_builders[i].action_probabilities.clone() {
                cumulative += weight;
                if action_roll < cumulative {
                    match action {
                        Continue => {
                            i += 1;
                        }
                        SplitOnce => {
                            let dir = rng.random_range(0..=1);
                            spawn_child_road_builder(road_builders, city, dir, i);
                            if road_builders[i].generation == 0 {
                                println!("KILLED BUILDER DUE TO SPLIT");
                            }
                            road_builders.remove(i);
                        }
                        SplitOnceAndContinue => {
                            let dir = rng.random_range(0..=1);
                            spawn_child_road_builder(road_builders, city, dir, i);
                            road_builders[i].moves_since_split = 0;
                            i += 1
                        }
                        SplitTwice => {
                            spawn_child_road_builder(road_builders, city, 0, i);
                            spawn_child_road_builder(road_builders, city, 1, i);
                            if road_builders[i].generation == 0 {
                                println!("KILLED BUILDER DUE TO SPLIT");
                            }
                            road_builders.remove(i);
                        }
                        SplitTwiceAndContinue => {
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
        road_builder.origin_chunk,
        road_builder.direction.yx() * if dir > 0 { 1 } else { -1 },
        road_builder.generation + 1,
    ));
}

fn connect_cities_with_road(
    game_map: &mut GameMap,
    city_a: &City,
    city_b: &City,
    rng: &mut ChaCha8Rng,
) {
    insert_road_tiles_between_cities_baised(
        &mut game_map.overmap_chunks,
        city_a.position,
        city_a.overmap_chunk_coordinate,
        city_b.position,
        city_b.overmap_chunk_coordinate,
        rng,
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

fn insert_road_tiles_between_cities_old(
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

    let direction_to_chunk = end_city_chunk.0 - start_city_chunk.0;
    let direction_to_city = end_city_pos - start_city_pos;

    let direction = IVec2 {
        x: if direction_to_chunk.x != 0 {
            direction_to_chunk.x.signum()
        } else {
            direction_to_city.x.signum()
        },
        y: if direction_to_chunk.y != 0 {
            direction_to_chunk.y.signum()
        } else {
            direction_to_city.y.signum()
        },
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

fn insert_road_tiles_between_cities(
    overmap_chunks: &mut HashMap<OvermapChunkCoords, OvermapChunk>,
    start_city_pos: IVec2,
    start_city_chunk: OvermapChunkCoords,
    end_city_pos: IVec2,
    end_city_chunk: OvermapChunkCoords,
    rng: &mut ChaCha8Rng,
) {
    let mut cursor = ChunkCursor {
        chunk_coord: start_city_chunk,
        local_pos: IVec2 {
            x: start_city_pos.x,
            y: start_city_pos.y,
        },
    };

    let mut distance_between_cities = ((start_city_pos
        + (start_city_chunk.0 * OVERMAP_CHUNK_SIZE))
        - (end_city_pos + (end_city_chunk.0 * OVERMAP_CHUNK_SIZE)))
        .abs();

    let direction_to_chunk = end_city_chunk.0 - start_city_chunk.0;
    let direction_to_city = end_city_pos - start_city_pos;

    let direction = IVec2 {
        x: if direction_to_chunk.x != 0 {
            direction_to_chunk.x.signum()
        } else {
            direction_to_city.x.signum()
        },
        y: if direction_to_chunk.y != 0 {
            direction_to_chunk.y.signum()
        } else {
            direction_to_city.y.signum()
        },
    };

    let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();
    cursor.set_tile(overmap_chunk, OvermapTileType::House);

    // draw the exit road
    if distance_between_cities.x > distance_between_cities.y {
        for _ in 0..distance_between_cities.x.min(32) * direction.x {
            cursor.step_x(direction.x);
            let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();
            cursor.set_tile(overmap_chunk, OvermapTileType::Road);
        }
    } else {
        for _ in 0..distance_between_cities.y.min(32) * direction.y {
            cursor.step_y(direction.y);
            let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();
            cursor.set_tile(overmap_chunk, OvermapTileType::Road);
        }
    };

    let mut distance_to_end = ((cursor.local_pos + (cursor.chunk_coord.0 * OVERMAP_CHUNK_SIZE))
        - (end_city_pos + (end_city_chunk.0 * OVERMAP_CHUNK_SIZE)))
        .abs();

    // get entrance road start pos
    if distance_to_end.x > distance_to_end.y {
        distance_to_end.x -= distance_to_end.x.min(32)
    } else {
        distance_to_end.y -= distance_to_end.y.min(32)
    };

    while distance_to_end.element_sum() > 0 {
        let roll = rng.random_range(0..distance_to_end.element_sum());
        if roll < distance_to_end.x {
            cursor.step_x(direction.x);
            let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();
            cursor.set_tile(overmap_chunk, OvermapTileType::Road);
            distance_to_end.x -= 1;
        } else {
            cursor.step_y(direction.y);
            let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();
            cursor.set_tile(overmap_chunk, OvermapTileType::Road);
            distance_to_end.y -= 1;
        }
    }

    distance_to_end = ((cursor.local_pos + (cursor.chunk_coord.0 * OVERMAP_CHUNK_SIZE))
        - (end_city_pos + (end_city_chunk.0 * OVERMAP_CHUNK_SIZE)))
        .abs();

    while distance_to_end.element_sum() > 0 {
        let roll = rng.random_range(0..distance_to_end.element_sum());
        if roll < distance_to_end.x {
            cursor.step_x(direction.x);
            let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();
            cursor.set_tile(overmap_chunk, OvermapTileType::Road);
            distance_to_end.x -= 1;
        } else {
            cursor.step_y(direction.y);
            let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();
            cursor.set_tile(overmap_chunk, OvermapTileType::Road);
            distance_to_end.y -= 1;
        }
    }

    let overmap_chunk = overmap_chunks.get_mut(&cursor.chunk_coord).unwrap();
    cursor.set_tile(overmap_chunk, OvermapTileType::House);
}

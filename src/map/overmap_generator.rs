use std::hash::Hash;

use bevy::{
    math::{IVec2, Vec2Swizzles},
    platform::collections::HashMap,
    ui::debug::print_ui_layout_tree,
};
use chacha20::ChaCha8Rng;
use rand::RngExt;

use crate::{
    map::{
        chunk_cursor::{self, ChunkCursor},
        overmap_generator::{
            BuildingShape::{OneByTwo, TwoByOne, TwoByTwo},
            OvermapTileType::Road,
            RoadBuilderAction::*,
        },
        renderer::TileType,
    },
    utils::{get_coords_in_square_radius, get_neighbouring_cardinal_coordinates, xy_idx},
};

pub const OVERMAP_CHUNK_SIZE: i32 = 128;

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

pub struct OvermapTile {
    tile_type: OvermapTileType,
    glyph: i32
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

#[derive(Debug)]
pub struct Building {
    position: IVec2,
    bounds: IVec2,
}

#[derive(Clone, Debug)]
struct BuildingCandidates {
    id_counter: usize,
    candidates: HashMap<usize, BuildingCandidate>,
    occupancy_map: HashMap<(OvermapChunkCoords, IVec2), Vec<usize>>,
}

impl BuildingCandidates {
    pub fn get_next_id(&mut self) -> usize {
        let new_id: usize = self.id_counter;
        self.id_counter += 1;

        new_id
    }
}

#[derive(Clone, Debug)]
/// Buildings are always anchored from the bottom left (relative to orientation)
/// ```
/// XX  RoX  RR
/// oX  RXX  Xo
/// RR       XX
/// ```
struct BuildingCandidate {
    id: usize,
    position: IVec2,
    chunk_coord: OvermapChunkCoords,
    direction_to_road: IVec2,
    shape: BuildingShape,
}

#[derive(Clone, Debug, PartialEq)]
enum BuildingShape {
    OneByOne,
    OneByTwo,
    TwoByOne,
    TwoByTwo,
}

impl BuildingShape {
    pub fn placement_chance(&self) -> i32 {
        match self {
            BuildingShape::OneByOne => 8,
            OneByTwo => 1,
            TwoByOne => 2,
            TwoByTwo => 4,
        }
    }

    pub fn footprint(&self, dir_to_road: IVec2) -> Vec<IVec2> {
        let away = dir_to_road * -1;
        let adjacent = dir_to_road.yx();

        match self {
            Self::OneByOne => vec![IVec2::ZERO],
            Self::OneByTwo => vec![IVec2::ZERO, away],
            Self::TwoByOne => vec![IVec2::ZERO, adjacent],
            Self::TwoByTwo => {
                vec![IVec2::ZERO, away, adjacent, away + adjacent]
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

    pub fn get_perpendicular_directions(&self) -> Vec<IVec2> {
        vec![self.direction.yx(), self.direction.yx() * -1]
    }
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

    connect_cities(game_map, current_overmap_coordinate, rng);
    generate_cities(game_map, current_overmap_coordinate, rng);
}

fn connect_cities(game_map: &mut GameMap, current_overmap_coordinate: IVec2, rng: &mut ChaCha8Rng) {
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
}

fn generate_cities(
    game_map: &mut GameMap,
    current_overmap_coordinate: IVec2,
    rng: &mut ChaCha8Rng,
) {
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

        let mut building_candidates = BuildingCandidates {
            id_counter: 0,
            candidates: HashMap::new(),
            occupancy_map: HashMap::new(),
        };

        for city in current_chunk_cities {
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

            while !road_builders.is_empty() {
                run_road_builder_iteration(
                    &mut road_builders,
                    &city,
                    &mut game_map.overmap_chunks,
                    &mut building_candidates,
                    rng,
                )
            }

            // Expand the building candidates
            expand_building_candidates(&mut building_candidates, &mut game_map.overmap_chunks);

            // DEBUG
            draw_houses(game_map, &mut building_candidates, rng);
        }
    }
}

fn draw_houses(
    game_map: &mut GameMap,
    building_candidates: &mut BuildingCandidates,
    rng: &mut ChaCha8Rng,
) {
    let mut candidate_keys: Vec<_> = building_candidates.candidates.keys().copied().collect();

    while let Some(candidate_id) = candidate_keys.pop() {
        // If the candidate was removed in another iteration continue
        if building_candidates.candidates.get(&candidate_id).is_none() {
            continue;
        }

        // If we decide to place the building
        if rng.random_range(0..10)
            < building_candidates.candidates[&candidate_id]
                .shape
                .placement_chance()
        {
            let chunk_cursor = ChunkCursor {
                chunk_coord: building_candidates.candidates[&candidate_id].chunk_coord,
                local_pos: building_candidates.candidates[&candidate_id].position,
            };

            // For all the squares in the buildings footprint
            for delta in building_candidates.candidates[&candidate_id]
                .shape
                .footprint(building_candidates.candidates[&candidate_id].direction_to_road)
            {
                let (chunk, pos) = chunk_cursor.get_position_delta(delta);

                // Remove ALL candididates in the occupancy map
                let ids_at_position = building_candidates.occupancy_map[&(chunk, pos)].clone();
                for id in ids_at_position {
                    // Get candidate
                    // Remove id from all occupancy squares it intersects
                    let maybe_candidate = building_candidates.candidates.remove(&id);

                        /*
                    if let Some(candidate_to_remove) = maybe_candidate {
                        //println!("REMOVED CANDIDATE {:?}", candidate_to_remove.id);
                        let chunk_cursor = ChunkCursor {
                            chunk_coord: candidate_to_remove.chunk_coord,
                            local_pos: candidate_to_remove.position,
                        };

                        for delta in candidate_to_remove
                            .shape
                            .footprint(candidate_to_remove.direction_to_road)
                        {
                            let (chunk, pos) = chunk_cursor.get_position_delta(delta);
                            building_candidates
                                .occupancy_map
                                .get_mut(&(chunk, pos))
                                .map(|o| o.retain(|x| *x != candidate_to_remove.id));
                        }
                    }
                        */
                }

                // Remove all entries in occupancy map
                //building_candidates.occupancy_map.remove(&(chunk, pos));

                game_map
                    .overmap_chunks
                    .get_mut(&chunk)
                    .unwrap()
                    .overmap_tiles[xy_idx(pos.x, pos.y, OVERMAP_CHUNK_SIZE as usize)] =
                    Some(OvermapTileType::House);
            }
        }
    }
}

fn expand_building_candidates(
    building_candidates: &mut BuildingCandidates,
    overmap_chunks: &mut HashMap<OvermapChunkCoords, OvermapChunk>,
) {
    let mut new_candidates: HashMap<usize, BuildingCandidate> = HashMap::new();
    let mut new_occupancies: HashMap<(OvermapChunkCoords, IVec2), Vec<usize>> = HashMap::new();

    // For each candidate, test the surrounding tiles and add new candidates if the space is free.
    let mut curr_candidate_id = building_candidates.id_counter;

    for (_, candidate) in &building_candidates.candidates {
        let dir_away_from_road = candidate.direction_to_road * -1;
        let dir_adjacent_to_road = candidate.direction_to_road.yx();

        let chunk_cursor = ChunkCursor {
            chunk_coord: candidate.chunk_coord,
            local_pos: candidate.position,
        };

        // Test positions
        let a_delta = dir_away_from_road;
        let b_delta = dir_adjacent_to_road;
        let c_delta = dir_away_from_road + dir_adjacent_to_road;
        let a_is_free = chunk_cursor
            .peek_tile(overmap_chunks, a_delta)
            .is_none_or(|x| x != OvermapTileType::Road || x != OvermapTileType::House);
        let b_is_free = chunk_cursor
            .peek_tile(overmap_chunks, b_delta)
            .is_none_or(|x| x != OvermapTileType::Road || x != OvermapTileType::House);
        let c_is_free = chunk_cursor
            .peek_tile(overmap_chunks, c_delta)
            .is_none_or(|x| x != OvermapTileType::Road || x != OvermapTileType::House);

        let shape = match (a_is_free, b_is_free, c_is_free) {
            // RoA
            // RBC
            (true, true, true) => Some(TwoByTwo),
            // RoA
            // Rxx
            (true, false, false) => Some(OneByTwo),
            // Rox
            // RBx
            (false, true, false) => Some(TwoByOne),
            _ => None,
        };

        if let Some(shape) = shape {
            for delta in shape.footprint(candidate.direction_to_road) {
                let pos = chunk_cursor.get_position_delta(delta);
                new_occupancies
                    .entry(pos)
                    .or_default()
                    .push(curr_candidate_id);
            }

            let new_candidate = BuildingCandidate {
                id: curr_candidate_id,
                position: candidate.position,
                chunk_coord: candidate.chunk_coord,
                direction_to_road: candidate.direction_to_road,
                shape,
            };

            new_candidates.insert(curr_candidate_id, new_candidate);
            curr_candidate_id += 1;
        }
    }

    building_candidates.id_counter = curr_candidate_id;
    building_candidates.candidates.extend(new_candidates);
    building_candidates.occupancy_map.extend(new_occupancies);
}

fn run_road_builder_iteration(
    road_builders: &mut Vec<RoadBuilder>,
    city: &City,
    overmap_chunks: &mut HashMap<OvermapChunkCoords, OvermapChunk>,
    building_candidates: &mut BuildingCandidates,
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
        if road_builders[i].generation > 0 {
            if let Some(tile) = current_tile
                && tile == Road
            {
                road_builders.remove(i);
                continue 'outer;
            } else {
                // Kill builder if next to a road
                for dir in road_builders[i].get_perpendicular_directions() {
                    if let Some(tile_type) =
                        road_builders[i].chunk_cursor.peek_tile(overmap_chunks, dir)
                        && tile_type == Road
                    {
                        road_builders.remove(i);
                        continue 'outer;
                    }
                }
            }
        }

        // Set road tile
        road_builders[i].chunk_cursor.set_tile(overmap_chunks, Road);

        // Remove any building candidates where we just placed a road.
        let maybe_building_ids = building_candidates.occupancy_map.get_mut(&(
            road_builders[i].chunk_cursor.chunk_coord,
            road_builders[i].chunk_cursor.local_pos,
        ));

        if let Some(building_ids) = maybe_building_ids {
            for id in building_ids {
                building_candidates.candidates.remove(id);
            }
        }

        building_candidates.occupancy_map.remove(&(
            road_builders[i].chunk_cursor.chunk_coord,
            road_builders[i].chunk_cursor.local_pos,
        ));

        // Set potential buildings
        for dir in road_builders[i].get_perpendicular_directions() {
            // Don't consider spots already occupied by another tile
            if let Some(tile) = road_builders[i].chunk_cursor.peek_tile(overmap_chunks, dir)
                && [OvermapTileType::Road, OvermapTileType::House].contains(&tile)
            {
                continue;
            }

            let (chunk_coord, pos) = road_builders[i].chunk_cursor.get_position_delta(dir);

            let next_candidate_id = building_candidates.id_counter;

            let building_candidate_ids = building_candidates
                .occupancy_map
                .entry((chunk_coord, pos))
                .or_insert(vec![]);

            if building_candidate_ids.is_empty() {
                let building_candidate = BuildingCandidate {
                    id: next_candidate_id,
                    position: pos,
                    chunk_coord,
                    direction_to_road: dir * -1,
                    shape: BuildingShape::OneByOne,
                };
                building_candidate_ids.push(building_candidate.id);
                building_candidates
                    .candidates
                    .insert(building_candidate.id, building_candidate);
                building_candidates.id_counter = next_candidate_id + 1;
            }
        }

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
    insert_road_tiles_between_cities(&mut game_map.overmap_chunks, city_a, city_b, rng);

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
    if rng.random_range(0..=2) > 0 {
        // Add a city somewhere
        let new_city_position = IVec2 {
            x: rng.random_range(0..OVERMAP_CHUNK_SIZE),
            y: rng.random_range(0..OVERMAP_CHUNK_SIZE),
        };

        let city_size: i32 = rng.random_range(64..=128);

        let city = City {
            id: CityId(0), // TODO: when/if we decide to add multiple cities then this will need to be set
            position: new_city_position,
            overmap_chunk_coordinate: overmap_chunk.coordinates,
            size: city_size,
            connected_to: vec![],
        };

        overmap_chunk.cities.insert(CityId(0), city);
    }
}

fn insert_road_tiles_between_cities(
    overmap_chunks: &mut HashMap<OvermapChunkCoords, OvermapChunk>,
    city_a: &City,
    city_b: &City,
    rng: &mut ChaCha8Rng,
) {
    let start_city_pos: IVec2 = city_a.position;
    let start_city_chunk: OvermapChunkCoords = city_a.overmap_chunk_coordinate;
    let end_city_pos: IVec2 = city_b.position;
    let end_city_chunk: OvermapChunkCoords = city_b.overmap_chunk_coordinate;

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

    cursor.set_tile(overmap_chunks, OvermapTileType::House);

    // draw the exit road
    if distance_between_cities.x > distance_between_cities.y {
        for _ in 0..distance_between_cities.x.min(city_a.size / 2) * direction.x {
            cursor.step_x(direction.x);
            cursor.set_tile(overmap_chunks, OvermapTileType::Road);
        }
    } else {
        for _ in 0..distance_between_cities.y.min(city_a.size / 2) * direction.y {
            cursor.step_y(direction.y);
            cursor.set_tile(overmap_chunks, OvermapTileType::Road);
        }
    };

    let mut distance_to_end = ((cursor.local_pos + (cursor.chunk_coord.0 * OVERMAP_CHUNK_SIZE))
        - (end_city_pos + (end_city_chunk.0 * OVERMAP_CHUNK_SIZE)))
        .abs();

    // get entrance road start pos
    if distance_to_end.x > distance_to_end.y {
        distance_to_end.x -= distance_to_end.x.min(city_b.size / 2)
    } else {
        distance_to_end.y -= distance_to_end.y.min(city_b.size / 2)
    };

    while distance_to_end.element_sum() > 0 {
        let roll = rng.random_range(0..distance_to_end.element_sum());
        if roll < distance_to_end.x {
            cursor.step_x(direction.x);
            cursor.set_tile(overmap_chunks, OvermapTileType::Road);
            distance_to_end.x -= 1;
        } else {
            cursor.step_y(direction.y);
            cursor.set_tile(overmap_chunks, OvermapTileType::Road);
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
            cursor.set_tile(overmap_chunks, OvermapTileType::Road);
            distance_to_end.x -= 1;
        } else {
            cursor.step_y(direction.y);
            cursor.set_tile(overmap_chunks, OvermapTileType::Road);
            distance_to_end.y -= 1;
        }
    }

    cursor.set_tile(overmap_chunks, OvermapTileType::House);
}

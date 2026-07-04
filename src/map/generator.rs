use bevy::{
    math::IVec2,
    platform::collections::{HashMap, HashSet},
    sprite_render::TileData,
};
use chacha20::ChaCha8Rng;
use rand::{Rng, RngExt, SeedableRng};

const OVERMAP_CHUNK_SIZE: i32 = 2048;

struct GameMap {
    overmap_chunks: HashMap<OvermapChunkCoords, OvermapChunk>,
}

#[derive(Eq, Hash, PartialEq, Clone, Copy)]
struct OvermapChunkCoords(IVec2);

struct OvermapChunk {
    coordinates: OvermapChunkCoords,
    cities: HashSet<City>,
    roads: RoadGraph,
}

impl OvermapChunk {
    pub fn new(coordinates: OvermapChunkCoords) -> OvermapChunk {
        OvermapChunk {
            coordinates,
            cities: HashSet::new(),
            roads: RoadGraph::new(),
        }
    }
}

struct RoadGraph {
    nodes: Vec<RoadNode>,
    edges: Vec<RoadEdge>,
    connections: HashMap<RoadNodeId, Vec<(OvermapChunkCoords, RoadEdgeId)>>,
}

impl RoadGraph {
    pub fn new() -> RoadGraph {
        RoadGraph {
            nodes: vec![],
            edges: vec![],
            connections: HashMap::new(),
        }
    }
}

struct RoadEdgeId(i32);
struct RoadEdge {
    id: RoadEdgeId,
    a: RoadNodeId,
    b: RoadNodeId,
}

struct RoadNodeId(i32);
struct RoadNode {
    id: RoadNodeId,
    position: IVec2,
}

#[derive(Eq, Hash, PartialEq)]
struct CityId(i32);

#[derive(Eq, Hash, PartialEq)]
struct City {
    id: CityId,
    position: IVec2,
    size: i32, // The estimated size of the city, used for influencing the procgen
    connected_to: Vec<CityConnection>,
}

#[derive(PartialEq, Eq, Hash)]
struct CityConnection {
    overmap_coordinate: OvermapChunkCoords,
    city_id: CityId
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
            let pos = IVec2 {
                x: rng.random_range(0..OVERMAP_CHUNK_SIZE),
                y: rng.random_range(0..OVERMAP_CHUNK_SIZE),
            };

            let city = City {
                id: CityId(cities.len() as i32),
                position: pos,
                size: 512,
                connected_to: vec![],
            };

            overmap_chunk.cities.insert(city);

            // Add connecting road to adjacent cities.
            let adjacent_chunks = get_potential_neighbouring_city_overmap_chunks(chunk_coordinate.0);

            for city_chunk in adjacent_chunks {
                // Does it contain a city
                let chunk = game_map.overmap_chunks.get(&city_chunk).unwrap();

                if !chunk.cities.is_empty() {
                    // Add connecting road
                }
            }
        }

        game_map.overmap_chunks.insert(
             chunk_coordinate,
            overmap_chunk
        );
    }
}

fn get_potential_neighbouring_city_overmap_chunks(chunk_coordinate: IVec2) -> Vec<OvermapChunkCoords> {
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

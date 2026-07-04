use bevy::{
    math::IVec2,
    platform::collections::{HashMap, HashSet},
    sprite_render::TileData,
};
use chacha20::ChaCha8Rng;
use rand::{Rng, RngExt, SeedableRng};

const OVERMAP_CHUNK_SIZE: i32 = 2048;

struct OvermapChunkCoords(IVec2);
struct OvermapChunk {
    coordinates: OvermapChunkCoords,
    cities: HashSet<City>,
    roads: RoadGraph,
    structure: HashSet<City>,
}

struct RoadGraph {
    nodes: Vec<RoadNode>,
    edges: Vec<RoadEdge>,
    connections: HashMap<RoadNodeId, Vec<(OvermapChunkCoords, RoadEdgeId)>>,
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

struct CityId(i32);

struct City {
    id: CityId,
    position: IVec2,
    size: i32, // The estimated size of the city, used for influencing the procgen
    connected_to: HashSet<(OvermapChunkCoords, CityId)>,
}

struct Building {
    position: IVec2,
    bounds: IVec2,
}

pub fn generate_map(current_overmap_coordinate: IVec2) {
    let neighbouring_chunk_positions =
        get_neighbouring_overmap_chunk_positions(current_overmap_coordinate);
    let mut rng = ChaCha8Rng::seed_from_u64(42); // TODO: remove and use the resource

    let cities: Vec<City> = vec![];

    for chunk_pos in neighbouring_chunk_positions {
        if rng.random_range(0..=1) == 1 {
            // Add a city somewhere
            let pos = IVec2 {
                x: rng.random_range(0..2048),
                y: rng.random_range(0..2048),
            };

            let city = City {
                id: CityId(cities.len() as i32),
                position: pos,
                size: 512,
                connected_to: HashSet::new(),
            };
        }
    }
}

fn get_neighbouring_overmap_chunk_positions(chunk_coordinate: IVec2) -> Vec<IVec2> {
    let mut adjacent_positions: Vec<IVec2> = vec![];

    for x in -2..=2 {
        for y in -2..=2 {
            adjacent_positions.push(chunk_coordinate + IVec2 { x, y });
        }
    }

    adjacent_positions
}

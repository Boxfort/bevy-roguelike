use bevy::{math::IVec2, platform::collections::HashMap};

use crate::{
    map::overmap_generator::{
        OVERMAP_CHUNK_SIZE, OvermapChunk, OvermapChunkCoords, OvermapTileType,
    },
    utils::xy_idx,
};

#[derive(Debug)]
pub struct ChunkCursor {
    pub chunk_coord: OvermapChunkCoords,
    pub local_pos: IVec2,
}

impl ChunkCursor {
    pub fn step_x(&mut self, delta: i32) {
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

    pub fn step_y(&mut self, delta: i32) {
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

    pub fn get_position_delta(&self, delta: IVec2) -> (OvermapChunkCoords, IVec2) {
        let chunk_delta = (delta + self.local_pos).div_euclid(IVec2::splat(OVERMAP_CHUNK_SIZE));
        let new_pos = (delta + self.local_pos).rem_euclid(IVec2::splat(OVERMAP_CHUNK_SIZE));
        let new_chunk_coord = OvermapChunkCoords(self.chunk_coord.0 + chunk_delta);

        (new_chunk_coord, new_pos)
    }

    pub fn peek_tile(
        &self,
        overmap_chunks: &HashMap<OvermapChunkCoords, OvermapChunk>,
        delta: IVec2,
    ) -> Option<OvermapTileType> {
        let (chunk_coord, new_pos) = self.get_position_delta(delta);
        overmap_chunks.get(&chunk_coord).and_then(|x| {
            x.overmap_tiles[xy_idx(new_pos.x, new_pos.y, OVERMAP_CHUNK_SIZE as usize)]
        })
    }

    pub fn set_tile(
        &self,
        overmap_chunks: &mut HashMap<OvermapChunkCoords, OvermapChunk>,
        tile_type: OvermapTileType,
    ) {
        if overmap_chunks[&self.chunk_coord].overmap_tiles[xy_idx(
            self.local_pos.x,
            self.local_pos.y,
            OVERMAP_CHUNK_SIZE as usize,
        )] != Some(OvermapTileType::House)
        {
            overmap_chunks
                .get_mut(&self.chunk_coord)
                .unwrap()
                .overmap_tiles[xy_idx(
                self.local_pos.x,
                self.local_pos.y,
                OVERMAP_CHUNK_SIZE as usize,
            )] = Some(tile_type);
        }
    }
}

use bevy::{platform::collections::HashMap, prelude::*};
use chacha20::ChaCha8Rng;
use rand::SeedableRng;

use crate::{
    map::renderer::{MapData, generate_chunk_data, load_chunks, render_map_chunks},
    systems::camera::move_camera,
};

mod map;
mod systems;

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_systems(Startup, (setup, generate_chunk_data).chain())
        .add_systems(
            Update,
            (move_camera, load_chunks, render_map_chunks).chain(),
        );

    app.run();
}

#[derive(Resource, Deref, DerefMut)]
struct SeededRng(ChaCha8Rng);

fn setup(mut commands: Commands) {
    // We're seeding the PRNG here to make this example deterministic for testing purposes.
    // This isn't strictly required in practical use unless you need your app to be deterministic.
    let rng = ChaCha8Rng::seed_from_u64(42);

    commands.spawn(Camera2d);
    commands.insert_resource(SeededRng(rng));
    commands.insert_resource(MapData {
        loaded_chunks: vec![],
        chunk_data: HashMap::new(),
    });
}

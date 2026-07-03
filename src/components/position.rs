use bevy::{ecs::component::Component, math::IVec2};

#[derive(Component)]
pub struct Position(pub IVec2);

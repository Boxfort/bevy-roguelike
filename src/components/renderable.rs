use bevy::{
    color::{Color, LinearRgba, Srgba},
    ecs::component::Component,
};

#[derive(Component)]
pub struct NeedsSprite;

#[derive(Component)]
pub struct Renderable {
    pub tilemap_index: i32,
    pub fg: GlyphColor,
    pub bg: GlyphColor,
}

#[derive(Clone)]
pub enum GlyphColor {
    BLACK,
    WHITE,
    BLUE,
    GREEN,
    ORANGE,
    PINK,
}

impl Into<LinearRgba> for GlyphColor {
    fn into(self) -> LinearRgba {
        match self {
            GlyphColor::WHITE => LinearRgba::from(Srgba::WHITE),
            GlyphColor::BLACK => LinearRgba::from(Srgba::BLACK),
            GlyphColor::BLUE => LinearRgba::from(Srgba::hex("a2fff3").unwrap()),
            GlyphColor::GREEN => LinearRgba::from(Srgba::hex("cbf382").unwrap()),
            GlyphColor::ORANGE => LinearRgba::from(Srgba::hex("ffcbba").unwrap()),
            GlyphColor::PINK => LinearRgba::from(Srgba::hex("e3b2ff").unwrap()),
        }
    }
}

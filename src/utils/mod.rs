use bevy::math::IVec2;

/// Convert an x,y coordinate into an index based on some logical max bounds.
pub fn xy_idx(x: i32, y: i32, max_bounds: usize) -> usize {
    (y as usize * max_bounds) + x as usize
}

/// Returns all coordinates around [from_coord](IVec2) in a square [radius](i32).
/// Setting [border_only](bool) to true will only return the outer ring of coordinates.
pub fn get_coords_in_square_radius(
    from_coord: IVec2,
    radius: i32,
    border_only: bool,
) -> Vec<IVec2> {
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

/// Returns the neighbouring coordinates around [from_coord](IVec2) in each cardinal direction.
pub fn get_neighbouring_cardinal_coordinates(from_coord: IVec2) -> Vec<IVec2> {
    vec![
        from_coord + IVec2 { x: 1, y: 0 },
        from_coord + IVec2 { x: -1, y: 0 },
        from_coord + IVec2 { x: 0, y: 1 },
        from_coord + IVec2 { x: 0, y: -1 },
    ]
}

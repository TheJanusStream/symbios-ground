//! On-demand tile streaming around a moving viewer.
//!
//! Demonstrates [`TiledHeightMap`] driving a sparse, infinite world. A
//! virtual viewer walks a path through tile space; each step ensures the
//! tiles within a radius are generated and evicts anything farther than the
//! retention radius. The example prints a per-step summary so you can see
//! the loaded-tile count rise and fall as the viewer moves.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example streaming_tiles
//! ```

use symbios_ground::{FbmNoise, TiledHeightMap};

fn main() {
    // 256-cell tiles at 1 world-unit per cell ⇒ 256-unit-wide tiles.
    let mut world = TiledHeightMap::new(256, 1.0, 1234, |seed| Box::new(FbmNoise::new(seed)));

    // Viewer walks east, then north-east, then south. Each entry is a
    // (tile_x, tile_z) coordinate centred on the viewer.
    let path = [
        (0, 0),
        (1, 0),
        (2, 0),
        (3, 0),
        (4, 1),
        (5, 2),
        (5, 3),
        (5, 4),
        (4, 4),
        (3, 4),
    ];

    let load_radius: i32 = 1; // ensure 3×3 around viewer
    let retain_radius: i32 = 2; // keep 5×5; evict beyond that

    println!(
        "world: tile_size={} cells, scale={} world units, base_seed={}",
        world.tile_size(),
        world.scale(),
        1234,
    );
    println!(
        "loading {} tiles around the viewer, retaining {} tiles\n",
        (2 * load_radius + 1).pow(2),
        (2 * retain_radius + 1).pow(2),
    );

    for (step, &centre) in path.iter().enumerate() {
        world.ensure_radius(centre, load_radius);
        let evicted = world.evict_outside(centre, retain_radius);

        // Sample the height at the viewer's exact world position to
        // demonstrate world-space queries.
        let world_x = centre.0 as f32 * world.tile_world_size() + world.tile_world_size() * 0.5;
        let world_z = centre.1 as f32 * world.tile_world_size() + world.tile_world_size() * 0.5;
        let height = world.sample_height_at(world_x, world_z);

        println!(
            "step {step:>2}: viewer at tile {centre:?}  loaded={:>3}  evicted={evicted:>2}  height@centre={height:.4}",
            world.loaded_count(),
        );
    }

    println!(
        "\nfinal: {} tiles in memory across the visited path",
        world.loaded_count()
    );
}

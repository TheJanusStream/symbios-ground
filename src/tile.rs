//! LOD / tile streaming abstraction for large heightmaps.
//!
//! [`TiledHeightMap`] partitions an arbitrarily large world into fixed-size
//! [`HeightMapTile`]s and generates them on demand using a per-tile seed
//! derived deterministically from a base seed and the tile's `(x, z)`
//! coordinates. Unrequested tiles never allocate — memory grows with the
//! number of *visited* tiles, not the size of the world.
//!
//! ## Tile coordinates
//!
//! Tile `(0, 0)` covers the world rectangle
//! `[0, tile_size * scale) × [0, tile_size * scale)`. Tile `(i, j)` covers
//! `[i * tile_world_size, (i + 1) * tile_world_size) × …`. Negative tile
//! indices extend the world in the `-x` / `-z` directions, so the world is
//! infinite in all four cardinal directions.
//!
//! ## Determinism
//!
//! Each tile is generated with a seed produced by [`derive_tile_seed`], a
//! fast 64-bit mix of the base seed and tile coordinates. Identical
//! `(base_seed, tile_x, tile_z)` always produce identical tiles, regardless
//! of which order tiles are requested in or how many tiles have been seen.
//!
//! ## Seams
//!
//! Per-tile generators (DiamondSquare, Voronoi, FbmNoise as currently
//! implemented) do not share state between tiles, so adjacent tiles will
//! generally have visible seams along their shared border. Callers that need
//! seamless terrain across tile boundaries should write a generator that
//! samples noise functions at world position rather than tile-local position.

use std::collections::HashMap;

use crate::{HeightMap, TerrainGenerator};

/// One generated tile within a [`TiledHeightMap`]. The `coord` is the integer
/// `(x, z)` tile index (not a world position); the `heightmap` holds the
/// generated heights for that tile.
#[derive(Debug, Clone)]
pub struct HeightMapTile {
    pub coord: (i32, i32),
    pub heightmap: HeightMap,
}

/// A sparse, infinite, on-demand-generated heightmap composed of fixed-size
/// tiles. Construct with [`TiledHeightMap::new`], request tiles via
/// [`TiledHeightMap::tile`] or [`TiledHeightMap::ensure_tile`], and free
/// memory with [`TiledHeightMap::evict_outside`].
pub struct TiledHeightMap {
    tile_size: usize,
    scale: f32,
    base_seed: u64,
    generator_factory: Box<dyn Fn(u64) -> Box<dyn TerrainGenerator>>,
    tiles: HashMap<(i32, i32), HeightMapTile>,
}

impl std::fmt::Debug for TiledHeightMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TiledHeightMap")
            .field("tile_size", &self.tile_size)
            .field("scale", &self.scale)
            .field("base_seed", &self.base_seed)
            .field("loaded_tiles", &self.tiles.len())
            .finish()
    }
}

impl TiledHeightMap {
    /// Build a new tiled heightmap.
    ///
    /// * `tile_size` — grid-cell side length of every tile (must be `> 0`).
    ///   Typical values are 128, 256, or 512.
    /// * `scale` — world units per cell (must be `> 0`).
    /// * `base_seed` — root seed; each tile's actual generator seed is mixed
    ///   from this value and the tile coordinates via [`derive_tile_seed`].
    /// * `generator_factory` — closure that returns a fresh
    ///   [`TerrainGenerator`] given a per-tile seed. Called once per tile, on
    ///   demand. Examples:
    ///   ```ignore
    ///   |seed| Box::new(DiamondSquare::new(seed, 0.6))
    ///   |seed| Box::new(FbmNoise::new(seed))
    ///   ```
    ///
    /// # Panics
    ///
    /// Panics if `tile_size == 0` or `scale <= 0.0`.
    pub fn new<F>(tile_size: usize, scale: f32, base_seed: u64, generator_factory: F) -> Self
    where
        F: Fn(u64) -> Box<dyn TerrainGenerator> + 'static,
    {
        assert!(tile_size > 0, "tile_size must be > 0");
        assert!(scale > 0.0, "scale must be positive");
        Self {
            tile_size,
            scale,
            base_seed,
            generator_factory: Box::new(generator_factory),
            tiles: HashMap::new(),
        }
    }

    /// Tile-cell side length.
    #[inline]
    pub fn tile_size(&self) -> usize {
        self.tile_size
    }

    /// World-space side length of one tile (`tile_size * scale`).
    #[inline]
    pub fn tile_world_size(&self) -> f32 {
        self.tile_size as f32 * self.scale
    }

    /// World units per cell.
    #[inline]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Number of tiles currently allocated in memory.
    #[inline]
    pub fn loaded_count(&self) -> usize {
        self.tiles.len()
    }

    /// Convert a world position to its containing tile coordinate.
    pub fn tile_for_world(&self, world_x: f32, world_z: f32) -> (i32, i32) {
        let tw = self.tile_world_size();
        let tx = (world_x / tw).floor() as i32;
        let tz = (world_z / tw).floor() as i32;
        (tx, tz)
    }

    /// Borrow a tile, generating it on demand if it is not already cached.
    pub fn tile(&mut self, coord: (i32, i32)) -> &HeightMapTile {
        if !self.tiles.contains_key(&coord) {
            self.generate_into_cache(coord);
        }
        self.tiles
            .get(&coord)
            .expect("tile inserted by generate_into_cache")
    }

    /// Generate a tile if it is not already cached, but do not return it.
    /// Useful for eagerly priming a region around a viewer.
    pub fn ensure_tile(&mut self, coord: (i32, i32)) {
        if !self.tiles.contains_key(&coord) {
            self.generate_into_cache(coord);
        }
    }

    /// Generate every tile within a Chebyshev radius of `centre`. Tiles
    /// already cached are left alone; new tiles are generated on demand.
    pub fn ensure_radius(&mut self, centre: (i32, i32), radius: i32) {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                self.ensure_tile((centre.0 + dx, centre.1 + dz));
            }
        }
    }

    /// Drop tiles outside a Chebyshev radius of `centre`. Returns the number
    /// of tiles evicted.
    pub fn evict_outside(&mut self, centre: (i32, i32), radius: i32) -> usize {
        let before = self.tiles.len();
        self.tiles.retain(|&(tx, tz), _| {
            let dx = (tx - centre.0).abs();
            let dz = (tz - centre.1).abs();
            dx <= radius && dz <= radius
        });
        before - self.tiles.len()
    }

    /// Look up a tile's heightmap without triggering generation. Returns
    /// `None` if the tile has never been requested.
    pub fn loaded_tile(&self, coord: (i32, i32)) -> Option<&HeightMapTile> {
        self.tiles.get(&coord)
    }

    /// Iterate over every currently-cached tile.
    pub fn loaded_tiles(&self) -> impl Iterator<Item = &HeightMapTile> {
        self.tiles.values()
    }

    /// Sample the heightmap at a world position, generating the surrounding
    /// tile on demand if needed.
    pub fn sample_height_at(&mut self, world_x: f32, world_z: f32) -> f32 {
        let coord = self.tile_for_world(world_x, world_z);
        let tile = self.tile(coord);
        let tw = tile.heightmap.world_width();
        let local_x = world_x - coord.0 as f32 * tw;
        let local_z = world_z - coord.1 as f32 * tw;
        tile.heightmap.get_height_at(local_x, local_z)
    }

    fn generate_into_cache(&mut self, coord: (i32, i32)) {
        let seed = derive_tile_seed(self.base_seed, coord.0, coord.1);
        let mut hm = HeightMap::new(self.tile_size, self.tile_size, self.scale);
        let generator = (self.generator_factory)(seed);
        generator.generate(&mut hm);
        self.tiles.insert(
            coord,
            HeightMapTile {
                coord,
                heightmap: hm,
            },
        );
    }
}

/// Mix a base seed and tile coordinates into a per-tile seed.
///
/// Uses SplitMix64 finalisation so that adjacent tiles produce wildly
/// different seeds, avoiding visual correlation between neighbours.
pub fn derive_tile_seed(base_seed: u64, tile_x: i32, tile_z: i32) -> u64 {
    // Pack the two i32s into a single u64; sign-bit handling falls out of
    // the cast since splitmix64 randomises the entire 64-bit state.
    let packed = ((tile_x as u32 as u64) << 32) | (tile_z as u32 as u64);
    splitmix64(base_seed.wrapping_add(splitmix64(packed)))
}

/// SplitMix64 finalisation function — fast, well-mixed, public domain.
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

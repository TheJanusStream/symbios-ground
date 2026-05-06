//! # symbios-ground
//!
//! An algorithmic terrain engine for procedural heightmap generation, erosion
//! simulation, and texture-weight (splat) mapping.
//!
//! ## Core types
//!
//! | Type | Description |
//! |------|-------------|
//! | [`HeightMap`] | 2-D grid of `f32` heights with world-space sampling helpers |
//! | [`TerrainGenerator`] | Trait implemented by all generators |
//! | [`SplatMapper`] / [`WeightMap`] | 4-channel RGBA texture-weight map from height + slope |
//!
//! ## Generators
//!
//! | Type | Algorithm |
//! |------|-----------|
//! | [`DiamondSquare`] | Classic fractal Diamond-Square; preserves caller dimensions via bilinear downsample |
//! | [`FbmNoise`] | Fractional Brownian Motion (multi-octave value noise) |
//! | [`VoronoiTerracing`] | Voronoi-based stepped plateaus |
//!
//! ## Erosion
//!
//! | Type | Description |
//! |------|-------------|
//! | [`HydraulicErosion`] | Droplet-based particle erosion |
//! | [`ThermalErosion`] | Talus/slope-smoothing erosion |
//!
//! ## Quick start
//!
//! ```rust
//! use symbios_ground::{DiamondSquare, HeightMap, HydraulicErosion,
//!                      SplatMapper, TerrainGenerator, ThermalErosion};
//!
//! // 1. Create a 129×129 heightmap with 1 world-unit per cell.
//! let mut heightmap = HeightMap::new(129, 129, 1.0);
//!
//! // 2. Generate fractal terrain.
//! DiamondSquare::new(42, 0.6).generate(&mut heightmap);
//!
//! // 3. Smooth with thermal erosion, then carve with hydraulic erosion.
//! ThermalErosion::new().erode(&mut heightmap);
//! HydraulicErosion::new(42).erode(&mut heightmap);
//!
//! // 4. Build a 4-channel texture weight map (grass / dirt / rock / snow).
//! let weight_map = SplatMapper::default().generate(&heightmap);
//!
//! // 5. Query world-space height and surface normal at any position.
//! let height = heightmap.get_height_at(64.5, 64.5);
//! let normal = heightmap.get_normal_at(64.5, 64.5);
//! # let _ = (height, normal, weight_map);
//! ```

pub mod erosion;
pub mod generator;
pub mod generators;
pub mod heightmap;
pub mod splat;
pub mod tile;

pub use erosion::{HydraulicErosion, ThermalErosion};
pub use generator::TerrainGenerator;
pub use generators::{DiamondSquare, FbmNoise, VoronoiTerracing};
pub use heightmap::{HeightMap, Lake};
pub use splat::{SplatMapper, SplatRule, WeightMap, sample_biome_at, sample_splat_weights_at};
pub use tile::{HeightMapTile, TiledHeightMap, derive_tile_seed};

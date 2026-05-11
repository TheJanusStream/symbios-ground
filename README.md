# symbios-ground

An algorithmic terrain engine for Rust. Provides procedural heightmap
generation, physically-inspired erosion simulation, and GPU-ready texture
weight (splat) mapping.

## Features

- **Three terrain generators** — Diamond-Square fractal, Fractional Brownian
  Motion value noise, Voronoi terracing
- **Two erosion simulations** — droplet-based hydraulic erosion (with lake
  pooling and river-delta deposition) and talus-based thermal erosion (with
  optional underwater rules)
- **Splat mapping** — 4-channel RGBA texture weight map keyed on height and
  slope, plus point-query helpers for sampling weights or the dominant biome
  at any world position
- **World-space queries** — bilinear height sampling and central-difference
  surface normals at any floating-point world coordinate; a cached per-grid
  normal table for hot scan-the-whole-map paths
- **Tile streaming** — `TiledHeightMap` generates sparse, infinite worlds
  tile-by-tile on demand, with deterministic per-tile seeding
- **Deterministic** — all generators are seeded with a `u64`; identical seeds
  always produce identical output
- **`serde` support** — `HeightMap` derives `Serialize` / `Deserialize`,
  including the lake list

## Quick start

```toml
# Cargo.toml
[dependencies]
symbios-ground = "0.3"
```

```rust
use symbios_ground::{
    DiamondSquare, HeightMap, HydraulicErosion,
    SplatMapper, TerrainGenerator, ThermalErosion,
};

// 1. Allocate a 129×129 heightmap (1 world-unit per cell).
let mut hm = HeightMap::new(129, 129, 1.0);

// 2. Fill with fractal Diamond-Square terrain.
DiamondSquare::new(42, 0.6).generate(&mut hm);

// 3. Smooth steep slopes with thermal erosion.
ThermalErosion::new()
    .with_iterations(100)
    .with_talus_angle(0.04)
    .erode(&mut hm);

// 4. Carve river valleys with hydraulic erosion.
HydraulicErosion::new(42).erode(&mut hm);

// 5. Generate a 4-channel texture weight map (grass/dirt/rock/snow).
let weights = SplatMapper::default().generate(&hm);

// 6. Query world-space height and surface normal at any position.
let h = hm.get_height_at(64.5, 64.5);
let n = hm.get_normal_at(64.5, 64.5); // unit [x, y, z], y = up
```

## API overview

### `HeightMap`

The central data structure. Stores a flat row-major `Vec<f32>` buffer of
`width × height` cells. `scale` is the world-unit size of each cell.

| Method | Description |
| ------ | ----------- |
| `new(w, h, scale)` | Allocate zeroed heightmap |
| `width()` / `height()` / `scale()` | Grid dimensions and world-units-per-cell |
| `get(x, z)` / `set(x, z, v)` | Grid-cell access |
| `get_mut(x, z)` | Mutable reference to a grid cell |
| `get_clamped(x, z)` | Grid access with edge clamping |
| `get_height_at(wx, wz)` | Bilinear world-space height sample |
| `get_normal_at(wx, wz)` | Central-difference surface normal (computed on the fly) |
| `normal_at_grid(x, z)` | Cached central-difference normal at a grid cell |
| `normals_grid()` | Borrow the lazily-populated row-major normal table |
| `lakes()` | Lakes detected by the last hydraulic erosion pass (empty otherwise) |
| `normalize()` | Rescale all values to `[0, 1]` |
| `data()` / `data_mut()` | Direct slice access |
| `world_width()` / `world_depth()` | `width * scale` / `height * scale` |

Mutating methods (`set`, `get_mut`, `data_mut`, `normalize`) invalidate the
cached normal table; the next read of `normal_at_grid` / `normals_grid`
recomputes it. The cache itself is skipped from `serde`, so deserialised
heightmaps always rebuild it from the current height data.

### `Lake`

A pooled body of standing water produced by `HydraulicErosion`. Stored on the
heightmap (`hm.lakes()`) and persisted across `serde` round-trips.

```rust
pub struct Lake {
    pub index: usize, // row-major index: z * width + x
    pub depth: f32,   // accumulated water depth at this cell
    pub area: f32,    // world-space area of this cell: scale * scale
}
```

### Generators

All generators implement `TerrainGenerator`:

```rust
pub trait TerrainGenerator {
    fn generate(&self, heightmap: &mut HeightMap);
}
```

#### `DiamondSquare`

Classic fractal subdivision. Generates internally at the smallest `2^n + 1`
square that covers the heightmap, then bilinearly downsamples to the
user-requested dimensions. Heightmaps of any size — square or rectangular,
power-of-two or not — are preserved as the caller allocated them.

```rust
DiamondSquare::new(seed, roughness)
// roughness: 0.4 = smooth, 0.8 = jagged
```

##### Generator dimension constraints

| Generator | Accepted dimensions |
| --------- | ------------------- |
| `DiamondSquare` | Any `width × height ≥ 1×1`; internally rounds up to `2^n + 1` and downsamples |
| `FbmNoise` | Any `width × height ≥ 1×1` |
| `VoronoiTerracing` | Any `width × height ≥ 1×1` |

#### `FbmNoise`

Multi-octave value noise with quintic smoothstep interpolation. Builder API:

```rust
FbmNoise::new(seed)
    .with_octaves(8)        // 1–32; default 6; more = finer detail
    .with_persistence(0.5)  // amplitude decay per octave (default)

// Additional public fields for advanced tuning:
// fbm.lacunarity     — frequency multiplier per octave (default 2.0)
// fbm.base_frequency — world-space frequency of the first octave (default 1.0)
```

#### `VoronoiTerracing`

Distributes random seed points, assigns each cell to its nearest seed, and
quantises heights into discrete terraces.

```rust
VoronoiTerracing::new(seed, num_seeds, num_terraces)
// e.g. ::new(1, 50, 8)  →  50 regions, 8 terrace levels
```

### Erosion

Erosion modifies a `HeightMap` in-place via `.erode(&mut hm)`.

#### `ThermalErosion`

Iterative slope-smoothing. Material on slopes steeper than `talus_angle` slides
to downhill neighbours.

```rust
ThermalErosion::new()
    .with_iterations(50)
    .with_talus_angle(0.05)
    .with_water_level(0.2)            // material below this is "underwater"
    .with_underwater_talus_angle(0.1) // applies only when BOTH adjacent cells are underwater
    .erode(&mut hm);

// The `fraction` field (default 0.25) controls how much excess material
// is transferred per iteration. Clamped to (0.0, 0.25] internally.
```

Note: `underwater_talus_angle` only kicks in when *both* the current cell and
its neighbour are `≤ water_level`; a shoreline cell next to a dry cell still
uses the normal `talus_angle`.

#### `HydraulicErosion`

Particle simulation. Each droplet flows downhill, eroding and depositing
sediment to carve valleys and ridges. When a droplet stalls (velocity drops
below `vel_threshold`, or water evaporates) it is resolved as one of:

- **Lake pooling** — when the stall happens above `water_level`, the
  droplet's remaining water is splayed bilinearly over the surrounding cells
  and accumulated into a sparse list of [`Lake`](#lake) entries, written to
  `hm.lakes()`.
- **River-delta fan** — when the stall happens at or below `water_level`, the
  droplet's remaining sediment is distributed over a `delta_radius` square
  kernel with a Gaussian-like falloff, simulating the splay of a delta.

```rust
HydraulicErosion::new(seed).erode(&mut hm);

// Inspect pooled lakes:
for lake in hm.lakes() {
    let z = lake.index / hm.width();
    let x = lake.index % hm.width();
    println!("lake at ({x}, {z}) depth={} area={}", lake.depth, lake.area);
}

// Fine-tune via public fields:
let mut eroder = HydraulicErosion::new(seed);
eroder.num_drops = 100_000;
eroder.erosion_rate = 0.4;
eroder.water_level = 0.2;   // heights ≤ this force deposition / delta fans
eroder.vel_threshold = 0.05; // stalled droplets pool into lakes or deltas
eroder.delta_radius = 1;     // 3×3 fan; 2 → 5×5, etc.
eroder.erode(&mut hm);
```

### `SplatMapper` / `WeightMap`

Produces a 4-channel RGBA weight map for GPU terrain shaders. Each pixel's
channels sum to ~255.

| Channel | Default layer | Conditions |
| ------- | ------------- | ----------- |
| R | Grass | Low altitude, gentle slope |
| G | Dirt | Mid altitude, any slope |
| B | Rock | Steep slopes |
| A | Snow | High altitude, gentle slope |

```rust
// Default grass/dirt/rock/snow preset:
let wm = SplatMapper::default().generate(&hm);

// Custom rules:
use symbios_ground::{SplatMapper, SplatRule};
let mapper = SplatMapper::new([
    SplatRule::new((0.0, 0.4), (0.0, 0.25), 4.0), // R: grass
    SplatRule::new((0.3, 0.6), (0.0, 0.6),  2.0), // G: dirt
    SplatRule::new((0.0, 1.0), (0.2, 1.0),  3.0), // B: rock
    SplatRule::new((0.65, 1.0),(0.0, 0.3),  4.0), // A: snow
]);
let wm = mapper.generate(&hm);

// wm.data: Vec<[u8; 4]>, row-major, wm.width × wm.height pixels
```

#### World-space point queries

Most callers want a full `WeightMap` covering the grid, but for tasks like
gameplay queries ("what biome is the player standing on?") it is cheaper to
ask for a single point. `SplatMapper` provides:

```rust
use symbios_ground::{SplatMapper, sample_biome_at, sample_splat_weights_at};

let mapper = SplatMapper::default();

// Normalised four-channel weights at a world position. Sums to 1.0 unless
// no rule applies, in which case the rock-channel fallback [0, 0, 1, 0]
// is returned — matching the [0, 0, 255, 0] fallback used by `generate`.
let weights: [f32; 4] = mapper.sample_weights_at(&hm, 12.5, 7.25);

// Dominant channel index (0 = R, 1 = G, 2 = B, 3 = A). Ties go to the
// lowest channel index.
let biome: u8 = mapper.sample_biome_at(&hm, 12.5, 7.25);

// Free-function form for callers that prefer it:
let _ = sample_splat_weights_at(&hm, &mapper, 12.5, 7.25);
let _ = sample_biome_at(&hm, &mapper, 12.5, 7.25);
```

### `TiledHeightMap` (LOD / streaming)

Sparse, infinite-world heightmap composed of fixed-size tiles. Tiles are
generated on demand from a base seed plus their `(x, z)` coordinates,
keeping memory proportional to the number of *visited* tiles rather than the
size of the world. See [`examples/streaming_tiles.rs`](examples/streaming_tiles.rs)
for a viewer-driven streaming demo.

```rust
use symbios_ground::{FbmNoise, TiledHeightMap};

let mut world = TiledHeightMap::new(256, 1.0, 1234, |seed| Box::new(FbmNoise::new(seed)));
world.ensure_radius((0, 0), 1);            // pre-load a 3×3 region around origin
let h = world.sample_height_at(120.0, 4.0); // generate underlying tile if absent
world.evict_outside((0, 0), 2);            // drop tiles outside Chebyshev radius 2
```

Per-tile generators are independent, so adjacent tiles will generally have
visible seams. Callers needing seamless terrain across boundaries should
write a generator that samples noise at world-space coordinates rather than
tile-local coordinates.

## Running the example

```sh
cargo run --example streaming_tiles
```

## Running benchmarks

```sh
cargo bench
```

Criterion benchmarks for `get_height_at`, `get_normal_at`, `DiamondSquare`,
and `FbmNoise` are in [`benches/bench_main.rs`](benches/bench_main.rs).

## License

MIT — see [LICENSE](LICENSE).

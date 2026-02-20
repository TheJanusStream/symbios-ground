# symbios-ground

An algorithmic terrain engine for Rust. Provides procedural heightmap
generation, physically-inspired erosion simulation, and GPU-ready texture
weight (splat) mapping.

## Features

- **Three terrain generators** — Diamond-Square fractal, Fractional Brownian
  Motion value noise, Voronoi terracing
- **Two erosion simulations** — droplet-based hydraulic erosion, talus-based
  thermal erosion
- **Splat mapping** — 4-channel RGBA texture weight map keyed on height and
  slope
- **World-space queries** — bilinear height sampling and central-difference
  surface normals at any floating-point world coordinate
- **Deterministic** — all generators are seeded with a `u64`; identical seeds
  always produce identical output
- **`serde` support** — `HeightMap` derives `Serialize` / `Deserialize`

## Quick start

```toml
# Cargo.toml
[dependencies]
symbios-ground = "0.1"
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
|--------|-------------|
| `new(w, h, scale)` | Allocate zeroed heightmap |
| `get(x, z)` / `set(x, z, v)` | Grid-cell access |
| `get_clamped(x, z)` | Grid access with edge clamping |
| `get_height_at(wx, wz)` | Bilinear world-space height sample |
| `get_normal_at(wx, wz)` | Central-difference surface normal |
| `normalize()` | Rescale all values to `[0, 1]` |
| `data()` / `data_mut()` | Direct slice access |
| `world_width()` / `world_depth()` | `width * scale` / `height * scale` |

### Generators

All generators implement `TerrainGenerator`:

```rust
pub trait TerrainGenerator {
    fn generate(&self, heightmap: &mut HeightMap);
}
```

#### `DiamondSquare`

Classic fractal subdivision. Resizes the heightmap to the smallest `2^n + 1`
that covers its current dimensions (e.g. a 100×100 map becomes 129×129).

```rust
DiamondSquare::new(seed, roughness)
// roughness: 0.4 = smooth, 0.8 = jagged
```

#### `FbmNoise`

Multi-octave value noise with quintic smoothstep interpolation. Builder API:

```rust
FbmNoise::new(seed)
    .with_octaves(8)        // 1–32; more = finer detail
    .with_persistence(0.5)  // amplitude decay per octave
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
    .erode(&mut hm);
```

#### `HydraulicErosion`

Particle simulation. Each droplet flows downhill, eroding and depositing
sediment to carve valleys and ridges.

```rust
HydraulicErosion::new(seed).erode(&mut hm);

// Fine-tune via public fields:
let mut eroder = HydraulicErosion::new(seed);
eroder.num_drops = 100_000;
eroder.erosion_rate = 0.4;
eroder.erode(&mut hm);
```

### `SplatMapper` / `WeightMap`

Produces a 4-channel RGBA weight map for GPU terrain shaders. Each pixel's
channels sum to ~255.

| Channel | Default layer | Conditions |
|---------|--------------|------------|
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

## Running benchmarks

```sh
cargo bench
```

Criterion benchmarks for `get_height_at`, `get_normal_at`, `DiamondSquare`,
and `FbmNoise` are in [`benches/bench_main.rs`](benches/bench_main.rs).

## License

MIT — see [LICENSE](LICENSE).

use rand::Rng;
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;

use crate::{HeightMap, TerrainGenerator};

/// Voronoi-based terracing generator.
///
/// Places random seed points across the heightmap and assigns each cell a
/// quantised height based on which seed it is closest to, creating the
/// characteristic stepped plateau landscape of Voronoi terracing.
///
/// Nearest-seed queries are accelerated with a uniform spatial grid, reducing
/// complexity from O(N·M·S) to approximately O(N·M·√S).
#[derive(Debug, Clone)]
pub struct VoronoiTerracing {
    pub seed: u64,
    /// Number of Voronoi seed points.
    pub num_seeds: usize,
    /// Number of discrete terrace height levels.
    pub num_terraces: usize,
}

impl VoronoiTerracing {
    /// Create a new `VoronoiTerracing` generator.
    ///
    /// * `seed` — RNG seed for reproducible placement of seed points.
    /// * `num_seeds` — number of Voronoi seed points; more seeds produce
    ///   smaller, more fragmented plateaus. Must be `>= 1`.
    /// * `num_terraces` — number of discrete height levels; `1` gives a flat
    ///   map, higher values produce finer staircase detail. Must be `>= 1`.
    ///
    /// # Panics
    ///
    /// Panics if `num_seeds == 0` or `num_terraces == 0`.
    pub fn new(seed: u64, num_seeds: usize, num_terraces: usize) -> Self {
        assert!(num_seeds > 0, "num_seeds must be > 0");
        assert!(num_terraces > 0, "num_terraces must be > 0");
        Self {
            seed,
            num_seeds,
            num_terraces,
        }
    }
}

impl TerrainGenerator for VoronoiTerracing {
    fn generate(&self, heightmap: &mut HeightMap) {
        let mut rng = Pcg64Mcg::seed_from_u64(self.seed);

        // Generate seed points in normalised [0, 1] space.
        let seeds: Vec<(f32, f32)> = (0..self.num_seeds)
            .map(|_| (rng.random::<f32>(), rng.random::<f32>()))
            .collect();

        // Assign each seed a terrace height level.
        let seed_heights: Vec<f32> = (0..self.num_seeds)
            .map(|i| i as f32 / (self.num_seeds - 1).max(1) as f32)
            .collect();

        // Build a uniform spatial grid to accelerate nearest-seed queries.
        // Grid side length ≈ √S gives ~1 seed per cell on average.
        let grid_size = ((self.num_seeds as f64).sqrt().ceil() as usize).max(1);
        let cell_w = 1.0_f32 / grid_size as f32;
        let mut grid: Vec<Vec<usize>> = vec![vec![]; grid_size * grid_size];
        for (i, &(sx, sz)) in seeds.iter().enumerate() {
            let gx = ((sx / cell_w) as usize).min(grid_size - 1);
            let gz = ((sz / cell_w) as usize).min(grid_size - 1);
            grid[gz * grid_size + gx].push(i);
        }

        let w = heightmap.width();
        let h = heightmap.height();

        for z in 0..h {
            for x in 0..w {
                let nx = x as f32 / w as f32;
                let nz = z as f32 / h as f32;

                let cx = ((nx / cell_w) as usize).min(grid_size - 1);
                let cz = ((nz / cell_w) as usize).min(grid_size - 1);

                let mut best_dist = f32::MAX;
                let mut nearest = 0usize;

                // Expand outward in Chebyshev rings until the minimum possible
                // distance to the next ring exceeds our best distance so far.
                // Stopping bound: any point inside the center cell is at least
                // (r-1)*cell_w from cells at Chebyshev distance r, so
                // ((r-1)*cell_w)² is a valid lower bound on ring-r distances.
                for r in 0..=(grid_size as i32) {
                    let min_ring_dist_sq = if r == 0 {
                        0.0f32
                    } else {
                        let d = (r - 1) as f32 * cell_w;
                        d * d
                    };

                    if min_ring_dist_sq > best_dist {
                        break;
                    }

                    for dz in -r..=r {
                        for dx in -r..=r {
                            // Only visit cells on the border of this ring.
                            if dx.abs() < r && dz.abs() < r {
                                continue;
                            }
                            let gx = cx as i32 + dx;
                            let gz = cz as i32 + dz;
                            if gx < 0 || gz < 0 || gx >= grid_size as i32 || gz >= grid_size as i32
                            {
                                continue;
                            }
                            for &si in &grid[gz as usize * grid_size + gx as usize] {
                                let (sx, sz) = seeds[si];
                                let d = (sx - nx).powi(2) + (sz - nz).powi(2);
                                if d < best_dist {
                                    best_dist = d;
                                    nearest = si;
                                }
                            }
                        }
                    }
                }

                // Quantise to the nearest terrace level.
                // Clamp the floor result to num_terraces-1 so that raw==1.0
                // doesn't produce a spurious (num_terraces+1)-th level.
                let raw = seed_heights[nearest];
                let terraced = (raw * self.num_terraces as f32)
                    .floor()
                    .min(self.num_terraces as f32 - 1.0)
                    / self.num_terraces as f32;

                heightmap.set(x, z, terraced);
            }
        }
    }
}

use rand::Rng;
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;

use crate::{HeightMap, TerrainGenerator};

/// Voronoi-based terracing generator.
///
/// Places random seed points across the heightmap and assigns each cell a
/// quantised height based on which seed it is closest to, creating the
/// characteristic stepped plateau landscape of Voronoi terracing.
#[derive(Debug, Clone)]
pub struct VoronoiTerracing {
    pub seed: u64,
    /// Number of Voronoi seed points.
    pub num_seeds: usize,
    /// Number of discrete terrace height levels.
    pub num_terraces: usize,
}

impl VoronoiTerracing {
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

        let w = heightmap.width as f32;
        let h = heightmap.height as f32;

        for z in 0..heightmap.height {
            for x in 0..heightmap.width {
                let nx = x as f32 / w;
                let nz = z as f32 / h;

                // Find the nearest seed (squared distance is sufficient for comparison).
                let nearest = seeds
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        let da = (a.0 - nx).powi(2) + (a.1 - nz).powi(2);
                        let db = (b.0 - nx).powi(2) + (b.1 - nz).powi(2);
                        da.partial_cmp(&db).unwrap()
                    })
                    .map(|(idx, _)| idx)
                    .unwrap_or(0);

                // Quantise to the nearest terrace level.
                let raw = seed_heights[nearest];
                let terraced = (raw * self.num_terraces as f32).floor() / self.num_terraces as f32;

                heightmap.set(x, z, terraced);
            }
        }
    }
}

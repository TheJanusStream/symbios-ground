use rand::Rng;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_pcg::Pcg64Mcg;

use crate::{HeightMap, TerrainGenerator};

/// Fractional Brownian Motion (multi-octave value noise) terrain generator.
///
/// Stacks multiple octaves of smooth value noise, each with half the amplitude
/// and double the frequency of the previous, producing natural fractal terrain.
#[derive(Debug, Clone)]
pub struct FbmNoise {
    pub seed: u64,
    /// Number of noise octaves to stack. Capped at 32 to prevent hangs.
    pub octaves: u32,
    /// Amplitude scale per octave (e.g. 0.5 = each octave half as tall).
    pub persistence: f32,
    /// Frequency scale per octave (e.g. 2.0 = each octave twice as dense).
    pub lacunarity: f32,
    /// World-space frequency of the first octave (lower = broader features).
    pub base_frequency: f32,
}

impl FbmNoise {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            octaves: 6,
            persistence: 0.5,
            lacunarity: 2.0,
            base_frequency: 1.0,
        }
    }

    pub fn with_octaves(mut self, octaves: u32) -> Self {
        assert!(
            (1..=32).contains(&octaves),
            "octaves must be in [1, 32], got {octaves}"
        );
        self.octaves = octaves;
        self
    }

    pub fn with_persistence(mut self, persistence: f32) -> Self {
        self.persistence = persistence;
        self
    }
}

// ---------------------------------------------------------------------------
// Internal value-noise implementation
// ---------------------------------------------------------------------------

struct ValueNoise {
    values: [f32; 256],
    perm: [u8; 512], // doubled permutation table for fast wrapping
}

impl ValueNoise {
    fn new(seed: u64) -> Self {
        let mut rng = Pcg64Mcg::seed_from_u64(seed);

        let mut raw_perm: Vec<u8> = (0..=255u8).collect();
        raw_perm.shuffle(&mut rng);

        let mut perm = [0u8; 512];
        perm[..256].copy_from_slice(&raw_perm);
        perm[256..].copy_from_slice(&raw_perm);

        // Use the rng bytes directly for the value table via bit manipulation
        // to avoid the deprecated gen_range.
        let mut values = [0.0f32; 256];
        for v in values.iter_mut() {
            // random::<u32>() is the non-deprecated API in rand 0.9
            let bits: u32 = rng.random();
            // Map to [0, 1) via the upper 23 mantissa bits
            *v = (bits >> 9) as f32 / (1u32 << 23) as f32;
        }

        Self { values, perm }
    }

    #[inline]
    fn hash(&self, ix: i32, iz: i32) -> f32 {
        let xi = ix.rem_euclid(256) as usize;
        let zi = iz.rem_euclid(256) as usize;
        let idx = self.perm[self.perm[xi] as usize + zi] as usize;
        self.values[idx]
    }

    /// Sample smooth value noise at (x, z).
    fn sample(&self, x: f32, z: f32) -> f32 {
        let x0 = x.floor() as i32;
        let z0 = z.floor() as i32;
        let fx = x - x0 as f32;
        let fz = z - z0 as f32;

        // Quintic smoothstep
        let u = fx * fx * fx * (fx * (fx * 6.0 - 15.0) + 10.0);
        let v = fz * fz * fz * (fz * (fz * 6.0 - 15.0) + 10.0);

        let v00 = self.hash(x0, z0);
        let v10 = self.hash(x0 + 1, z0);
        let v01 = self.hash(x0, z0 + 1);
        let v11 = self.hash(x0 + 1, z0 + 1);

        let a = v00 + (v10 - v00) * u;
        let b = v01 + (v11 - v01) * u;
        a + (b - a) * v
    }
}

impl TerrainGenerator for FbmNoise {
    fn generate(&self, heightmap: &mut HeightMap) {
        assert!(
            (1..=32).contains(&self.octaves),
            "octaves must be in [1, 32], got {}",
            self.octaves
        );

        let noise = ValueNoise::new(self.seed);

        let mut max_amp = 0.0_f32;
        let mut amp = 1.0_f32;
        for _ in 0..self.octaves {
            max_amp += amp;
            amp *= self.persistence;
        }

        let w = heightmap.width();
        let h = heightmap.height();

        for z in 0..h {
            for x in 0..w {
                let nx = x as f32 / w as f32 * self.base_frequency;
                let nz = z as f32 / h as f32 * self.base_frequency;

                let mut height = 0.0_f32;
                let mut frequency = 1.0_f32;
                let mut amplitude = 1.0_f32;

                for _ in 0..self.octaves {
                    height += noise.sample(nx * frequency, nz * frequency) * amplitude;
                    amplitude *= self.persistence;
                    frequency *= self.lacunarity;
                }

                heightmap.set(x, z, height / max_amp);
            }
        }
    }
}

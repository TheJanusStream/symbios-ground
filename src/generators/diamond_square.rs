use rand::Rng;
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;

use crate::{HeightMap, TerrainGenerator};

/// Classic Diamond-Square fractal terrain generator.
///
/// Internally generates at the smallest `2^n + 1` square that covers the
/// user's heightmap, then bilinearly downsamples to the requested dimensions.
/// Heightmaps of any size (including non-square and non-power-of-two) are
/// preserved as the caller requested them.
#[derive(Debug, Clone)]
pub struct DiamondSquare {
    /// Random seed for reproducibility.
    pub seed: u64,
    /// Roughness factor in `[0.0, 1.0]`. Higher = more jagged.
    pub roughness: f32,
}

impl DiamondSquare {
    /// Create a new `DiamondSquare` generator.
    ///
    /// * `seed` — RNG seed for reproducibility.
    /// * `roughness` — amplitude decay per subdivision step; typical values are
    ///   `0.4` (smooth) to `0.8` (jagged). Values outside `[0, 1]` are accepted
    ///   but may produce extreme terrain; very large values clamp safely via the
    ///   `is_finite` guard inside `generate`.
    pub fn new(seed: u64, roughness: f32) -> Self {
        Self { seed, roughness }
    }

    /// Returns the smallest `2^n + 1 >= n`.
    ///
    /// Uses `checked_shl` so an unreasonably large `n` panics with a clear
    /// message instead of shifting the bit out and looping forever.
    fn required_size(n: usize) -> usize {
        if n <= 2 {
            return 2;
        }
        let mut power = 1usize;
        loop {
            if power + 1 >= n {
                return power + 1;
            }
            power = power.checked_shl(1).expect(
                "HeightMap dimension too large for DiamondSquare (max 2^(usize::BITS-1)+1)",
            );
        }
    }

    /// Run the Diamond-Square algorithm into a square `size × size` flat
    /// row-major buffer. `size` is expected to satisfy `size = 2^n + 1`.
    fn generate_square(&self, size: usize) -> Vec<f32> {
        let mut data = vec![0.0_f32; size * size];
        let mut rng = Pcg64Mcg::seed_from_u64(self.seed);

        macro_rules! get {
            ($x:expr, $z:expr) => {
                data[$z * size + $x]
            };
        }
        macro_rules! set {
            ($x:expr, $z:expr, $v:expr) => {
                data[$z * size + $x] = $v;
            };
        }

        set!(0, 0, rng.random_range(0.0_f32..1.0));
        set!(size - 1, 0, rng.random_range(0.0_f32..1.0));
        set!(0, size - 1, rng.random_range(0.0_f32..1.0));
        set!(size - 1, size - 1, rng.random_range(0.0_f32..1.0));

        let mut step = size - 1;
        let mut amp = self.roughness;

        while step >= 2 {
            let half = step / 2;

            // Diamond step: fill center of each square.
            let mut z = 0;
            while z < size - 1 {
                let mut x = 0;
                while x < size - 1 {
                    let avg = (get!(x, z)
                        + get!(x + step, z)
                        + get!(x, z + step)
                        + get!(x + step, z + step))
                        / 4.0;
                    let offset = if amp > 0.0 && amp.is_finite() {
                        rng.random_range(-amp..amp)
                    } else {
                        0.0
                    };
                    set!(x + half, z + half, avg + offset);
                    x += step;
                }
                z += step;
            }

            // Square step: fill edge midpoints of each diamond.
            let mut z = 0;
            while z < size {
                let x_start = if (z / half).is_multiple_of(2) {
                    half
                } else {
                    0
                };
                let mut x = x_start;
                while x < size {
                    let mut sum = 0.0_f32;
                    let mut count = 0u32;
                    if z >= half {
                        sum += get!(x, z - half);
                        count += 1;
                    }
                    if z + half < size {
                        sum += get!(x, z + half);
                        count += 1;
                    }
                    if x >= half {
                        sum += get!(x - half, z);
                        count += 1;
                    }
                    if x + half < size {
                        sum += get!(x + half, z);
                        count += 1;
                    }
                    let offset = if amp > 0.0 && amp.is_finite() {
                        rng.random_range(-amp..amp)
                    } else {
                        0.0
                    };
                    set!(x, z, sum / count as f32 + offset);
                    x += step;
                }
                z += half;
            }

            step = half;
            amp *= 0.5_f32.powf(1.0 - self.roughness + 0.5);
        }

        data
    }
}

impl TerrainGenerator for DiamondSquare {
    fn generate(&self, heightmap: &mut HeightMap) {
        let user_w = heightmap.width();
        let user_h = heightmap.height();
        let internal_size = Self::required_size(user_w.max(user_h));

        let src = self.generate_square(internal_size);

        // Bilinearly resample from the internal `internal_size × internal_size`
        // grid to the user's `user_w × user_h` heightmap. When user_w (or user_h)
        // equals the internal size this is a 1:1 copy; smaller sizes downsample,
        // larger non-power-of-two sizes are not supported (internal is always
        // ≥ user dim, so this path only handles ≤).
        let max_src_x = (internal_size - 1) as f32;
        let max_src_z = (internal_size - 1) as f32;
        let scale_x = if user_w > 1 {
            max_src_x / (user_w - 1) as f32
        } else {
            0.0
        };
        let scale_z = if user_h > 1 {
            max_src_z / (user_h - 1) as f32
        } else {
            0.0
        };

        let dst = heightmap.data_mut();
        for z in 0..user_h {
            let sz = z as f32 * scale_z;
            let z0 = sz.floor() as usize;
            let z1 = (z0 + 1).min(internal_size - 1);
            let fz = sz - z0 as f32;

            for x in 0..user_w {
                let sx = x as f32 * scale_x;
                let x0 = sx.floor() as usize;
                let x1 = (x0 + 1).min(internal_size - 1);
                let fx = sx - x0 as f32;

                let h00 = src[z0 * internal_size + x0];
                let h10 = src[z0 * internal_size + x1];
                let h01 = src[z1 * internal_size + x0];
                let h11 = src[z1 * internal_size + x1];

                let h0 = h00 + (h10 - h00) * fx;
                let h1 = h01 + (h11 - h01) * fx;
                dst[z * user_w + x] = h0 + (h1 - h0) * fz;
            }
        }

        heightmap.normalize();
    }
}

use rand::Rng;
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;

use crate::{HeightMap, TerrainGenerator};

/// Classic Diamond-Square fractal terrain generator.
///
/// Produces natural-looking heightmaps with configurable roughness.
/// Resizes the heightmap to the smallest `2^n + 1` that fits its current dimensions.
#[derive(Debug, Clone)]
pub struct DiamondSquare {
    /// Random seed for reproducibility.
    pub seed: u64,
    /// Roughness factor in `[0.0, 1.0]`. Higher = more jagged.
    pub roughness: f32,
}

impl DiamondSquare {
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
}

impl TerrainGenerator for DiamondSquare {
    fn generate(&self, heightmap: &mut HeightMap) {
        let size = Self::required_size(heightmap.width().max(heightmap.height()));

        heightmap.reinitialize(size, size);

        let mut rng = Pcg64Mcg::seed_from_u64(self.seed);

        // All algorithm work is done through a direct slice borrow so the
        // borrow ends before `heightmap.normalize()` is called.
        {
            let data = heightmap.data_mut();

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

            // Seed the four corners.
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
                        set!(x + half, z + half, avg + rng.random_range(-amp..amp));
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
                        set!(x, z, sum / count as f32 + rng.random_range(-amp..amp));
                        x += step;
                    }
                    z += half;
                }

                step = half;
                amp *= 0.5_f32.powf(1.0 - self.roughness + 0.5);
            }
        } // data borrow released here

        heightmap.normalize();
    }
}

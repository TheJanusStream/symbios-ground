use rand::Rng;
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;

use crate::HeightMap;

/// Droplet-based hydraulic erosion simulation.
///
/// Simulates individual water droplets flowing downhill, picking up and
/// depositing sediment to carve realistic river valleys and ridges.
#[derive(Debug, Clone)]
pub struct HydraulicErosion {
    pub seed: u64,
    /// Number of simulated droplets.
    pub num_drops: u32,
    /// Maximum steps a droplet travels before evaporating.
    pub max_steps: u32,
    /// How strongly the droplet follows its previous direction vs. the slope.
    /// `0.0` = pure gradient descent, `1.0` = no turning.
    pub inertia: f32,
    /// Fraction of erodible material picked up per step.
    pub erosion_rate: f32,
    /// Fraction of carried sediment deposited per step when over capacity.
    pub deposition_rate: f32,
    /// Fraction of water that evaporates per step. Must be in `[0.0, 1.0]`.
    pub evaporation_rate: f32,
    /// Scales the sediment capacity of a droplet.
    pub capacity_factor: f32,
    /// Minimum slope used for capacity calculation (avoids division by zero).
    pub min_slope: f32,
    /// Height threshold below which droplets will deposit sediment instead of eroding.
    pub water_level: f32,
}

impl HydraulicErosion {
    /// Create a `HydraulicErosion` simulator with sensible defaults:
    /// 50 000 droplets, 64 max steps, inertia 0.05, erosion/deposition rates
    /// 0.3, evaporation rate 0.02, capacity factor 8.0, min slope 0.01.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            num_drops: 50_000,
            max_steps: 64,
            inertia: 0.05,
            erosion_rate: 0.3,
            deposition_rate: 0.3,
            evaporation_rate: 0.02,
            capacity_factor: 8.0,
            min_slope: 0.01,
            water_level: 0.0,
        }
    }

    /// Apply erosion to `heightmap` in-place.
    pub fn erode(&self, heightmap: &mut HeightMap) {
        let mut rng = Pcg64Mcg::seed_from_u64(self.seed);
        let w = heightmap.width();
        let h = heightmap.height();

        for _ in 0..self.num_drops {
            // Spawn droplet at a random position.
            let mut px: f32 = rng.random::<f32>() * (w - 1) as f32;
            let mut pz: f32 = rng.random::<f32>() * (h - 1) as f32;
            let mut dir_x = 0.0_f32;
            let mut dir_z = 0.0_f32;
            let mut vel = 1.0_f32;
            let mut water = 1.0_f32;
            let mut sediment = 0.0_f32;

            for _ in 0..self.max_steps {
                let ix = px.floor() as usize;
                let iz = pz.floor() as usize;

                // Bail if we're out of bounds.
                if ix + 1 >= w || iz + 1 >= h {
                    break;
                }

                let fx = px - ix as f32;
                let fz = pz - iz as f32;

                // Sample the four surrounding heights.
                let h00 = heightmap.get(ix, iz);
                let h10 = heightmap.get(ix + 1, iz);
                let h01 = heightmap.get(ix, iz + 1);
                let h11 = heightmap.get(ix + 1, iz + 1);

                // Bilinear height at current position.
                let height_here = h00 * (1.0 - fx) * (1.0 - fz)
                    + h10 * fx * (1.0 - fz)
                    + h01 * (1.0 - fx) * fz
                    + h11 * fx * fz;

                // Bilinear gradient.
                let grad_x = (h10 - h00) * (1.0 - fz) + (h11 - h01) * fz;
                let grad_z = (h01 - h00) * (1.0 - fx) + (h11 - h10) * fx;

                // Update direction: blend previous with gradient.
                dir_x = dir_x * self.inertia - grad_x * (1.0 - self.inertia);
                dir_z = dir_z * self.inertia - grad_z * (1.0 - self.inertia);

                // Normalise direction.
                let len = (dir_x * dir_x + dir_z * dir_z).sqrt();
                if len < f32::EPSILON {
                    break;
                }
                dir_x /= len;
                dir_z /= len;

                // Move droplet.
                let new_px = px + dir_x;
                let new_pz = pz + dir_z;

                if !new_px.is_finite()
                    || !new_pz.is_finite()
                    || new_px < 0.0
                    || new_px >= (w - 1) as f32
                    || new_pz < 0.0
                    || new_pz >= (h - 1) as f32
                {
                    break;
                }

                // Height at new position (use grid sample for speed).
                let new_ix = new_px.floor() as usize;
                let new_iz = new_pz.floor() as usize;
                let new_fx = new_px - new_ix as f32;
                let new_fz = new_pz - new_iz as f32;
                let nh00 = heightmap.get(new_ix, new_iz);
                let nh10 = heightmap.get((new_ix + 1).min(w - 1), new_iz);
                let nh01 = heightmap.get(new_ix, (new_iz + 1).min(h - 1));
                let nh11 = heightmap.get((new_ix + 1).min(w - 1), (new_iz + 1).min(h - 1));
                let height_new = nh00 * (1.0 - new_fx) * (1.0 - new_fz)
                    + nh10 * new_fx * (1.0 - new_fz)
                    + nh01 * (1.0 - new_fx) * new_fz
                    + nh11 * new_fx * new_fz;

                let delta_h = height_new - height_here;

                // Sediment capacity proportional to speed, water, and slope.
                // Clamp to >= 0 so a misconfigured evaporation_rate > 1 cannot
                // make water negative and invert the capacity formula.
                let capacity = if height_here <= self.water_level {
                    0.0 // Force deposition (River Delta effect)
                } else {
                    let slope = (-delta_h).max(self.min_slope);
                    (slope * vel * water * self.capacity_factor).max(0.0)
                };

                // Bilinear weights for the four surrounding cells at old pos.
                let w00 = (1.0 - fx) * (1.0 - fz);
                let w10 = fx * (1.0 - fz);
                let w01 = (1.0 - fx) * fz;
                let w11 = fx * fz;

                if sediment > capacity || delta_h > 0.0 {
                    // Deposit sediment.
                    let deposit = if delta_h > 0.0 {
                        delta_h.min(sediment)
                    } else {
                        (sediment - capacity) * self.deposition_rate
                    };
                    sediment -= deposit;

                    // Spread deposit over the four surrounding cells.
                    // The bounds check `ix+1 >= w || iz+1 >= h` above ensures
                    // all four indices are valid; direct indexing is safe.
                    let data = heightmap.data_mut();
                    data[iz * w + ix] += deposit * w00;
                    data[iz * w + ix + 1] += deposit * w10;
                    data[(iz + 1) * w + ix] += deposit * w01;
                    data[(iz + 1) * w + ix + 1] += deposit * w11;
                } else {
                    // Erode from the four surrounding cells.
                    let erode = ((capacity - sediment) * self.erosion_rate)
                        .min(-delta_h)
                        .max(0.0);
                    sediment += erode;

                    let data = heightmap.data_mut();
                    data[iz * w + ix] -= erode * w00;
                    data[iz * w + ix + 1] -= erode * w10;
                    data[(iz + 1) * w + ix] -= erode * w01;
                    data[(iz + 1) * w + ix + 1] -= erode * w11;
                }

                // Update speed and water. Clamp water to >= 0 so that a
                // user-supplied evaporation_rate > 1.0 does not make water
                // negative and corrupt the capacity calculation.
                vel = (vel * vel + delta_h * (-9.8)).max(0.0).sqrt();
                water = (water * (1.0 - self.evaporation_rate)).max(0.0);

                if water < 0.01 {
                    break;
                }

                px = new_px;
                pz = new_pz;
            }
        }
    }
}

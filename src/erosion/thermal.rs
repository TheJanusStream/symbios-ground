use crate::HeightMap;

/// Thermal erosion (talus / slope-smoothing) simulation.
///
/// Iteratively redistributes material from steep slopes to their downhill
/// neighbours until no slope exceeds the configured talus angle, producing
/// the characteristic scree-smoothed hillsides of real-world terrain.
#[derive(Debug, Clone)]
pub struct ThermalErosion {
    /// Number of smoothing passes.
    pub iterations: u32,
    /// Maximum stable height difference between adjacent cells.
    /// Material above this threshold slides downhill.
    pub talus_angle: f32,
    /// Fraction of excess material moved per iteration (in `(0.0, 0.5]`).
    pub fraction: f32,
}

impl ThermalErosion {
    pub fn new() -> Self {
        Self {
            iterations: 50,
            talus_angle: 0.05,
            fraction: 0.25,
        }
    }

    pub fn with_iterations(mut self, n: u32) -> Self {
        self.iterations = n;
        self
    }

    pub fn with_talus_angle(mut self, angle: f32) -> Self {
        self.talus_angle = angle;
        self
    }

    /// Apply thermal erosion to `heightmap` in-place.
    pub fn erode(&self, heightmap: &mut HeightMap) {
        let w = heightmap.width();
        let h = heightmap.height();

        // Offsets for the 4 cardinal neighbours.
        const NEIGHBOURS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

        // Allocate the delta buffer once and zero it each iteration to avoid
        // 50+ separate heap allocations inside the loop.
        let mut delta = vec![0.0_f32; w * h];

        for _ in 0..self.iterations {
            // Zero the reused buffer.
            delta.fill(0.0);

            for z in 0..h {
                for x in 0..w {
                    let h_here = heightmap.get(x, z);
                    let mut total_excess = 0.0_f32;
                    let mut excess = [0.0_f32; 4];

                    for (i, &(dx, dz)) in NEIGHBOURS.iter().enumerate() {
                        let nx = x as i32 + dx;
                        let nz = z as i32 + dz;
                        if nx < 0 || nx >= w as i32 || nz < 0 || nz >= h as i32 {
                            continue;
                        }
                        let h_nb = heightmap.get(nx as usize, nz as usize);
                        let diff = h_here - h_nb;
                        if diff > self.talus_angle {
                            excess[i] = diff - self.talus_angle;
                            total_excess += excess[i];
                        }
                    }

                    if total_excess <= 0.0 {
                        continue;
                    }

                    // Move a fraction of the total excess to each downhill neighbour.
                    for (i, &(dx, dz)) in NEIGHBOURS.iter().enumerate() {
                        if excess[i] <= 0.0 {
                            continue;
                        }
                        let nx = x as i32 + dx;
                        let nz = z as i32 + dz;
                        if nx < 0 || nx >= w as i32 || nz < 0 || nz >= h as i32 {
                            continue;
                        }
                        let transfer = self.fraction * excess[i];
                        delta[z * w + x] -= transfer;
                        delta[nz as usize * w + nx as usize] += transfer;
                    }
                }
            }

            // Apply the accumulated delta.
            for (v, d) in heightmap.data_mut().iter_mut().zip(delta.iter()) {
                *v += d;
            }
        }
    }
}

impl Default for ThermalErosion {
    fn default() -> Self {
        Self::new()
    }
}

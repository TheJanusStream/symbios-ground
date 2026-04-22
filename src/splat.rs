use crate::HeightMap;

/// A rule that maps terrain properties to a weight for one texture channel.
#[derive(Debug, Clone)]
pub struct SplatRule {
    /// Height range `[min, max]` in which this layer is active.
    pub height_range: (f32, f32),
    /// Slope range `[min, max]` (0 = flat, 1 = vertical) in which this layer is active.
    pub slope_range: (f32, f32),
    /// Sharpness of the blend falloff. Higher = harder edges.
    pub sharpness: f32,
}

impl SplatRule {
    /// Create a `SplatRule`.
    ///
    /// * `height_range` — `(min, max)` normalised height `[0, 1]` in which this
    ///   layer is active.
    /// * `slope_range` — `(min, max)` slope `[0, 1]` (`0` = flat, `1` = vertical)
    ///   in which this layer is active.
    /// * `sharpness` — power applied to the smooth falloff; higher values produce
    ///   harder, more abrupt transitions between layers.
    pub fn new(height_range: (f32, f32), slope_range: (f32, f32), sharpness: f32) -> Self {
        Self {
            height_range,
            slope_range,
            sharpness,
        }
    }

    /// Compute the raw (unnormalised) weight for a given height and slope.
    fn weight(&self, height: f32, slope: f32) -> f32 {
        let h_w = smooth_range(
            height,
            self.height_range.0,
            self.height_range.1,
            self.sharpness,
        );
        let s_w = smooth_range(
            slope,
            self.slope_range.0,
            self.slope_range.1,
            self.sharpness,
        );
        h_w * s_w
    }
}

/// A 4-channel (RGBA) texture weight map produced by [`SplatMapper`].
///
/// Each pixel holds four `u8` weights that sum to (approximately) 255, one
/// per texture layer. Feed directly into a GPU splat/terrain shader.
#[derive(Debug, Clone)]
pub struct WeightMap {
    /// Row-major RGBA data; `data[z * width + x]` = `[r, g, b, a]`.
    pub data: Vec<[u8; 4]>,
    pub width: usize,
    pub height: usize,
}

impl WeightMap {
    /// Creates a flat weight map with all weight in the first channel (R).
    pub fn new(width: usize, height: usize) -> Self {
        let data = vec![[255, 0, 0, 0]; width * height];
        Self {
            data,
            width,
            height,
        }
    }
}

/// Generates a 4-channel [`WeightMap`] from a [`HeightMap`] using four
/// configurable [`SplatRule`]s, one per RGBA channel.
///
/// # Default layers (used by [`SplatMapper::default`])
///
/// | Channel | Layer  | Description             |
/// |---------|--------|-------------------------|
/// | R       | Grass  | Low altitude, flat      |
/// | G       | Dirt   | Mid altitude, any slope |
/// | B       | Rock   | Steep slopes            |
/// | A       | Snow   | High altitude, flat     |
#[derive(Debug, Clone)]
pub struct SplatMapper {
    /// Rules for channels R, G, B, and A respectively.
    pub rules: [SplatRule; 4],
}

impl SplatMapper {
    /// Create a `SplatMapper` with custom per-channel rules.
    ///
    /// `rules[0]` drives the **R** channel, `[1]` → **G**, `[2]` → **B**,
    /// `[3]` → **A**. Use [`SplatMapper::default`] for the built-in
    /// grass / dirt / rock / snow preset.
    pub fn new(rules: [SplatRule; 4]) -> Self {
        Self { rules }
    }

    /// Compute the weight map for the given heightmap.
    ///
    /// Normals are computed via central differences; the slope is derived as
    /// `1.0 - normal.y` so that 0 = perfectly flat and 1 = vertical.
    pub fn generate(&self, heightmap: &HeightMap) -> WeightMap {
        let w = heightmap.width();
        let h = heightmap.height();
        let mut wm = WeightMap::new(w, h);

        for z in 0..h {
            for x in 0..w {
                let wx = x as f32 * heightmap.scale();
                let wz = z as f32 * heightmap.scale();

                let height = heightmap.get(x, z);
                let normal = heightmap.get_normal_at(wx, wz);
                // normal.y (index 1) = cos of angle from vertical; 1-y gives slope in [0,1].
                let slope = 1.0 - normal[1];

                let weights: [f32; 4] = [
                    self.rules[0].weight(height, slope),
                    self.rules[1].weight(height, slope),
                    self.rules[2].weight(height, slope),
                    self.rules[3].weight(height, slope),
                ];

                let total: f32 = weights.iter().sum();
                let pixel = if total > f32::EPSILON {
                    [
                        (weights[0] / total * 255.0).round() as u8,
                        (weights[1] / total * 255.0).round() as u8,
                        (weights[2] / total * 255.0).round() as u8,
                        (weights[3] / total * 255.0).round() as u8,
                    ]
                } else {
                    // No rule matches — fall through to channel B.
                    [0, 0, 255, 0]
                };

                wm.data[z * w + x] = pixel;
            }
        }

        wm
    }
}

impl Default for SplatMapper {
    /// Reasonable defaults for a grass/dirt/rock/snow terrain.
    fn default() -> Self {
        Self::new([
            // R — Grass: low altitude, gentle slope
            SplatRule::new((0.0, 0.45), (0.0, 0.3), 4.0),
            // G — Dirt: mid altitude, any slope
            SplatRule::new((0.3, 0.65), (0.0, 0.6), 2.0),
            // B — Rock: steep slopes regardless of altitude
            SplatRule::new((0.0, 1.0), (0.25, 1.0), 3.0),
            // A — Snow: high altitude, gentle slope
            SplatRule::new((0.7, 1.0), (0.0, 0.35), 4.0),
        ])
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns a smooth weight for `value` within `[lo, hi]`.
/// Outside the range the weight falls to 0; inside it peaks at 1.
fn smooth_range(value: f32, lo: f32, hi: f32, sharpness: f32) -> f32 {
    if lo >= hi {
        return if (value - lo).abs() < f32::EPSILON {
            1.0
        } else {
            0.0
        };
    }
    let mid = (lo + hi) * 0.5;
    let half = (hi - lo) * 0.5;
    let dist = (value - mid).abs();
    (1.0 - (dist / half).min(1.0)).powf(sharpness)
}

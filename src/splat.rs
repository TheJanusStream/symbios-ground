use crate::HeightMap;

/// A rule that maps terrain properties to a weight for one texture channel.
///
/// Both ranges are **plateaus with soft edges**: the layer is at full weight
/// everywhere inside the range, including at both endpoints, and fades to zero
/// over a skirt outside it whose width `sharpness` controls. Before 0.4.0 they
/// were tents that peaked at the midpoint and vanished at the endpoints, so a
/// rule naming `0.0` as its slope minimum scored nothing on level ground —
/// see `smooth_range` and #23.
#[derive(Debug, Clone)]
pub struct SplatRule {
    /// Height range `[min, max]` in which this layer is at full weight.
    pub height_range: (f32, f32),
    /// Slope range `[min, max]` (0 = flat, 1 = vertical) in which this layer is
    /// at full weight.
    pub slope_range: (f32, f32),
    /// Sharpness of the blend falloff outside the ranges. Higher = harder
    /// edges: the skirt narrows and the ramp across it steepens.
    pub sharpness: f32,
}

impl SplatRule {
    /// Create a `SplatRule`.
    ///
    /// * `height_range` — `(min, max)` normalised height `[0, 1]` across which
    ///   this layer is at full weight; it fades out beyond each end.
    /// * `slope_range` — `(min, max)` slope `[0, 1]` (`0` = flat, `1` = vertical)
    ///   across which this layer is at full weight; likewise.
    /// * `sharpness` — how abruptly the layer fades outside its ranges. Higher
    ///   values narrow the skirt and steepen the ramp across it.
    pub fn new(height_range: (f32, f32), slope_range: (f32, f32), sharpness: f32) -> Self {
        Self {
            height_range,
            slope_range,
            sharpness,
        }
    }

    /// Compute the raw (unnormalised) weight for a given height and slope.
    pub fn weight(&self, height: f32, slope: f32) -> f32 {
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

        let normals = heightmap.normals_grid();

        for z in 0..h {
            for x in 0..w {
                let height = heightmap.get(x, z);
                let normal = normals[z * w + x];
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

    /// Compute the four normalised splat weights at world position
    /// `(world_x, world_z)`. Output sums to `1.0` in well-defined cases, or
    /// returns the rock-channel fallback `[0, 0, 1, 0]` when no rule matches —
    /// matching the behaviour of [`SplatMapper::generate`] before u8
    /// quantisation.
    pub fn sample_weights_at(&self, heightmap: &HeightMap, world_x: f32, world_z: f32) -> [f32; 4] {
        let height = heightmap.get_height_at(world_x, world_z);
        let normal = heightmap.get_normal_at(world_x, world_z);
        let slope = 1.0 - normal[1];

        let raw = [
            self.rules[0].weight(height, slope),
            self.rules[1].weight(height, slope),
            self.rules[2].weight(height, slope),
            self.rules[3].weight(height, slope),
        ];
        let total: f32 = raw.iter().sum();
        if total > f32::EPSILON {
            [
                raw[0] / total,
                raw[1] / total,
                raw[2] / total,
                raw[3] / total,
            ]
        } else {
            // Mirrors the [0, 0, 255, 0] fallback in generate(): no rule
            // applies, so the rock channel absorbs the whole weight.
            [0.0, 0.0, 1.0, 0.0]
        }
    }

    /// Return the dominant biome channel (0..=3 = R/G/B/A) at world position
    /// `(world_x, world_z)`. Ties are broken by lowest channel index.
    pub fn sample_biome_at(&self, heightmap: &HeightMap, world_x: f32, world_z: f32) -> u8 {
        argmax_channel(&self.sample_weights_at(heightmap, world_x, world_z))
    }
}

/// Free-function form of [`SplatMapper::sample_weights_at`] for callers that
/// already have an `&SplatMapper` and prefer not to write a method call chain.
pub fn sample_splat_weights_at(
    heightmap: &HeightMap,
    mapper: &SplatMapper,
    world_x: f32,
    world_z: f32,
) -> [f32; 4] {
    mapper.sample_weights_at(heightmap, world_x, world_z)
}

/// Free-function form of [`SplatMapper::sample_biome_at`].
pub fn sample_biome_at(
    heightmap: &HeightMap,
    mapper: &SplatMapper,
    world_x: f32,
    world_z: f32,
) -> u8 {
    mapper.sample_biome_at(heightmap, world_x, world_z)
}

fn argmax_channel(weights: &[f32; 4]) -> u8 {
    let mut best_idx = 0u8;
    let mut best_val = weights[0];
    for (i, &w) in weights.iter().enumerate().skip(1) {
        if w > best_val {
            best_val = w;
            best_idx = i as u8;
        }
    }
    best_idx
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

/// Returns a weight of `1` for any `value` inside `[lo, hi]`, falling smoothly
/// to `0` over a skirt of width `edge` *outside* each end.
///
/// # Why the range is a plateau and not a tent (#23)
///
/// This used to be a tent: it peaked at the midpoint of `[lo, hi]` and reached
/// zero at both ends. That reading of a range is the opposite of how every
/// caller writes one. `slope_range: (0.0, 0.3)` means "this layer covers
/// slopes from level up to 0.3"; under a tent it meant "this layer is absent
/// on level ground, strongest at 0.15, and absent again at 0.3". Three of the
/// four rules in [`SplatMapper::default`] name `0.0` as a slope minimum, so
/// dead-level terrain scored zero in *every* channel and fell through to the
/// rock fallback — grass did not grow on flat ground anywhere. Near-level
/// pixels were rescued only by the per-pixel normalisation, which is why it
/// read as a fringe artifact rather than the systematic hole it was.
///
/// So the declared range is now exactly what it says: full weight throughout,
/// including at both endpoints. Blending between layers happens in the skirts
/// beyond the range rather than inside it.
///
/// `sharpness` keeps its meaning — higher is harder — and now controls the
/// edge two ways at once: the skirt narrows as `half / (1 + sharpness)`, and
/// the exponent steepens the ramp within it. At a large `sharpness` the rule
/// approaches a box; at a small one it fades out over a band as wide as the
/// range itself.
///
/// Two plateaus that overlap both score `1` there, which normalises to an even
/// blend and, for [`SplatMapper::sample_biome_at`], a tie broken by channel
/// order. That is deliberate: "both layers are active here" is what overlapping
/// ranges mean, and the gradient lives in the skirts on either side.
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
    // Skirt width beyond each end. `half` is strictly positive here, and
    // `sharpness` is non-negative in every documented use, so this is only
    // zero if `sharpness` is infinite — in which case the rule is a box and
    // the branch below returns the right answer anyway.
    let edge = half / (1.0 + sharpness);
    if edge <= 0.0 || edge.is_nan() {
        return if dist <= half { 1.0 } else { 0.0 };
    }
    // 1 anywhere inside `[lo, hi]`, ramping to 0 across the skirt.
    let t = ((half + edge - dist) / edge).clamp(0.0, 1.0);
    // `libm::powf` — see the crate docs on cross-target determinism. This is
    // the sharpest-consequence site in the crate: the four rule weights are
    // COMPARED against each other to pick a dominant channel, so a one-ULP
    // difference between two builds is not a slightly different number, it is
    // a different material on that texel and a different biome for anything a
    // consumer scatters there.
    libm::powf(t, sharpness)
}

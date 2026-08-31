//! Cross-target determinism goldens (#22).
//!
//! Consumers of this crate derive terrain on more than one machine — a native
//! host and a browser guest looking at the same room — and the whole point of
//! a seed is that both get the same ground. Rust does not guarantee that on
//! its own: `f32::exp` and `f32::powf` have unspecified precision and link
//! whatever libm the target ships, so the same seed produces measurably
//! different terrain on `x86_64-unknown-linux-gnu` (glibc) and on
//! `wasm32-unknown-unknown` (compiler-builtins). The crate therefore routes
//! those calls through the pure-Rust `libm` crate.
//!
//! ## What these tests prove, and what they do not
//!
//! Each test pins a hash of a full bake against a constant committed in this
//! file. That is a real test with a real failure mode, but only because the
//! bake was chosen to make it one — see [`DIVERGENT_ROUGHNESS`]. Measured on
//! x86-64, glibc and `libm` disagree in the last bit on about one argument in
//! ten across these call sites' actual input ranges and agree on the rest, so a
//! bake at arbitrary parameters is as likely as not to hash identically either
//! way. Reverting the three call sites to the `f32` methods was actually run,
//! and at this bake's parameters it moves both hashes below.
//!
//! What they do NOT prove is that a wasm32 build produces the same number,
//! because this crate has no wasm test runner — `cargo test` cannot execute a
//! `wasm32-unknown-unknown` binary. The argument that it does is the `libm`
//! crate's own: it is pure Rust with no platform dispatch, so it computes the
//! same operations in the same order on every target. These goldens are what
//! make that argument *checkable*: a consumer with a browser can compare its
//! bake against these same constants and get a yes or no, which was impossible
//! before they existed.

use symbios_ground::{DiamondSquare, HeightMap, HydraulicErosion, SplatMapper, TerrainGenerator};

/// FNV-1a 64, so the goldens below are readable constants rather than a
/// dependency. Any stable hash would do — what matters is that it is the same
/// one on both sides of a comparison.
fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn hash_heights(hm: &HeightMap) -> u64 {
    let mut bytes = Vec::with_capacity(hm.data().len() * 4);
    for v in hm.data() {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    fnv(&bytes)
}

/// Roughness `0.768`, and the oddly specific value is the whole point.
///
/// Diamond-Square's per-octave amplitude is `powf(0.5, 1.5 - roughness)`.
/// glibc and `libm` agree on most of that argument range and disagree on about
/// one roughness in ten (measured on x86-64 over 1001 values in `[0, 1]`), so
/// a bake at a round roughness like `0.6` hashes IDENTICALLY whichever
/// implementation it uses — a golden taken there passes against the pre-fix
/// code and proves nothing. This value is one of the disagreeing ones.
///
/// It also has to be a LARGE roughness. At `0.005` the two implementations
/// disagree in the amplitude just as they do here, and the bake still hashes
/// the same: the amplitude is the size of the random offset added to each
/// midpoint, so when it is small the perturbed offset rounds back onto the same
/// `f32` as it is added to a much larger accumulated height, and the difference
/// is absorbed before it reaches the grid. That absorption is not a flaw in the
/// test, it is the honest scale of the effect — a low-relief terrain really is
/// less sensitive to this than a high-relief one.
///
/// Both facts were established by measurement, not argument: the goldens below
/// were taken with `libm` in place, then the three call sites were reverted to
/// the `f32` methods and the goldens re-taken. At `0.6` they matched; at
/// `0.005` they matched; at `0.768` both moved.
const DIVERGENT_ROUGHNESS: f32 = 0.768;

/// One fixed bake: Diamond-Square at a fixed seed and [`DIVERGENT_ROUGHNESS`],
/// then the hydraulic erosion pass at a fixed seed. Both stages sit on a
/// transcendental this crate now routes through `libm` — the per-octave
/// amplitude and the deposition kernel respectively.
fn baked() -> HeightMap {
    let mut hm = HeightMap::new(129, 129, 1.0);
    DiamondSquare::new(0x5EED_0001, DIVERGENT_ROUGHNESS).generate(&mut hm);
    HydraulicErosion::new(0x5EED_0002).erode(&mut hm);
    hm
}

/// Golden hash of [`baked`]'s heights, produced on x86-64 with the `libm`
/// routing in place.
///
/// A failure here means one of three things, in decreasing order of
/// likelihood: a call site went back to an `f32` method; a generator's
/// arithmetic changed (in which case re-cut this constant deliberately and say
/// so in the changelog, because every consumer's seeded terrain just moved);
/// or `libm` itself changed a result, which is worth reading its release notes
/// over.
const GOLDEN_HEIGHTS: u64 = 10_042_817_713_454_538_825;

/// Golden hash of the splat weights derived from that same bake. Separate from
/// the heights because it fails for a different reason: `SplatRule` scoring
/// COMPARES four `powf` results to pick a channel, so this constant moves when
/// a comparison flips even if every height is untouched.
///
/// Re-cut deliberately for 0.4.0 (#23), which is the case the paragraph above
/// describes: heights are untouched — `GOLDEN_HEIGHTS` did not move — and
/// every weight changed because ranges became plateaus instead of tents. The
/// previous value was `6_483_499_504_086_671_138`. On this bake the old
/// semantics left 13,708 of 16,641 texels on the no-rule-matched rock
/// fallback, on a terrain whose steepest slope is under 0.1; they are now
/// grass and dirt, and no texel takes the fallback.
const GOLDEN_SPLAT: u64 = 8_164_559_026_891_617_516;

#[test]
fn a_seeded_bake_hashes_to_its_golden() {
    assert_eq!(
        hash_heights(&baked()),
        GOLDEN_HEIGHTS,
        "the seeded heightmap moved — see this file's docs before re-cutting the constant"
    );
}

#[test]
fn seeded_splat_weights_hash_to_their_golden() {
    let wm = SplatMapper::default().generate(&baked());
    let bytes: Vec<u8> = wm.data.iter().flat_map(|p| p.iter().copied()).collect();
    assert_eq!(
        fnv(&bytes),
        GOLDEN_SPLAT,
        "the seeded splat weights moved — a flipped channel is a different \
         material on that texel, and a different biome for whatever a consumer \
         scatters on it"
    );
}

/// The goldens above only mean something if the bake is deterministic in the
/// ordinary sense first. This is the control: same seed, same process, same
/// answer. If it ever fails, the goldens are meaningless and the problem is
/// not the toolchain.
#[test]
fn the_same_seed_bakes_the_same_terrain_twice() {
    assert_eq!(hash_heights(&baked()), hash_heights(&baked()));
}

/// And the control's mirror: a different seed must actually produce different
/// terrain, or the goldens above would pass for a bake that generates nothing.
#[test]
fn a_different_seed_bakes_different_terrain() {
    let mut other = HeightMap::new(129, 129, 1.0);
    DiamondSquare::new(0x5EED_9999, DIVERGENT_ROUGHNESS).generate(&mut other);
    HydraulicErosion::new(0x5EED_0002).erode(&mut other);
    assert_ne!(hash_heights(&baked()), hash_heights(&other));
}

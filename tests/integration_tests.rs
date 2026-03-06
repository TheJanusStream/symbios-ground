use symbios_ground::{
    DiamondSquare, FbmNoise, HeightMap, HydraulicErosion, SplatMapper, TerrainGenerator,
    ThermalErosion, VoronoiTerracing,
};

// ---------------------------------------------------------------------------
// HeightMap
// ---------------------------------------------------------------------------

#[test]
fn heightmap_get_set_roundtrip() {
    let mut hm = HeightMap::new(8, 8, 1.0);
    hm.set(3, 5, 0.75);
    assert!((hm.get(3, 5) - 0.75).abs() < f32::EPSILON);
}

#[test]
fn heightmap_bilinear_corners() {
    // A 2×2 heightmap with known corner values.
    let mut hm = HeightMap::new(2, 2, 1.0);
    hm.set(0, 0, 0.0);
    hm.set(1, 0, 1.0);
    hm.set(0, 1, 0.0);
    hm.set(1, 1, 1.0);

    // At world x=0.5, z=0 we should get exactly 0.5 (midpoint of 0 and 1).
    let h = hm.get_height_at(0.5, 0.0);
    assert!((h - 0.5).abs() < 1e-5, "expected 0.5, got {h}");
}

#[test]
fn heightmap_bilinear_clamped() {
    let mut hm = HeightMap::new(4, 4, 1.0);
    hm.set(0, 0, 0.5);
    // Query far outside the grid — should clamp, not panic.
    let h = hm.get_height_at(-100.0, -100.0);
    assert!((h - 0.5).abs() < 1e-5);
}

#[test]
fn heightmap_normal_at_denormal_scale_does_not_produce_nan() {
    // With a denormal scale, (hr - hl) / (2 * scale) overflows to ±INF.
    // The is_finite guard must return a flat [0, 1, 0] normal instead of NaN.
    let hm = HeightMap::new(4, 4, 1e-40);
    let n = hm.get_normal_at(0.0, 0.0);
    for component in n {
        assert!(
            component.is_finite(),
            "normal component is not finite: {component}"
        );
    }
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    assert!((len - 1.0).abs() < 1e-5, "normal is not unit length: {len}");
}

#[test]
fn heightmap_normal_flat_terrain() {
    // A flat heightmap should return a perfectly up-facing normal.
    let mut hm = HeightMap::new(8, 8, 1.0);
    for v in hm.data_mut().iter_mut() {
        *v = 0.5;
    }
    let n = hm.get_normal_at(4.0, 4.0);
    assert!((n[0]).abs() < 1e-5, "nx should be 0, got {}", n[0]);
    assert!((n[1] - 1.0).abs() < 1e-5, "ny should be 1, got {}", n[1]);
    assert!((n[2]).abs() < 1e-5, "nz should be 0, got {}", n[2]);
}

#[test]
fn heightmap_normal_is_unit_length() {
    let mut hm = HeightMap::new(17, 17, 1.0);
    DiamondSquare::new(7, 0.6).generate(&mut hm);
    for z in 1..(hm.height() - 1) {
        for x in 1..(hm.width() - 1) {
            let n = hm.get_normal_at(x as f32, z as f32);
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-4, "normal not unit length: {len}");
        }
    }
}

#[test]
fn heightmap_normalize() {
    let mut hm = HeightMap::new(4, 4, 1.0);
    for (i, v) in hm.data_mut().iter_mut().enumerate() {
        *v = i as f32;
    }
    hm.normalize();
    let min = hm.data().iter().cloned().fold(f32::INFINITY, f32::min);
    let max = hm.data().iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    assert!((min - 0.0).abs() < 1e-5);
    assert!((max - 1.0).abs() < 1e-5);
}

#[test]
fn heightmap_world_dimensions() {
    let hm = HeightMap::new(10, 20, 2.5);
    assert!((hm.world_width() - 25.0).abs() < f32::EPSILON);
    assert!((hm.world_depth() - 50.0).abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// DiamondSquare
// ---------------------------------------------------------------------------

#[test]
fn diamond_square_output_in_unit_range() {
    let mut hm = HeightMap::new(65, 65, 1.0);
    DiamondSquare::new(42, 0.7).generate(&mut hm);
    for &v in hm.data() {
        assert!(v >= 0.0 && v <= 1.0, "value out of range: {v}");
    }
}

#[test]
fn diamond_square_resizes_to_power_of_two_plus_one() {
    let mut hm = HeightMap::new(100, 100, 1.0);
    DiamondSquare::new(1, 0.5).generate(&mut hm);
    // 100 → nearest 2^n+1 = 129
    assert_eq!(hm.width(), 129);
    assert_eq!(hm.height(), 129);
}

#[test]
fn diamond_square_deterministic() {
    let mut a = HeightMap::new(33, 33, 1.0);
    let mut b = HeightMap::new(33, 33, 1.0);
    DiamondSquare::new(99, 0.5).generate(&mut a);
    DiamondSquare::new(99, 0.5).generate(&mut b);
    assert_eq!(a.data(), b.data());
}

#[test]
fn diamond_square_out_of_range_roughness_does_not_panic() {
    // roughness far above 1.0 overflows amp to f32::INFINITY; the is_finite
    // guard must prevent the resulting random_range(-INF..INF) panic.
    let mut hm = HeightMap::new(17, 17, 1.0);
    DiamondSquare::new(1, 200.0).generate(&mut hm);
    for &v in hm.data() {
        assert!(v.is_finite(), "non-finite value in output: {v}");
    }
}

#[test]
fn diamond_square_different_seeds_differ() {
    let mut a = HeightMap::new(33, 33, 1.0);
    let mut b = HeightMap::new(33, 33, 1.0);
    DiamondSquare::new(1, 0.5).generate(&mut a);
    DiamondSquare::new(2, 0.5).generate(&mut b);
    assert_ne!(a.data(), b.data());
}

// ---------------------------------------------------------------------------
// FbmNoise
// ---------------------------------------------------------------------------

#[test]
fn fbm_output_in_unit_range() {
    let mut hm = HeightMap::new(64, 64, 1.0);
    FbmNoise::new(123).generate(&mut hm);
    for &v in hm.data() {
        assert!(v >= 0.0 && v <= 1.0, "value out of range: {v}");
    }
}

#[test]
fn fbm_deterministic() {
    let mut a = HeightMap::new(32, 32, 1.0);
    let mut b = HeightMap::new(32, 32, 1.0);
    FbmNoise::new(7).generate(&mut a);
    FbmNoise::new(7).generate(&mut b);
    assert_eq!(a.data(), b.data());
}

// ---------------------------------------------------------------------------
// VoronoiTerracing
// ---------------------------------------------------------------------------

#[test]
fn voronoi_output_in_unit_range() {
    let mut hm = HeightMap::new(64, 64, 1.0);
    VoronoiTerracing::new(5, 20, 5).generate(&mut hm);
    for &v in hm.data() {
        assert!(v >= 0.0 && v <= 1.0, "value out of range: {v}");
    }
}

#[test]
fn voronoi_deterministic() {
    let mut a = HeightMap::new(32, 32, 1.0);
    let mut b = HeightMap::new(32, 32, 1.0);
    VoronoiTerracing::new(3, 12, 4).generate(&mut a);
    VoronoiTerracing::new(3, 12, 4).generate(&mut b);
    assert_eq!(a.data(), b.data());
}

// ---------------------------------------------------------------------------
// HydraulicErosion
// ---------------------------------------------------------------------------

#[test]
fn hydraulic_erosion_does_not_panic() {
    let mut hm = HeightMap::new(65, 65, 1.0);
    DiamondSquare::new(1, 0.6).generate(&mut hm);
    HydraulicErosion::new(42).erode(&mut hm);
}

#[test]
fn hydraulic_erosion_changes_heightmap() {
    let mut before = HeightMap::new(65, 65, 1.0);
    DiamondSquare::new(1, 0.6).generate(&mut before);
    let mut after = before.clone();
    HydraulicErosion::new(42).erode(&mut after);
    assert_ne!(before.data(), after.data());
}

// ---------------------------------------------------------------------------
// ThermalErosion
// ---------------------------------------------------------------------------

#[test]
fn thermal_erosion_does_not_panic() {
    let mut hm = HeightMap::new(65, 65, 1.0);
    DiamondSquare::new(2, 0.8).generate(&mut hm);
    ThermalErosion::new().erode(&mut hm);
}

#[test]
fn thermal_erosion_reduces_extreme_slopes() {
    // Create a step function: left half height 0, right half height 1.
    let mut hm = HeightMap::new(32, 32, 1.0);
    for z in 0..32 {
        for x in 0..32 {
            hm.set(x, z, if x < 16 { 0.0 } else { 1.0 });
        }
    }

    let erosion = ThermalErosion {
        iterations: 200,
        talus_angle: 0.05,
        fraction: 0.25,
        water_level: 0.0,
        underwater_talus_angle: 0.1,
    };
    erosion.erode(&mut hm);

    // After erosion the cliff should be softer: height at x=15 rises, x=16 falls.
    let left_edge = hm.get(15, 16);
    let right_edge = hm.get(16, 16);
    assert!(left_edge > 0.0, "left edge should have gained material");
    assert!(right_edge < 1.0, "right edge should have lost material");
}

// ---------------------------------------------------------------------------
// SplatMapper
// ---------------------------------------------------------------------------

#[test]
fn splat_mapper_weights_sum_to_255() {
    let mut hm = HeightMap::new(32, 32, 1.0);
    DiamondSquare::new(10, 0.5).generate(&mut hm);
    let wm = SplatMapper::default().generate(&hm);

    for px in &wm.data {
        let total: u16 = px.iter().map(|&b| b as u16).sum();
        // Rounding means the sum can be 253–257; verify it is close to 255.
        assert!(
            total >= 250 && total <= 260,
            "channel sum out of range: {total}"
        );
    }
}

#[test]
fn splat_mapper_dimensions_match_heightmap() {
    let mut hm = HeightMap::new(16, 24, 1.0);
    FbmNoise::new(0).generate(&mut hm);
    let wm = SplatMapper::default().generate(&hm);
    assert_eq!(wm.width, 16);
    assert_eq!(wm.height, 24);
    assert_eq!(wm.data.len(), 16 * 24);
}

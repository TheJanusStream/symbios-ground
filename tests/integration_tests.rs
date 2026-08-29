use symbios_ground::{
    DiamondSquare, FbmNoise, HeightMap, HydraulicErosion, Lake, SplatMapper, TerrainGenerator,
    ThermalErosion, TiledHeightMap, VoronoiTerracing, derive_tile_seed, sample_biome_at,
    sample_splat_weights_at,
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
        assert!((0.0..=1.0).contains(&v), "value out of range: {v}");
    }
}

#[test]
fn diamond_square_preserves_user_dimensions() {
    // After internal generation at 2^n+1, output is bilinearly resampled to
    // whatever size the caller asked for. Both square and rectangular,
    // power-of-two and arbitrary, must round-trip through unchanged.
    for &(w, h) in &[
        (100usize, 100usize),
        (129, 129),
        (64, 96),
        (10, 200),
        (33, 33),
    ] {
        let mut hm = HeightMap::new(w, h, 1.0);
        DiamondSquare::new(1, 0.5).generate(&mut hm);
        assert_eq!(hm.width(), w, "width changed for {w}x{h}");
        assert_eq!(hm.height(), h, "height changed for {w}x{h}");
        assert_eq!(hm.data().len(), w * h, "data length wrong for {w}x{h}");
        for &v in hm.data() {
            assert!(v.is_finite() && (0.0..=1.0).contains(&v));
        }
    }
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
        assert!((0.0..=1.0).contains(&v), "value out of range: {v}");
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
        assert!((0.0..=1.0).contains(&v), "value out of range: {v}");
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

#[test]
fn hydraulic_erosion_pools_lakes_in_basin() {
    // Construct a steep quadratic bowl strictly above water_level so any
    // pooling must come through the lake path (not the at/below-water delta
    // fan path).
    let size = 65usize;
    let mut hm = HeightMap::new(size, size, 1.0);
    let cx = (size as f32 - 1.0) * 0.5;
    let cz = (size as f32 - 1.0) * 0.5;
    let max_dist_sq = cx * cx + cz * cz;
    for z in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dz = z as f32 - cz;
            let d_sq = (dx * dx + dz * dz) / max_dist_sq;
            // Centre = 0.1, edge = 1.0 — a steep bowl above water_level=0.0.
            hm.set(x, z, 0.1 + 0.9 * d_sq);
        }
    }

    let mut eroder = HydraulicErosion::new(7);
    eroder.num_drops = 5_000;
    eroder.water_level = 0.0;
    eroder.erode(&mut hm);

    let lakes: &[Lake] = hm.lakes();
    assert!(
        !lakes.is_empty(),
        "expected at least one pooled lake in a bowl-shaped heightmap"
    );

    // Sanity: total pooled water should be a sensible fraction of the input
    // (each droplet starts with water=1.0, so a 5_000-drop run can deposit at
    // most ~5_000 units; expect some non-trivial fraction to pool).
    let total_depth: f32 = lakes.iter().map(|l| l.depth).sum();
    assert!(
        total_depth > 0.5,
        "expected meaningful total lake depth, got {total_depth}"
    );

    // Each lake cell records the heightmap's per-cell area.
    let expected_area = hm.scale() * hm.scale();
    for l in lakes {
        assert!(
            (l.area - expected_area).abs() < 1e-6,
            "lake area mismatch: {} vs {}",
            l.area,
            expected_area
        );
    }
}

#[test]
fn hydraulic_erosion_lakes_persist_through_serde() {
    // After erosion, serialising and deserialising the heightmap must round-trip
    // the lake list — renderers can persist eroded terrain to disk.
    let mut hm = HeightMap::new(33, 33, 1.0);
    DiamondSquare::new(2, 0.6).generate(&mut hm);
    HydraulicErosion::new(11).erode(&mut hm);

    let json = serde_json::to_string(&hm).expect("serialize");
    let restored: HeightMap = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.lakes().len(), hm.lakes().len());
    if !hm.lakes().is_empty() {
        assert_eq!(restored.lakes()[0].index, hm.lakes()[0].index);
    }
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
            (250..=260).contains(&total),
            "channel sum out of range: {total}"
        );
    }
}

#[test]
fn heightmap_cached_normals_match_get_normal_at() {
    // Cached per-grid normals must match on-the-fly central-difference normals
    // for the equivalent world position, otherwise SplatMapper would drift.
    let mut hm = HeightMap::new(33, 33, 1.5);
    DiamondSquare::new(11, 0.6).generate(&mut hm);

    for z in 0..hm.height() {
        for x in 0..hm.width() {
            let cached = hm.normal_at_grid(x, z);
            let live = hm.get_normal_at(x as f32 * hm.scale(), z as f32 * hm.scale());
            for i in 0..3 {
                assert!(
                    (cached[i] - live[i]).abs() < 1e-5,
                    "cached normal diverges at ({x},{z}) component {i}: {} vs {}",
                    cached[i],
                    live[i]
                );
            }
        }
    }
}

#[test]
fn heightmap_normal_cache_invalidates_on_mutation() {
    let mut hm = HeightMap::new(8, 8, 1.0);
    for v in hm.data_mut().iter_mut() {
        *v = 0.0;
    }
    let flat = hm.normal_at_grid(4, 4);
    assert!(
        (flat[1] - 1.0).abs() < 1e-5,
        "expected up-normal on flat map"
    );

    // Introduce a slope at the centre and verify the cache reflects it.
    hm.set(5, 4, 1.0);
    let after = hm.normal_at_grid(4, 4);
    assert!(
        after[0].abs() > 1e-3,
        "x component should reflect new slope"
    );
}

#[test]
fn splat_point_query_agrees_with_full_grid() {
    let mut hm = HeightMap::new(33, 33, 1.0);
    DiamondSquare::new(7, 0.5).generate(&mut hm);
    let mapper = SplatMapper::default();
    let wm = mapper.generate(&hm);

    for z in 0..hm.height() {
        for x in 0..hm.width() {
            let weights =
                mapper.sample_weights_at(&hm, x as f32 * hm.scale(), z as f32 * hm.scale());

            // Channel sum is ~1.0 (or fallback distribution).
            let total: f32 = weights.iter().sum();
            assert!(
                (total - 1.0).abs() < 1e-4,
                "weights at ({x},{z}) do not sum to 1: {weights:?}"
            );

            // Compare against the full-grid value (rescaled to [0,1]).
            let pixel = wm.data[z * wm.width + x];
            for c in 0..4 {
                let grid_w = pixel[c] as f32 / 255.0;
                assert!(
                    (weights[c] - grid_w).abs() < 0.01,
                    "channel {c} mismatch at ({x},{z}): point={} grid={}",
                    weights[c],
                    grid_w
                );
            }
        }
    }
}

#[test]
fn splat_biome_at_returns_argmax_of_weights() {
    let mut hm = HeightMap::new(33, 33, 1.0);
    FbmNoise::new(3).generate(&mut hm);
    let mapper = SplatMapper::default();

    for z in 0..hm.height() {
        for x in 0..hm.width() {
            let wx = x as f32 * hm.scale();
            let wz = z as f32 * hm.scale();
            let weights = mapper.sample_weights_at(&hm, wx, wz);
            let biome = mapper.sample_biome_at(&hm, wx, wz);

            // The reported biome must hold the maximum weight.
            for (c, &w) in weights.iter().enumerate() {
                assert!(
                    weights[biome as usize] >= w - 1e-6,
                    "biome {biome} at ({x},{z}) is not argmax (channel {c} = {w})"
                );
            }
        }
    }
}

#[test]
fn splat_free_function_helpers_match_methods() {
    let mut hm = HeightMap::new(17, 17, 1.0);
    DiamondSquare::new(2, 0.5).generate(&mut hm);
    let mapper = SplatMapper::default();
    let wf = sample_splat_weights_at(&hm, &mapper, 8.5, 4.25);
    let wm_method = mapper.sample_weights_at(&hm, 8.5, 4.25);
    assert_eq!(wf, wm_method);
    assert_eq!(
        sample_biome_at(&hm, &mapper, 8.5, 4.25),
        mapper.sample_biome_at(&hm, 8.5, 4.25)
    );
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

// ---------------------------------------------------------------------------
// TiledHeightMap
// ---------------------------------------------------------------------------

#[test]
fn tiled_heightmap_only_allocates_requested_tiles() {
    let mut tiled = TiledHeightMap::new(64, 1.0, 42, |seed| Box::new(FbmNoise::new(seed)));
    assert_eq!(tiled.loaded_count(), 0);

    let _ = tiled.tile((0, 0));
    assert_eq!(tiled.loaded_count(), 1);

    let _ = tiled.tile((0, 0)); // cached, no new allocation
    assert_eq!(tiled.loaded_count(), 1);

    let _ = tiled.tile((-3, 7));
    assert_eq!(tiled.loaded_count(), 2);
}

#[test]
fn tiled_heightmap_per_tile_seeds_are_deterministic() {
    let mut a = TiledHeightMap::new(32, 1.0, 17, |seed| Box::new(DiamondSquare::new(seed, 0.6)));
    let mut b = TiledHeightMap::new(32, 1.0, 17, |seed| Box::new(DiamondSquare::new(seed, 0.6)));

    // Different request orders must produce identical tiles for the same coord.
    let _ = a.tile((1, 1));
    let _ = a.tile((0, 0));
    let _ = b.tile((0, 0));
    let _ = b.tile((1, 1));

    for coord in [(0, 0), (1, 1)] {
        assert_eq!(
            a.loaded_tile(coord).unwrap().heightmap.data(),
            b.loaded_tile(coord).unwrap().heightmap.data(),
            "tile {coord:?} differs between identical TiledHeightMaps"
        );
    }
}

#[test]
fn tiled_heightmap_neighbour_tiles_differ_under_independent_seeds() {
    let mut tiled = TiledHeightMap::new(32, 1.0, 5, |seed| Box::new(DiamondSquare::new(seed, 0.6)));
    let a = tiled.tile((0, 0)).heightmap.data().to_vec();
    let b = tiled.tile((1, 0)).heightmap.data().to_vec();
    assert_ne!(
        a, b,
        "adjacent tiles with independent seeds should not match"
    );
}

#[test]
fn tiled_heightmap_world_to_tile_mapping() {
    let tiled = TiledHeightMap::new(64, 0.5, 0, |seed| Box::new(FbmNoise::new(seed)));
    // tile_world_size = 64 * 0.5 = 32. Tile (0,0) covers [0,32). Tile (1,0)
    // starts at world_x=32. Negative tiles handle the -x/-z half-spaces.
    assert_eq!(tiled.tile_for_world(0.0, 0.0), (0, 0));
    assert_eq!(tiled.tile_for_world(31.99, 0.0), (0, 0));
    assert_eq!(tiled.tile_for_world(32.0, 0.0), (1, 0));
    assert_eq!(tiled.tile_for_world(-0.01, 0.0), (-1, 0));
    assert_eq!(tiled.tile_for_world(-32.0, 0.0), (-1, 0));
    assert_eq!(tiled.tile_for_world(-32.01, 0.0), (-2, 0));
}

#[test]
fn tiled_heightmap_evict_outside_drops_far_tiles() {
    let mut tiled = TiledHeightMap::new(16, 1.0, 0, |seed| Box::new(FbmNoise::new(seed)));
    tiled.ensure_radius((0, 0), 3); // 7×7 = 49 tiles
    assert_eq!(tiled.loaded_count(), 49);

    let evicted = tiled.evict_outside((0, 0), 1); // keep 3×3 = 9
    assert_eq!(evicted, 40);
    assert_eq!(tiled.loaded_count(), 9);
}

#[test]
fn tiled_heightmap_sample_height_at_world_generates_on_demand() {
    let mut tiled = TiledHeightMap::new(16, 1.0, 99, |seed| Box::new(FbmNoise::new(seed)));
    assert_eq!(tiled.loaded_count(), 0);
    let h = tiled.sample_height_at(8.5, 8.5);
    assert!(h.is_finite());
    assert_eq!(tiled.loaded_count(), 1);
}

#[test]
fn derive_tile_seed_is_deterministic_and_distinct() {
    assert_eq!(derive_tile_seed(42, 0, 0), derive_tile_seed(42, 0, 0));
    assert_ne!(derive_tile_seed(42, 0, 0), derive_tile_seed(42, 1, 0));
    assert_ne!(derive_tile_seed(42, 0, 0), derive_tile_seed(42, 0, 1));
    assert_ne!(derive_tile_seed(42, 1, 0), derive_tile_seed(42, 0, 1));
    assert_ne!(derive_tile_seed(42, 0, 0), derive_tile_seed(43, 0, 0));
}

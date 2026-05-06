use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use symbios_ground::{DiamondSquare, FbmNoise, HeightMap, SplatMapper, TerrainGenerator};

fn bench_heightmap_query(c: &mut Criterion) {
    let mut hm = HeightMap::new(129, 129, 1.0);
    DiamondSquare::new(42, 0.6).generate(&mut hm);

    c.bench_function("get_height_at", |b| b.iter(|| hm.get_height_at(64.5, 64.5)));

    c.bench_function("get_normal_at", |b| b.iter(|| hm.get_normal_at(64.5, 64.5)));
}

fn bench_generators(c: &mut Criterion) {
    let sizes = [65usize, 129];
    let mut group = c.benchmark_group("generators");

    for &size in &sizes {
        group.bench_with_input(BenchmarkId::new("DiamondSquare", size), &size, |b, &s| {
            b.iter(|| {
                let mut hm = HeightMap::new(s, s, 1.0);
                DiamondSquare::new(42, 0.6).generate(&mut hm);
            })
        });

        group.bench_with_input(BenchmarkId::new("FbmNoise", size), &size, |b, &s| {
            b.iter(|| {
                let mut hm = HeightMap::new(s, s, 1.0);
                FbmNoise::new(42).generate(&mut hm);
            })
        });
    }

    group.finish();
}

fn bench_splat(c: &mut Criterion) {
    let sizes = [129usize, 257, 513];
    let mut group = c.benchmark_group("splat");

    for &size in &sizes {
        group.bench_with_input(BenchmarkId::new("regenerate", size), &size, |b, &s| {
            let mut hm = HeightMap::new(s, s, 1.0);
            DiamondSquare::new(42, 0.6).generate(&mut hm);
            // Prime the normal cache so we benchmark steady-state regeneration.
            let _ = hm.normals_grid();
            let mapper = SplatMapper::default();
            b.iter(|| mapper.generate(&hm));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_heightmap_query,
    bench_generators,
    bench_splat
);
criterion_main!(benches);

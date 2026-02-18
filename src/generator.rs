use crate::HeightMap;

/// Trait for procedural terrain generators.
///
/// Implementors fill a `HeightMap` with generated height data.
pub trait TerrainGenerator {
    /// Fill `heightmap` with generated terrain data.
    fn generate(&self, heightmap: &mut HeightMap);
}

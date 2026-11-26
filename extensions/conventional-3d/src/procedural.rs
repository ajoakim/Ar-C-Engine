//! Procedural fallback geometry and textures (no files required).

use crate::import::{ImportedMaterial, ImportedMesh, ImportedModel, ImportedNode, ImportedTexture};
use engine_sdk::{Transform, VertexPosNormalUv};

/// Axis-aligned cube centered at the origin with edge length `size`.
///
/// 24 vertices (per-face normals), 36 indices, counter-clockwise outward faces.
#[must_use]
pub fn cube_mesh(size: f32) -> ImportedMesh {
    let h = size * 0.5;
    // (normal, u axis, v axis) per face; position = n + u*(+-1) + v*(+-1), with
    // u x v == n so CCW winding faces outward.
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];
    // Corner signs (u, v) and uvs: (-,-)->(0,1) (+,-)->(1,1) (+,+)->(1,0) (-,+)->(0,0).
    let corners: [(f32, f32, [f32; 2]); 4] = [
        (-1.0, -1.0, [0.0, 1.0]),
        (1.0, -1.0, [1.0, 1.0]),
        (1.0, 1.0, [1.0, 0.0]),
        (-1.0, 1.0, [0.0, 0.0]),
    ];
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for (n, u, v) in faces {
        let base = vertices.len() as u32;
        for (su, sv, uv) in corners {
            let position = [
                (n[0] + u[0] * su + v[0] * sv) * h,
                (n[1] + u[1] * su + v[1] * sv) * h,
                (n[2] + u[2] * su + v[2] * sv) * h,
            ];
            vertices.push(VertexPosNormalUv {
                position,
                normal: n,
                uv,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    ImportedMesh {
        vertices,
        indices,
        material: None,
    }
}

/// Horizontal plane at y = 0, normal +Y, spanning `+-half_extent` in X and Z.
#[must_use]
pub fn plane_mesh(half_extent: f32) -> ImportedMesh {
    let h = half_extent;
    let v = |x: f32, z: f32, uv: [f32; 2]| VertexPosNormalUv {
        position: [x, 0.0, z],
        normal: [0.0, 1.0, 0.0],
        uv,
    };
    ImportedMesh {
        vertices: vec![
            v(-h, h, [0.0, 1.0]),
            v(h, h, [1.0, 1.0]),
            v(h, -h, [1.0, 0.0]),
            v(-h, -h, [0.0, 0.0]),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        material: None,
    }
}

/// Checkerboard RGBA8 texture of `size` x `size` texels.
#[must_use]
pub fn checker_texture(size: u32, a: [u8; 4], b: [u8; 4]) -> ImportedTexture {
    let mut rgba8 = Vec::with_capacity((size as usize) * (size as usize) * 4);
    for y in 0..size {
        for x in 0..size {
            rgba8.extend_from_slice(if (x + y) % 2 == 0 { &a } else { &b });
        }
    }
    ImportedTexture {
        width: size,
        height: size,
        rgba8,
    }
}

/// A single textured cube model built without any file.
#[must_use]
pub fn procedural_cube_model() -> ImportedModel {
    let mut mesh = cube_mesh(1.0);
    mesh.material = Some(0);
    ImportedModel {
        meshes: vec![mesh],
        materials: vec![ImportedMaterial {
            base_color: [1.0, 1.0, 1.0, 1.0],
            texture: Some(0),
        }],
        textures: vec![checker_texture(8, [230, 230, 230, 255], [40, 90, 200, 255])],
        nodes: vec![ImportedNode {
            name: Some("cube".to_owned()),
            parent: None,
            local: Transform::IDENTITY,
            meshes: vec![0],
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }
    fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }
    fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    #[test]
    fn cube_mesh_shape() {
        let m = cube_mesh(1.0);
        assert_eq!(m.vertices.len(), 24);
        assert_eq!(m.indices.len(), 36);
        for v in &m.vertices {
            let n = v.normal;
            assert!((dot(n, n) - 1.0).abs() < 1e-6);
            assert_eq!(n.iter().filter(|c| c.abs() == 1.0).count(), 1);
            assert!(v.position.iter().all(|c| c.abs() <= 0.5 + 1e-6));
        }
        for tri in m.indices.chunks(3) {
            let p: Vec<_> = tri.iter().map(|&i| m.vertices[i as usize]).collect();
            let g = cross(
                sub(p[1].position, p[0].position),
                sub(p[2].position, p[0].position),
            );
            assert!(dot(g, p[0].normal) > 0.0, "face not CCW outward");
        }
    }

    #[test]
    fn plane_mesh_shape() {
        let m = plane_mesh(10.0);
        assert_eq!(m.vertices.len(), 4);
        assert!(m.vertices.iter().all(|v| v.normal == [0.0, 1.0, 0.0]));
        for tri in m.indices.chunks(3) {
            let p: Vec<_> = tri.iter().map(|&i| m.vertices[i as usize]).collect();
            let g = cross(
                sub(p[1].position, p[0].position),
                sub(p[2].position, p[0].position),
            );
            assert!(g[1] > 0.0);
        }
    }

    #[test]
    fn checker_texture_shape() {
        let a = [1, 2, 3, 4];
        let b = [5, 6, 7, 8];
        let t = checker_texture(8, a, b);
        assert_eq!((t.width, t.height, t.rgba8.len()), (8, 8, 256));
        assert_eq!(&t.rgba8[0..4], &a);
        assert_eq!(&t.rgba8[4..8], &b);
    }
}

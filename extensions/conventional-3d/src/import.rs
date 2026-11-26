//! glTF 2.0 import into engine-shaped CPU data.
//!
//! All input is untrusted: sizes, dimensions, counts and URIs are bounded by
//! [`ImportLimits`] and [`check_uri`]; document data is never indexed unchecked.
//! Images are decoded by [`decode_image`] (not by `gltf::import`), so every decode
//! is bounded.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::Path;

use engine_sdk::{DVec3, MAX_TEXTURE_DIMENSION, Quat, Transform, Vec3, VertexPosNormalUv};

use crate::error::ImportError;

/// Caps applied while importing untrusted data.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportLimits {
    /// Maximum size of any single input file or image buffer in bytes.
    pub max_file_bytes: u64,
    /// Maximum texture width/height in texels.
    pub max_texture_dimension: u32,
    /// Maximum decoder allocation in bytes.
    pub max_image_alloc: u64,
    /// Maximum number of imported nodes.
    pub max_nodes: usize,
    /// Maximum node hierarchy depth.
    pub max_depth: usize,
    /// Maximum vertices per primitive.
    pub max_vertices_per_mesh: u32,
    /// Maximum indices per primitive.
    pub max_indices_per_mesh: u32,
}

impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 256 * 1024 * 1024,
            max_texture_dimension: MAX_TEXTURE_DIMENSION,
            max_image_alloc: 256 * 1024 * 1024,
            max_nodes: 10_000,
            max_depth: 64,
            max_vertices_per_mesh: 8_388_608,
            max_indices_per_mesh: 67_108_864,
        }
    }
}

/// CPU mesh data (one glTF primitive).
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedMesh {
    /// Interleaved vertices.
    pub vertices: Vec<VertexPosNormalUv>,
    /// Triangle-list indices.
    pub indices: Vec<u32>,
    /// Index into [`ImportedModel::materials`].
    pub material: Option<usize>,
}

/// Base-color material.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedMaterial {
    /// Linear RGBA base color factor.
    pub base_color: [f32; 4],
    /// Index into [`ImportedModel::textures`].
    pub texture: Option<usize>,
}

/// Decoded RGBA8 texture.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedTexture {
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
    /// Tightly packed RGBA8 texels.
    pub rgba8: Vec<u8>,
}

/// Scene node.
///
/// Invariant: nodes are stored in pre-order, so `parent` is always less than the
/// node's own index.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedNode {
    /// Optional node name.
    pub name: Option<String>,
    /// Parent node index.
    pub parent: Option<usize>,
    /// Local transform.
    pub local: Transform,
    /// Indices into [`ImportedModel::meshes`].
    pub meshes: Vec<usize>,
}

/// Imported model.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportedModel {
    /// Meshes.
    pub meshes: Vec<ImportedMesh>,
    /// Materials.
    pub materials: Vec<ImportedMaterial>,
    /// Textures.
    pub textures: Vec<ImportedTexture>,
    /// Nodes in pre-order.
    pub nodes: Vec<ImportedNode>,
}

/// Validate a buffer/image URI against the confinement policy.
///
/// `data:` URIs are always allowed. Anything else requires a `base` directory and
/// must be a plain relative path. `%` is rejected because gltf 1.4.1 unwraps a
/// percent-decode that panics on invalid UTF-8.
///
/// # Errors
/// `ExternalReference` when there is no base; `UnsupportedUri` for unsafe paths.
pub fn check_uri(uri: &str, base: Option<&Path>) -> Result<(), ImportError> {
    if uri.starts_with("data:") {
        return Ok(());
    }
    if base.is_none() {
        return Err(ImportError::ExternalReference(uri.to_owned()));
    }
    let bad_char = uri
        .chars()
        .any(|c| matches!(c, ':' | '\\' | '%' | '?' | '#') || c == '\0');
    let bad_segment = uri
        .split('/')
        .any(|s| s.is_empty() || s == "." || s == "..");
    if uri.is_empty() || uri.starts_with('/') || bad_char || bad_segment {
        return Err(ImportError::UnsupportedUri(uri.to_owned()));
    }
    Ok(())
}

/// Decode a PNG/JPEG image with dimension, allocation and size caps.
///
/// # Errors
/// Size/dimension cap violations, unsupported formats or color types, decode errors.
pub fn decode_image(bytes: &[u8], limits: &ImportLimits) -> Result<ImportedTexture, ImportError> {
    use image::{ColorType, ImageFormat, ImageReader};

    let len = bytes.len() as u64;
    if len > limits.max_file_bytes {
        return Err(ImportError::FileTooLarge {
            size: len,
            max: limits.max_file_bytes,
        });
    }
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    match reader.format() {
        Some(ImageFormat::Png | ImageFormat::Jpeg) => {}
        other => {
            return Err(ImportError::UnsupportedImageFormat(format!("{other:?}")));
        }
    }
    let (w, h) = reader.into_dimensions()?;
    if w == 0 || h == 0 {
        return Err(ImportError::Image("zero-sized image".to_owned()));
    }
    let max = limits.max_texture_dimension;
    if w > max || h > max {
        return Err(ImportError::TextureTooLarge {
            width: w,
            height: h,
            max,
        });
    }

    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut lim = image::Limits::default();
    lim.max_image_width = Some(max);
    lim.max_image_height = Some(max);
    lim.max_alloc = Some(limits.max_image_alloc);
    reader.limits(lim);
    let img = reader.decode()?;
    match img.color() {
        ColorType::L8 | ColorType::La8 | ColorType::Rgb8 | ColorType::Rgba8 => {}
        other => {
            return Err(ImportError::UnsupportedImageFormat(format!("{other:?}")));
        }
    }
    let rgba = img.to_rgba8();
    Ok(ImportedTexture {
        width: rgba.width(),
        height: rgba.height(),
        rgba8: rgba.into_raw(),
    })
}

/// Import a `.glb`/`.gltf` file with default limits.
///
/// # Errors
/// See [`ImportError`].
pub fn import_gltf_path(path: &Path) -> Result<ImportedModel, ImportError> {
    import_gltf_path_with(path, &ImportLimits::default())
}

/// Import a `.glb`/`.gltf` file with explicit limits.
///
/// # Errors
/// See [`ImportError`].
pub fn import_gltf_path_with(
    path: &Path,
    limits: &ImportLimits,
) -> Result<ImportedModel, ImportError> {
    let size = std::fs::metadata(path)?.len();
    if size > limits.max_file_bytes {
        return Err(ImportError::FileTooLarge {
            size,
            max: limits.max_file_bytes,
        });
    }
    let bytes = std::fs::read(path)?;
    import_gltf_slice_with(&bytes, path.parent(), limits)
}

/// Import from bytes with default limits; external URIs are rejected.
///
/// # Errors
/// See [`ImportError`].
pub fn import_gltf_slice(bytes: &[u8]) -> Result<ImportedModel, ImportError> {
    import_gltf_slice_with(bytes, None, &ImportLimits::default())
}

fn check_external_size(
    uri: &str,
    base: Option<&Path>,
    limits: &ImportLimits,
) -> Result<(), ImportError> {
    check_uri(uri, base)?;
    if uri.starts_with("data:") {
        return Ok(());
    }
    if let Some(base) = base {
        let size = std::fs::metadata(base.join(uri))?.len();
        if size > limits.max_file_bytes {
            return Err(ImportError::FileTooLarge {
                size,
                max: limits.max_file_bytes,
            });
        }
    }
    Ok(())
}

/// Import from bytes. `base` is the directory relative URIs resolve against.
///
/// Supported: static triangle meshes, base-color factor and texture (TEXCOORD_0 only),
/// node TRS transforms. Everything else is ignored.
///
/// # Errors
/// See [`ImportError`]; never panics on malformed input.
pub fn import_gltf_slice_with(
    bytes: &[u8],
    base: Option<&Path>,
    limits: &ImportLimits,
) -> Result<ImportedModel, ImportError> {
    let len = bytes.len() as u64;
    if len > limits.max_file_bytes {
        return Err(ImportError::FileTooLarge {
            size: len,
            max: limits.max_file_bytes,
        });
    }
    let gltf::Gltf { document, blob } = gltf::Gltf::from_slice(bytes)?;

    // URI pre-flight, before gltf touches any URI.
    for buffer in document.buffers() {
        if let gltf::buffer::Source::Uri(uri) = buffer.source() {
            check_external_size(uri, base, limits)?;
        }
    }
    for image in document.images() {
        if let gltf::image::Source::Uri { uri, .. } = image.source() {
            check_external_size(uri, base, limits)?;
        }
    }
    let buffers = gltf::import_buffers(&document, base, blob)?;

    // Textures: only base-color images on TEXCOORD_0 are decoded.
    let mut wanted: Vec<usize> = Vec::new();
    for material in document.materials() {
        if let Some(info) = material.pbr_metallic_roughness().base_color_texture()
            && info.tex_coord() == 0
        {
            let idx = info.texture().source().index();
            if !wanted.contains(&idx) {
                wanted.push(idx);
            }
        }
    }
    wanted.sort_unstable();
    let mut model = ImportedModel::default();
    let mut image_map: HashMap<usize, usize> = HashMap::new();
    for image_index in wanted {
        let Some(image) = document.images().nth(image_index) else {
            return Err(ImportError::InvalidAccessor("image index out of range"));
        };
        let data: Vec<u8> = match image.source() {
            gltf::image::Source::View { view, .. } => {
                let buf = buffers
                    .get(view.buffer().index())
                    .ok_or(ImportError::InvalidAccessor("image buffer missing"))?;
                let oob = ImportError::InvalidAccessor("image buffer view out of range");
                let end = view.offset().checked_add(view.length()).ok_or(oob)?;
                buf.get(view.offset()..end)
                    .ok_or(ImportError::InvalidAccessor(
                        "image buffer view out of range",
                    ))?
                    .to_vec()
            }
            gltf::image::Source::Uri { uri, .. } => {
                gltf::buffer::Data::from_source(gltf::buffer::Source::Uri(uri), base)?
                    .0
                    .clone()
            }
        };
        let tex = decode_image(&data, limits)?;
        image_map.insert(image_index, model.textures.len());
        model.textures.push(tex);
    }

    for material in document.materials() {
        let pbr = material.pbr_metallic_roughness();
        let base_color = pbr.base_color_factor();
        if !base_color.iter().all(|c| c.is_finite()) {
            return Err(ImportError::NonFinite("material base color"));
        }
        let texture = pbr
            .base_color_texture()
            .filter(|info| info.tex_coord() == 0)
            .and_then(|info| image_map.get(&info.texture().source().index()).copied());
        model.materials.push(ImportedMaterial {
            base_color,
            texture,
        });
    }

    let mut mesh_map: Vec<Vec<usize>> = Vec::new();
    for mesh in document.meshes() {
        let mut mapped = Vec::new();
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                engine_sdk::tracing::debug!("skipping non-triangle primitive");
                continue;
            }
            let reader = primitive.reader(|b| buffers.get(b.index()).map(|d| &d[..]));
            let Some(pos_iter) = reader.read_positions() else {
                engine_sdk::tracing::debug!("skipping primitive without positions");
                continue;
            };
            let n = pos_iter.len();
            if n > limits.max_vertices_per_mesh as usize {
                return Err(ImportError::TooManyVertices {
                    mesh: mesh.index(),
                    count: n,
                    max: limits.max_vertices_per_mesh,
                });
            }
            let positions: Vec<[f32; 3]> = pos_iter.collect();
            if !positions.iter().flatten().all(|c| c.is_finite()) {
                return Err(ImportError::NonFinite("position"));
            }
            let uvs: Vec<[f32; 2]> = match reader.read_tex_coords(0) {
                Some(t) => {
                    let v: Vec<[f32; 2]> = t.into_f32().collect();
                    if v.len() != n {
                        return Err(ImportError::InvalidAccessor("TEXCOORD_0 count mismatch"));
                    }
                    v
                }
                None => vec![[0.0, 0.0]; n],
            };
            if !uvs.iter().flatten().all(|c| c.is_finite()) {
                return Err(ImportError::NonFinite("texcoord"));
            }
            let indices: Vec<u32> = match reader.read_indices() {
                Some(i) => {
                    let it = i.into_u32();
                    if it.len() > limits.max_indices_per_mesh as usize {
                        return Err(ImportError::TooManyIndices {
                            mesh: mesh.index(),
                            count: it.len(),
                            max: limits.max_indices_per_mesh,
                        });
                    }
                    it.collect()
                }
                None => {
                    if n > limits.max_indices_per_mesh as usize {
                        return Err(ImportError::TooManyIndices {
                            mesh: mesh.index(),
                            count: n,
                            max: limits.max_indices_per_mesh,
                        });
                    }
                    (0..n as u32).collect()
                }
            };
            if !indices.len().is_multiple_of(3) {
                return Err(ImportError::InvalidAccessor(
                    "index count not a multiple of 3",
                ));
            }
            if let Some(&bad) = indices.iter().find(|&&i| i as usize >= n) {
                return Err(ImportError::InvalidIndex {
                    mesh: mesh.index(),
                    index: bad,
                    vertex_count: n as u32,
                });
            }
            let normals: Vec<[f32; 3]> = match reader.read_normals() {
                Some(it) => {
                    let v: Vec<[f32; 3]> = it.collect();
                    if v.len() == n && v.iter().flatten().all(|c| c.is_finite()) {
                        v
                    } else {
                        generate_normals(&positions, &indices)
                    }
                }
                None => generate_normals(&positions, &indices),
            };
            let vertices = positions
                .iter()
                .zip(&normals)
                .zip(&uvs)
                .map(|((p, nm), uv)| VertexPosNormalUv {
                    position: *p,
                    normal: *nm,
                    uv: *uv,
                })
                .collect();
            mapped.push(model.meshes.len());
            model.meshes.push(ImportedMesh {
                vertices,
                indices,
                material: primitive.material().index(),
            });
        }
        mesh_map.push(mapped);
    }

    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next());
    if let Some(scene) = scene {
        // Iterative pre-order DFS: children pushed in reverse so document order is kept.
        let mut stack: Vec<(gltf::Node<'_>, Option<usize>, usize)> =
            scene.nodes().map(|n| (n, None, 0)).collect();
        stack.reverse();
        let mut visited: HashSet<usize> = HashSet::new();
        while let Some((node, parent, depth)) = stack.pop() {
            if !visited.insert(node.index()) {
                return Err(ImportError::HierarchyCycle);
            }
            if depth > limits.max_depth {
                return Err(ImportError::HierarchyTooDeep {
                    max: limits.max_depth,
                });
            }
            if model.nodes.len() >= limits.max_nodes {
                return Err(ImportError::TooManyNodes {
                    max: limits.max_nodes,
                });
            }
            let (t, r, s) = node.transform().decomposed();
            let rotation = Quat::from_array(r);
            let local = Transform::new(
                DVec3::new(f64::from(t[0]), f64::from(t[1]), f64::from(t[2])),
                rotation,
                Vec3::from_array(s),
            );
            if !local.is_finite() || rotation.length() < 1e-6 {
                return Err(ImportError::NonFinite("node transform"));
            }
            let local = Transform::new(local.translation, rotation.normalize(), local.scale);
            let meshes = match node.mesh() {
                Some(m) => mesh_map
                    .get(m.index())
                    .ok_or(ImportError::InvalidAccessor("mesh index out of range"))?
                    .clone(),
                None => Vec::new(),
            };
            let own = model.nodes.len();
            model.nodes.push(ImportedNode {
                name: node.name().map(str::to_owned),
                parent,
                local,
                meshes,
            });
            let mark = stack.len();
            for child in node.children() {
                stack.push((child, Some(own), depth + 1));
            }
            if let Some(tail) = stack.get_mut(mark..) {
                tail.reverse();
            }
        }
    }
    Ok(model)
}

/// Area-weighted vertex normals; `(0, 1, 0)` where no usable face contributes.
///
/// Indices must already be validated against `positions.len()`.
fn generate_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut acc = vec![[0.0f32; 3]; positions.len()];
    for tri in indices.as_chunks::<3>().0 {
        let (Some(a), Some(b), Some(c)) = (
            positions.get(tri[0] as usize),
            positions.get(tri[1] as usize),
            positions.get(tri[2] as usize),
        ) else {
            continue;
        };
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        for &i in tri {
            if let Some(slot) = acc.get_mut(i as usize) {
                slot[0] += n[0];
                slot[1] += n[1];
                slot[2] += n[2];
            }
        }
    }
    acc.into_iter()
        .map(|n| {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len.is_finite() && len > 1e-12 {
                [n[0] / len, n[1] / len, n[2] / len]
            } else {
                [0.0, 1.0, 0.0]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

    fn png(w: u32, h: u32, color: ExtendedColorType, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        PngEncoder::new(&mut out)
            .write_image(data, w, h, color)
            .expect("encode");
        out
    }

    #[test]
    fn uri_policy() {
        assert!(check_uri("data:application/octet-stream;base64,AAAA", None).is_ok());
        assert!(matches!(
            check_uri("cube.bin", None),
            Err(ImportError::ExternalReference(_))
        ));
        let base = Some(Path::new("."));
        assert!(check_uri("cube.bin", base).is_ok());
        assert!(check_uri("sub/tex.png", base).is_ok());
        for bad in [
            "../secret.bin",
            "/abs.bin",
            "file:///etc/passwd",
            "C:\\x.bin",
            "%FF.bin",
            "a/./b",
            "",
            "a//b",
            "a?b",
            "a#b",
        ] {
            assert!(
                matches!(check_uri(bad, base), Err(ImportError::UnsupportedUri(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn decode_limits() {
        let limits = ImportLimits::default();
        let wide = png(9000, 1, ExtendedColorType::L8, &vec![0u8; 9000]);
        assert!(matches!(
            decode_image(&wide, &limits),
            Err(ImportError::TextureTooLarge { .. })
        ));
        let small = png(8, 8, ExtendedColorType::Rgb8, &[100u8; 8 * 8 * 3]);
        let tight = ImportLimits {
            max_texture_dimension: 4,
            ..ImportLimits::default()
        };
        assert!(matches!(
            decode_image(&small, &tight),
            Err(ImportError::TextureTooLarge { .. })
        ));
        assert!(matches!(
            decode_image(b"not an image at all", &limits),
            Err(ImportError::Image(_) | ImportError::UnsupportedImageFormat(_))
        ));
        let tex = decode_image(&small, &limits).expect("decode");
        assert_eq!((tex.width, tex.height), (8, 8));
        assert_eq!(tex.rgba8.len(), 256);
        assert!(tex.rgba8.chunks(4).all(|p| p[3] == 255));
    }

    #[test]
    fn generate_normals_ccw_and_degenerate() {
        let p = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let n = generate_normals(&p, &[0, 1, 2]);
        assert!(n.iter().all(|v| *v == [0.0, 0.0, 1.0]));
        let d = [[0.0; 3]; 3];
        let n = generate_normals(&d, &[0, 1, 2]);
        assert!(n.iter().all(|v| *v == [0.0, 1.0, 0.0]));
    }

    #[test]
    fn file_size_cap() {
        let small = png(2, 2, ExtendedColorType::L8, &[0u8; 4]);
        let limits = ImportLimits {
            max_file_bytes: 4,
            ..ImportLimits::default()
        };
        assert!(matches!(
            decode_image(&small, &limits),
            Err(ImportError::FileTooLarge { .. })
        ));
    }
}

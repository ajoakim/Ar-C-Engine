//! glTF 2.0 import into engine-shaped CPU data.
//!
//! All input is untrusted: sizes, dimensions, counts and URIs are bounded by
//! [`ImportLimits`] and [`check_uri`]; document data is never indexed unchecked.
//! Images are decoded by [`decode_image`] (not by `gltf::import`), so every decode
//! is bounded.

use std::io::Cursor;
use std::path::Path;

use engine_sdk::{MAX_TEXTURE_DIMENSION, Transform, VertexPosNormalUv};

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

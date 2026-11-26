//! Test-only GLB builders (deterministic, no external tools).
#![allow(dead_code)]

use conventional_3d::procedural::{checker_texture, cube_mesh};

/// Build a GLB v2 container. The BIN chunk is omitted when `bin` is empty.
pub fn glb(json: &str, bin: &[u8]) -> Vec<u8> {
    let mut json_bytes = json.as_bytes().to_vec();
    while !json_bytes.len().is_multiple_of(4) {
        json_bytes.push(b' ');
    }
    let mut bin_bytes = bin.to_vec();
    while !bin_bytes.len().is_multiple_of(4) {
        bin_bytes.push(0);
    }
    let mut total = 12 + 8 + json_bytes.len();
    if !bin_bytes.is_empty() {
        total += 8 + bin_bytes.len();
    }
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    out.extend_from_slice(&json_bytes);
    if !bin_bytes.is_empty() {
        out.extend_from_slice(&(bin_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
        out.extend_from_slice(&bin_bytes);
    }
    out
}

/// Standard base64 with padding.
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

/// Encode RGBA8 pixels as PNG.
pub fn png_rgba(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(rgba, w, h, image::ExtendedColorType::Rgba8)
        .expect("png encode");
    out
}

fn f32_bytes(values: impl IntoIterator<Item = f32>) -> Vec<u8> {
    values.into_iter().flat_map(f32::to_le_bytes).collect()
}

fn pad4(v: &mut Vec<u8>) {
    while !v.len().is_multiple_of(4) {
        v.push(0);
    }
}

/// The checked-in fixture: a textured cube under a two-node hierarchy.
pub fn textured_cube_glb() -> Vec<u8> {
    let mesh = cube_mesh(1.0);
    let tex = checker_texture(8, [230, 230, 230, 255], [40, 90, 200, 255]);
    let png = png_rgba(tex.width, tex.height, &tex.rgba8);

    let mut bin = Vec::new();
    let mut views = Vec::new();
    let mut push_view = |bin: &mut Vec<u8>, data: &[u8]| {
        pad4(bin);
        views.push((bin.len(), data.len()));
        bin.extend_from_slice(data);
    };
    push_view(
        &mut bin,
        &f32_bytes(mesh.vertices.iter().flat_map(|v| v.position)),
    );
    push_view(
        &mut bin,
        &f32_bytes(mesh.vertices.iter().flat_map(|v| v.normal)),
    );
    push_view(
        &mut bin,
        &f32_bytes(mesh.vertices.iter().flat_map(|v| v.uv)),
    );
    let idx: Vec<u8> = mesh
        .indices
        .iter()
        .flat_map(|&i| (i as u16).to_le_bytes())
        .collect();
    push_view(&mut bin, &idx);
    push_view(&mut bin, &png);
    pad4(&mut bin);

    let v = |i: usize| views[i];
    let json = format!(
        concat!(
            "{{\"asset\":{{\"version\":\"2.0\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],",
            "\"nodes\":[{{\"name\":\"root\",\"children\":[1]}},",
            "{{\"name\":\"cube\",\"mesh\":0,\"translation\":[0,0.5,0],",
            "\"rotation\":[0,0.38268343,0,0.9238795]}}],",
            "\"meshes\":[{{\"primitives\":[{{\"attributes\":{{\"POSITION\":0,\"NORMAL\":1,",
            "\"TEXCOORD_0\":2}},\"indices\":3,\"material\":0,\"mode\":4}}]}}],",
            "\"materials\":[{{\"pbrMetallicRoughness\":{{\"baseColorFactor\":[1,1,1,1],",
            "\"baseColorTexture\":{{\"index\":0}}}}}}],",
            "\"textures\":[{{\"source\":0}}],",
            "\"images\":[{{\"bufferView\":4,\"mimeType\":\"image/png\"}}],",
            "\"accessors\":[",
            "{{\"bufferView\":0,\"componentType\":5126,\"count\":24,\"type\":\"VEC3\",",
            "\"min\":[-0.5,-0.5,-0.5],\"max\":[0.5,0.5,0.5]}},",
            "{{\"bufferView\":1,\"componentType\":5126,\"count\":24,\"type\":\"VEC3\"}},",
            "{{\"bufferView\":2,\"componentType\":5126,\"count\":24,\"type\":\"VEC2\"}},",
            "{{\"bufferView\":3,\"componentType\":5123,\"count\":36,\"type\":\"SCALAR\"}}],",
            "\"bufferViews\":[",
            "{{\"buffer\":0,\"byteOffset\":{o0},\"byteLength\":{l0}}},",
            "{{\"buffer\":0,\"byteOffset\":{o1},\"byteLength\":{l1}}},",
            "{{\"buffer\":0,\"byteOffset\":{o2},\"byteLength\":{l2}}},",
            "{{\"buffer\":0,\"byteOffset\":{o3},\"byteLength\":{l3}}},",
            "{{\"buffer\":0,\"byteOffset\":{o4},\"byteLength\":{l4}}}],",
            "\"buffers\":[{{\"byteLength\":{total}}}]}}"
        ),
        o0 = v(0).0,
        l0 = v(0).1,
        o1 = v(1).0,
        l1 = v(1).1,
        o2 = v(2).0,
        l2 = v(2).1,
        o3 = v(3).0,
        l3 = v(3).1,
        o4 = v(4).0,
        l4 = v(4).1,
        total = bin.len(),
    );
    glb(&json, &bin)
}

/// POSITION-only triangle (0,0,0),(1,0,0),(0,1,0) with optional u16 indices.
///
/// `mode` applies to the triangle primitive; `extra_line_primitive` adds a leading
/// LINES primitive over the same positions.
pub fn triangle_glb(indices: Option<&[u16]>, mode: u32, extra_line_primitive: bool) -> Vec<u8> {
    let mut bin = f32_bytes([0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let mut accessors = String::from(
        "{\"bufferView\":0,\"componentType\":5126,\"count\":3,\"type\":\"VEC3\",\
         \"min\":[0,0,0],\"max\":[1,1,0]}",
    );
    let mut views = String::from("{\"buffer\":0,\"byteOffset\":0,\"byteLength\":36}");
    let mut tri_attrs = String::from("\"attributes\":{\"POSITION\":0}");
    if let Some(ix) = indices {
        let data: Vec<u8> = ix.iter().flat_map(|i| i.to_le_bytes()).collect();
        let off = bin.len();
        bin.extend_from_slice(&data);
        views.push_str(&format!(
            ",{{\"buffer\":0,\"byteOffset\":{off},\"byteLength\":{}}}",
            data.len()
        ));
        accessors.push_str(&format!(
            ",{{\"bufferView\":1,\"componentType\":5123,\"count\":{},\"type\":\"SCALAR\"}}",
            ix.len()
        ));
        tri_attrs.push_str(",\"indices\":1");
    }
    pad4(&mut bin);
    let mut prims = String::new();
    if extra_line_primitive {
        prims.push_str("{\"attributes\":{\"POSITION\":0},\"mode\":1},");
    }
    prims.push_str(&format!("{{{tri_attrs},\"mode\":{mode}}}"));
    let json = format!(
        "{{\"asset\":{{\"version\":\"2.0\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],\
         \"nodes\":[{{\"mesh\":0}}],\"meshes\":[{{\"primitives\":[{prims}]}}],\
         \"accessors\":[{accessors}],\"bufferViews\":[{views}],\
         \"buffers\":[{{\"byteLength\":{}}}]}}",
        bin.len()
    );
    glb(&json, &bin)
}

/// Linear chain of `depth` nodes (node i has child i+1).
pub fn chain_glb(depth: usize) -> Vec<u8> {
    let mut nodes = String::new();
    for i in 0..depth {
        if i > 0 {
            nodes.push(',');
        }
        if i + 1 < depth {
            nodes.push_str(&format!("{{\"children\":[{}]}}", i + 1));
        } else {
            nodes.push_str("{}");
        }
    }
    let json = format!(
        "{{\"asset\":{{\"version\":\"2.0\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],\
         \"nodes\":[{nodes}]}}"
    );
    glb(&json, &[])
}

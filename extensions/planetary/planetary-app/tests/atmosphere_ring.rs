//! PLAN-08 image scene: the limb ring is occluded by the planet disc (reverse-Z) and fades to the
//! clear colour at its outer edge, rendered on the software adapter (WARP / lavapipe).
//!
//! Lives in planetary-app (not in `engine-render-wgpu/tests/image_diff.rs`) because the ring
//! builder is planetary-app code and `crates/` may never depend on `extensions/` (R1). It reuses
//! the plan 03 skip and require logic:
//! - `ARC_REQUIRE_ADAPTER=1`: a missing software adapter fails instead of skipping.
//! - `ARC_UPDATE_GOLDENS=1`: rewrite the golden.
//! - `ARC_REQUIRE_GOLDENS=1`: a missing golden fails instead of skipping.
//!
//! The two pixel assertions (disc occludes the ring, the ring is tinted outside the disc) run
//! whenever an adapter exists, independent of any golden.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use engine_render::{
    BufferDesc, BufferUsage, DirectionalLight, DistanceFog, DrawCommand, FramePacket, Mat3, Mat4,
    MeshHandle, PassPacket, PipelineDesc, RenderBackend, ShaderId, TextureId, VertexLayout,
    VertexPosColor, encode_vertices,
};
use engine_render_wgpu::WgpuBackend;
use planetary_app::SPACE_COLOR;
use planetary_app::atmosphere::{ring_outer_ratio, ring_vertices, sky_color};

const W: usize = 64;
/// Per-channel absolute tolerance (of 255), as in `image_diff.rs`.
const CHANNEL_TOLERANCE: u8 = 2;
/// Maximum fraction of pixels allowed to exceed the channel tolerance (0.5%).
const MAX_BAD_FRACTION: f64 = 0.005;

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == "1")
}

fn platform_dir() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn golden_path(scene: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(platform_dir())
        .join(format!("{scene}.png"))
}

fn artifact_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("target")
        .join("image-diff")
}

fn write_png(path: &Path, rgba: &[u8]) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).expect("create dir");
    }
    let file = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), W as u32, W as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().expect("png header");
    writer.write_image_data(rgba).expect("png data");
}

fn read_png(path: &Path) -> Result<(Vec<u8>, u32, u32), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut reader = png::Decoder::new(Cursor::new(bytes))
        .read_info()
        .map_err(|e| format!("decode {}: {e}", path.display()))?;
    let (color, depth) = reader.output_color_type();
    if color != png::ColorType::Rgba || depth != png::BitDepth::Eight {
        return Err(format!("expected RGBA8, got {color:?} {depth:?}"));
    }
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| "png too large".to_string())?;
    let mut buf = vec![0u8; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("decode {}: {e}", path.display()))?;
    buf.truncate(info.buffer_size());
    Ok((buf, info.width, info.height))
}

/// `(max channel delta, pixels over tolerance)`, or an error for mismatched sizes.
fn compare(actual: &[u8], golden: &[u8]) -> Result<(u8, usize), String> {
    let want = W * W * 4;
    if actual.len() != want || golden.len() != want {
        return Err(format!(
            "size mismatch: expected {want} bytes, actual {} golden {}",
            actual.len(),
            golden.len()
        ));
    }
    let mut max_delta = 0u8;
    let mut bad = 0usize;
    for (a, g) in actual
        .as_chunks::<4>()
        .0
        .iter()
        .zip(golden.as_chunks::<4>().0)
    {
        let d = (0..4).map(|c| a[c].abs_diff(g[c])).max().unwrap_or(0);
        max_delta = max_delta.max(d);
        if d > CHANNEL_TOLERANCE {
            bad += 1;
        }
    }
    Ok((max_delta, bad))
}

fn adapter_or_skip() -> Option<WgpuBackend> {
    match WgpuBackend::new_offscreen_software(W as u32, W as u32) {
        Ok(b) => {
            println!("atmosphere_ring adapter: {}", b.adapter_summary());
            Some(b)
        }
        Err(e) => {
            assert!(
                !env_flag("ARC_REQUIRE_ADAPTER"),
                "atmosphere_ring: ARC_REQUIRE_ADAPTER=1 but no software adapter: {e}"
            );
            println!("SKIP atmosphere_ring: no software adapter: {e}");
            None
        }
    }
}

fn check_golden(scene: &str, actual: &[u8]) {
    let path = golden_path(scene);
    let exists = path.exists();
    if env_flag("ARC_UPDATE_GOLDENS") {
        write_png(&path, actual);
        println!("atmosphere_ring {scene}: wrote golden {}", path.display());
    } else if !exists {
        let msg = format!(
            "atmosphere_ring {scene}: no golden at {} (ARC_UPDATE_GOLDENS=1 creates it)",
            path.display()
        );
        assert!(!env_flag("ARC_REQUIRE_GOLDENS"), "{msg}");
        println!("SKIP {msg}");
    } else {
        let (golden, gw, gh) = read_png(&path).expect("golden decodes");
        assert_eq!((gw as usize, gh as usize), (W, W), "golden size");
        let (max_delta, bad) = compare(actual, &golden).expect("sizes match");
        let frac = bad as f64 / (W * W) as f64;
        if frac > MAX_BAD_FRACTION {
            write_png(&artifact_dir().join(format!("{scene}-actual.png")), actual);
            panic!(
                "atmosphere_ring {scene}: {bad} pixels differ by more than {CHANNEL_TOLERANCE} (max delta {max_delta})"
            );
        }
        println!(
            "atmosphere_ring {scene}: ok (max channel delta {max_delta}, {bad} over tolerance)"
        );
    }
}

// ---------------------------------------------------------------- scene

/// Disc radius, ring inner radius and ring outer ratio in clip space.
const DISC_R: f32 = 0.5;
const RING_INNER: f32 = 0.45;
const OUTER_RATIO: f32 = 1.8;
const DISC_COLOR: [f32; 3] = [0.5, 0.5, 0.5];

fn disc_vertices() -> Vec<VertexPosColor> {
    let n = 64u32;
    let at = |i: u32| {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        VertexPosColor {
            position: [a.cos() * DISC_R, a.sin() * DISC_R, 0.0],
            color: DISC_COLOR,
        }
    };
    let c = VertexPosColor {
        position: [0.0, 0.0, 0.0],
        color: DISC_COLOR,
    };
    (0..n).flat_map(|i| [c, at(i), at(i + 1)]).collect()
}

fn mesh(b: &mut WgpuBackend, verts: &[VertexPosColor]) -> MeshHandle {
    let vb = b
        .create_buffer(
            &BufferDesc {
                label: "mesh",
                usage: BufferUsage::Vertex,
            },
            &encode_vertices(verts),
        )
        .expect("vertex buffer");
    MeshHandle::new(VertexLayout::PosColor, vb, verts.len() as u32)
}

fn draw(pipeline: engine_render::PipelineId, mesh: MeshHandle, mvp: Mat4) -> DrawCommand {
    DrawCommand {
        pipeline,
        mesh,
        mvp,
        model: Mat4::IDENTITY,
        normal: Mat3::IDENTITY,
        base_color: [1.0; 4],
        texture: None,
        morph: 0.0,
    }
}

/// Disc at nearer depth (higher z under reverse-Z) than the ring plane; the ring is drawn AFTER
/// the disc so only the depth test (not draw order) can keep it out of the disc.
fn scene_atmosphere_ring(b: &mut WgpuBackend) -> Vec<u8> {
    let unlit = b
        .create_pipeline(&PipelineDesc {
            label: "unlit",
            shader: ShaderId::UnlitVertexColor,
            vertex_layout: VertexLayout::PosColor,
        })
        .expect("unlit pipeline");
    let clear = [SPACE_COLOR[0], SPACE_COLOR[1], SPACE_COLOR[2]];
    let sun = engine_sdk_light();
    let up_to_color =
        |c: f32, s: f32| sky_color(planetary_app_dvec(f64::from(c), f64::from(s), 0.0), sun);
    let ring = ring_vertices(64, OUTER_RATIO, &up_to_color, clear);
    let disc_mesh = mesh(b, &disc_vertices());
    let ring_mesh = mesh(b, &ring);
    let disc_mvp = Mat4::from_translation(glam_vec3(0.0, 0.0, 0.6));
    let ring_mvp = Mat4::from_translation(glam_vec3(0.0, 0.0, 0.4))
        * Mat4::from_scale(glam_vec3(RING_INNER, RING_INNER, 1.0));
    let packet = FramePacket {
        frame_index: 0,
        light: DirectionalLight::default(),
        fog: DistanceFog::default(),
        passes: vec![PassPacket {
            name: "main".to_string(),
            color_target: TextureId::BACKBUFFER,
            depth_target: Some(TextureId::DEPTH),
            clear_color: SPACE_COLOR,
            clear_depth: 0.0,
            draws: vec![
                draw(unlit, disc_mesh, disc_mvp),
                draw(unlit, ring_mesh, ring_mvp),
            ],
        }],
    };
    b.submit(&packet).expect("submit");
    let buf = b.read_offscreen_rgba8().expect("readback");
    assert_eq!(buf.len(), W * W * 4);
    buf
}

fn engine_sdk_light() -> engine_sdk::Vec3 {
    // Light travels towards -X and -Z: the sun stands at 53 degrees above the +X limb (day
    // sky, plain colour) and below the horizon on the opposite limb.
    engine_sdk::Vec3::new(-0.8, 0.0, -0.6)
}

fn planetary_app_dvec(x: f64, y: f64, z: f64) -> engine_sdk::DVec3 {
    engine_sdk::DVec3::new(x, y, z)
}

fn glam_vec3(x: f32, y: f32, z: f32) -> engine_sdk::Vec3 {
    engine_sdk::Vec3::new(x, y, z)
}

fn pixel(buf: &[u8], x: usize, y: usize) -> [u8; 4] {
    let o = (y * W + x) * 4;
    [buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]
}

/// Pixel column for a clip-space x.
fn col(clip_x: f32) -> usize {
    ((clip_x + 1.0) * 0.5 * W as f32) as usize
}

#[test]
fn atmosphere_ring_scene() {
    let Some(mut b) = adapter_or_skip() else {
        return;
    };
    assert!(ring_outer_ratio(6_371_000.0) > 1.0);
    let img = scene_atmosphere_ring(&mut b);
    let row = W / 2;
    let corner = pixel(&img, 0, 0);
    let centre = pixel(&img, W / 2, row);
    // Inside the disc, over the ring's inner band (clip x 0.47 is inside both): disc colour, not
    // ring colour, so the planet occludes the ring.
    let overlap = pixel(&img, col(0.47), row);
    assert_eq!(overlap, centre, "disc must occlude the ring");
    assert_ne!(centre, corner);
    // Outside the disc and inside the ring (clip x 0.60): ring tint, neither disc nor clear.
    let ring = pixel(&img, col(0.60), row);
    assert_ne!(ring, centre, "ring pixel equals disc pixel");
    assert_ne!(ring, corner, "ring pixel equals clear pixel");
    assert!(ring[2] > ring[0], "ring is blue-tinted: {ring:?}");
    // Beyond the outer edge (clip x 0.95 > 0.45 * 1.8 = 0.81): clear colour.
    assert_eq!(pixel(&img, col(0.95), row), corner);
    check_golden("atmosphere_ring", &img);
}

#[test]
fn compare_detects_a_perturbed_block() {
    let golden = vec![100u8; W * W * 4];
    let mut actual = golden.clone();
    assert_eq!(compare(&actual, &golden), Ok((0, 0)));
    for y in 0..8 {
        for x in 0..8 {
            actual[(y * W + x) * 4] = 140;
        }
    }
    let (max, bad) = compare(&actual, &golden).unwrap();
    assert_eq!((max, bad), (40, 64));
    assert!(bad as f64 / (W * W) as f64 > MAX_BAD_FRACTION);
    assert!(compare(&actual[..16], &golden).is_err());
}

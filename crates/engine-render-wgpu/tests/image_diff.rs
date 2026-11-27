//! Image-diff regression tests on the software adapter only (WARP on Windows, lavapipe on Linux).
//!
//! Scenes are rendered at a fixed size, read back as RGBA8 and compared with per-platform goldens
//! in `tests/golden/<os>/<scene>.png`. Goldens never come from hardware adapters.
//!
//! Environment switches:
//! - `ARC_REQUIRE_ADAPTER=1`: a missing software adapter fails instead of skipping.
//! - `ARC_UPDATE_GOLDENS=1`: rewrite goldens (only the workflow_dispatch CI step sets this).
//! - `ARC_REQUIRE_GOLDENS=1`: a missing golden fails instead of skipping.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use engine_render::{
    BufferDesc, BufferId, BufferUsage, DirectionalLight, DistanceFog, DrawCommand, FramePacket,
    Mat3, Mat4, MeshHandle, PassPacket, PipelineDesc, PipelineId, RenderBackend, ShaderId,
    TextureDesc, TextureId, VertexLayout, VertexPosColor, VertexPosNormalUv,
    VertexPosNormalUvMorph, encode_indices, encode_lit_morph_vertices, encode_lit_vertices,
    encode_vertices,
};
use engine_render_wgpu::WgpuBackend;

const W: usize = 64;

/// Per-channel absolute tolerance (of 255). Calibrated on WARP: see the calibration note in
/// 08-03-SUMMARY (observed max delta between runs); 2 only absorbs driver updates.
const CHANNEL_TOLERANCE: u8 = 2;
/// Maximum fraction of pixels allowed to exceed the channel tolerance (0.5%).
const MAX_BAD_FRACTION: f64 = 0.005;

// ---------------------------------------------------------------- harness

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GoldenAction {
    Compare,
    Write,
    Skip,
    Fail,
}

/// Missing-golden rule. Update mode always writes; otherwise an existing golden is compared and a
/// missing one skips (with a printed message) unless goldens are required.
fn golden_action(exists: bool, update: bool, require_goldens: bool) -> GoldenAction {
    match (exists, update) {
        (_, true) => GoldenAction::Write,
        (true, false) => GoldenAction::Compare,
        (false, false) if require_goldens => GoldenAction::Fail,
        (false, false) => GoldenAction::Skip,
    }
}

#[derive(Debug)]
struct DiffStats {
    max_delta: u8,
    bad_pixels: usize,
}

#[derive(Debug)]
struct DiffReport {
    message: String,
    /// RGBA8 visualisation (differing pixels red, others dimmed) when sizes matched.
    diff: Option<Vec<u8>>,
}

/// Compare two RGBA8 buffers of `w` x `h`. Dimensions are validated before any indexing.
fn compare(
    actual: &[u8],
    golden: &[u8],
    (w, h): (usize, usize),
    tol: u8,
    max_bad_frac: f64,
) -> Result<DiffStats, DiffReport> {
    let want = w * h * 4;
    if actual.len() != want || golden.len() != want {
        return Err(DiffReport {
            message: format!(
                "size mismatch: expected {want} bytes ({w}x{h}), actual {} golden {}",
                actual.len(),
                golden.len()
            ),
            diff: None,
        });
    }
    let mut max_delta = 0u8;
    let mut bad = 0usize;
    let mut diff = vec![0u8; want];
    for (i, (a, g)) in actual
        .chunks_exact(4)
        .zip(golden.chunks_exact(4))
        .enumerate()
    {
        let d = (0..4).map(|c| a[c].abs_diff(g[c])).max().unwrap_or(0);
        max_delta = max_delta.max(d);
        let o = i * 4;
        if d > tol {
            bad += 1;
            diff[o..o + 4].copy_from_slice(&[255, 0, 0, 255]);
        } else {
            diff[o..o + 4].copy_from_slice(&[a[0] / 4, a[1] / 4, a[2] / 4, 255]);
        }
    }
    let frac = bad as f64 / (w * h) as f64;
    if frac > max_bad_frac {
        return Err(DiffReport {
            message: format!(
                "{bad} pixels differ by more than {tol} ({:.3}% > {:.3}% budget), max delta {max_delta}",
                frac * 100.0,
                max_bad_frac * 100.0
            ),
            diff: Some(diff),
        });
    }
    Ok(DiffStats {
        max_delta,
        bad_pixels: bad,
    })
}

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
        .join("target")
        .join("image-diff")
}

fn write_png(path: &Path, rgba: &[u8], w: u32, h: u32) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).expect("create dir");
    }
    let file = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().expect("png header");
    writer.write_image_data(rgba).expect("png data");
}

/// Decode an RGBA8 PNG. Returns (pixels, width, height) or a descriptive error.
fn read_png(path: &Path) -> Result<(Vec<u8>, u32, u32), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut reader = png::Decoder::new(Cursor::new(bytes))
        .read_info()
        .map_err(|e| format!("decode {}: {e}", path.display()))?;
    let (color, depth) = reader.output_color_type();
    if color != png::ColorType::Rgba || depth != png::BitDepth::Eight {
        return Err(format!(
            "{}: expected RGBA8, got {color:?} {depth:?}",
            path.display()
        ));
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

/// Software backend, or None with a printed skip reason (panics under ARC_REQUIRE_ADAPTER=1).
fn adapter_or_skip() -> Option<WgpuBackend> {
    match WgpuBackend::new_offscreen_software(W as u32, W as u32) {
        Ok(b) => {
            println!("image_diff adapter: {}", b.adapter_summary());
            Some(b)
        }
        Err(e) => {
            assert!(
                !env_flag("ARC_REQUIRE_ADAPTER"),
                "image_diff: ARC_REQUIRE_ADAPTER=1 but no software adapter: {e}"
            );
            println!("SKIP image_diff: no software adapter: {e}");
            None
        }
    }
}

/// Check `actual` against the golden for `scene` following the missing-golden rule.
fn check_scene(scene: &str, actual: &[u8]) {
    let path = golden_path(scene);
    let action = golden_action(
        path.exists(),
        env_flag("ARC_UPDATE_GOLDENS"),
        env_flag("ARC_REQUIRE_GOLDENS"),
    );
    let missing = format!(
        "image_diff {scene}: no golden at {} (ARC_UPDATE_GOLDENS=1 creates it; ARC_REQUIRE_GOLDENS=1 makes this a failure)",
        path.display()
    );
    match action {
        GoldenAction::Skip => println!("SKIP {missing}"),
        GoldenAction::Fail => panic!("{missing}"),
        GoldenAction::Write => {
            write_png(&path, actual, W as u32, W as u32);
            println!("image_diff {scene}: wrote golden {}", path.display());
        }
        GoldenAction::Compare => {
            let result = read_png(&path)
                .map_err(|message| DiffReport {
                    message,
                    diff: None,
                })
                .and_then(|(golden, gw, gh)| {
                    if (gw as usize, gh as usize) != (W, W) {
                        return Err(DiffReport {
                            message: format!("golden is {gw}x{gh}, expected {W}x{W}"),
                            diff: None,
                        });
                    }
                    compare(actual, &golden, (W, W), CHANNEL_TOLERANCE, MAX_BAD_FRACTION)
                });
            match result {
                Ok(s) => println!(
                    "image_diff {scene}: ok (max channel delta {}, {} pixels over tolerance)",
                    s.max_delta, s.bad_pixels
                ),
                Err(r) => {
                    let dir = artifact_dir();
                    write_png(
                        &dir.join(format!("{scene}-actual.png")),
                        actual,
                        W as u32,
                        W as u32,
                    );
                    if let Some(d) = &r.diff {
                        write_png(
                            &dir.join(format!("{scene}-diff.png")),
                            d,
                            W as u32,
                            W as u32,
                        );
                    }
                    panic!(
                        "image_diff {scene}: {} (artifacts in {})",
                        r.message,
                        dir.display()
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------- scene helpers

fn light() -> DirectionalLight {
    DirectionalLight {
        direction: glam::Vec3::new(0.0, 0.0, -1.0),
        color: [1.0; 3],
        intensity: 1.0,
        ambient: [0.1; 3],
    }
}

fn lit_pipeline(b: &mut WgpuBackend) -> PipelineId {
    b.create_pipeline(&PipelineDesc {
        label: "lit",
        shader: ShaderId::LitTextured,
        vertex_layout: VertexLayout::PosNormalUv,
    })
    .expect("lit pipeline")
}

fn morph_pipeline(b: &mut WgpuBackend) -> PipelineId {
    b.create_pipeline(&PipelineDesc {
        label: "lit-morph",
        shader: ShaderId::LitTextured,
        vertex_layout: VertexLayout::PosNormalUvMorph,
    })
    .expect("morph pipeline")
}

fn index_quad(b: &mut WgpuBackend) -> BufferId {
    b.create_buffer(
        &BufferDesc {
            label: "ib",
            usage: BufferUsage::Index,
        },
        &encode_indices(&[0, 1, 2, 0, 2, 3]),
    )
    .expect("ib")
}

/// Quad in clip space (mvp = identity) spanning x0..x1, y -0.5..0.5, at depth z.
fn quad(b: &mut WgpuBackend, x0: f32, x1: f32, z: f32) -> MeshHandle {
    let v = |x: f32, y: f32, uv: [f32; 2]| VertexPosNormalUv {
        position: [x, y, z],
        normal: [0.0, 0.0, 1.0],
        uv,
    };
    let verts = [
        v(x0, -0.5, [0.0, 1.0]),
        v(x1, -0.5, [1.0, 1.0]),
        v(x1, 0.5, [1.0, 0.0]),
        v(x0, 0.5, [0.0, 0.0]),
    ];
    let vb = b
        .create_buffer(
            &BufferDesc {
                label: "vb",
                usage: BufferUsage::Vertex,
            },
            &encode_lit_vertices(&verts),
        )
        .expect("vb");
    let ib = index_quad(b);
    MeshHandle::new(VertexLayout::PosNormalUv, vb, 4).with_indices(ib, 6)
}

fn morph_quad(b: &mut WgpuBackend, x0: f32, x1: f32, z: f32, shift: [f32; 3]) -> MeshHandle {
    let v = |x: f32, y: f32, uv: [f32; 2]| VertexPosNormalUvMorph {
        position: [x, y, z],
        normal: [0.0, 0.0, 1.0],
        uv,
        morph_position: [x + shift[0], y + shift[1], z + shift[2]],
        morph_uv: uv,
    };
    let verts = [
        v(x0, -0.5, [0.0, 1.0]),
        v(x1, -0.5, [1.0, 1.0]),
        v(x1, 0.5, [1.0, 0.0]),
        v(x0, 0.5, [0.0, 0.0]),
    ];
    let vb = b
        .create_buffer(
            &BufferDesc {
                label: "mvb",
                usage: BufferUsage::Vertex,
            },
            &encode_lit_morph_vertices(&verts),
        )
        .expect("mvb");
    let ib = index_quad(b);
    MeshHandle::new(VertexLayout::PosNormalUvMorph, vb, 4).with_indices(ib, 6)
}

/// 2x2 texture: (0,0) red, (1,0) green, (0,1) blue, (1,1) white.
fn four_texel_texture(b: &mut WgpuBackend) -> TextureId {
    let data: [u8; 16] = [
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];
    b.create_texture(
        &TextureDesc {
            label: "four",
            width: 2,
            height: 2,
        },
        &data,
    )
    .expect("texture")
}

fn draw(pipeline: PipelineId, mesh: MeshHandle, texture: Option<TextureId>) -> DrawCommand {
    DrawCommand {
        pipeline,
        mesh,
        mvp: Mat4::IDENTITY,
        model: Mat4::IDENTITY,
        normal: Mat3::IDENTITY,
        base_color: [1.0; 4],
        texture,
        morph: 0.0,
    }
}

fn packet(draws: Vec<DrawCommand>, fog: DistanceFog) -> FramePacket {
    FramePacket {
        frame_index: 0,
        light: light(),
        fog,
        passes: vec![PassPacket {
            name: "main".to_string(),
            color_target: TextureId::BACKBUFFER,
            depth_target: Some(TextureId::DEPTH),
            clear_color: [0.0, 0.0, 0.0, 1.0],
            clear_depth: 0.0,
            draws,
        }],
    }
}

fn render(b: &mut WgpuBackend, p: &FramePacket) -> Vec<u8> {
    b.submit(p).expect("submit");
    let buf = b.read_offscreen_rgba8().expect("readback");
    assert_eq!(buf.len(), W * W * 4);
    buf
}

// ---------------------------------------------------------------- scenes

fn scene_unlit_triangle(b: &mut WgpuBackend, dx: f32) -> Vec<u8> {
    let unlit = b
        .create_pipeline(&PipelineDesc {
            label: "unlit",
            shader: ShaderId::UnlitVertexColor,
            vertex_layout: VertexLayout::PosColor,
        })
        .expect("unlit pipeline");
    let tri = [
        VertexPosColor {
            position: [dx, 0.7, 0.5],
            color: [1.0, 0.0, 0.0],
        },
        VertexPosColor {
            position: [-0.7 + dx, -0.6, 0.5],
            color: [0.0, 1.0, 0.0],
        },
        VertexPosColor {
            position: [0.7 + dx, -0.6, 0.5],
            color: [0.0, 0.0, 1.0],
        },
    ];
    let vb = b
        .create_buffer(
            &BufferDesc {
                label: "tri",
                usage: BufferUsage::Vertex,
            },
            &encode_vertices(&tri),
        )
        .expect("tri");
    let mesh = MeshHandle::new(VertexLayout::PosColor, vb, 3);
    render(
        b,
        &packet(vec![draw(unlit, mesh, None)], DistanceFog::default()),
    )
}

fn scene_lit_quad(b: &mut WgpuBackend) -> Vec<u8> {
    let lit = lit_pipeline(b);
    let tex = four_texel_texture(b);
    let q = quad(b, -0.7, 0.7, 0.5);
    let mut d = draw(lit, q, Some(tex));
    d.base_color = [1.0, 0.75, 0.5, 1.0];
    render(b, &packet(vec![d], DistanceFog::default()))
}

fn scene_fog_gradient(b: &mut WgpuBackend) -> Vec<u8> {
    let lit = lit_pipeline(b);
    let mut draws = Vec::new();
    for (i, dist) in [0.0f32, 2.0, 6.0, 20.0].into_iter().enumerate() {
        let x0 = -1.0 + i as f32 * 0.5;
        let q = quad(b, x0, x0 + 0.5, 0.5);
        let mut d = draw(lit, q, None);
        d.base_color = [1.0, 0.0, 0.0, 1.0];
        d.model = Mat4::from_translation(glam::Vec3::new(0.0, 0.0, dist));
        draws.push(d);
    }
    let fog = DistanceFog {
        color: [0.5, 0.25, 0.75],
        density: 0.2,
    };
    render(b, &packet(draws, fog))
}

fn scene_morph_half(b: &mut WgpuBackend) -> Vec<u8> {
    let mp = morph_pipeline(b);
    let tex = four_texel_texture(b);
    let q = morph_quad(b, -0.5, 0.5, 0.5, [0.25, 0.1, 0.0]);
    let mut d = draw(mp, q, Some(tex));
    d.morph = 0.5;
    render(b, &packet(vec![d], DistanceFog::default()))
}

// ---------------------------------------------------------------- tests: scenes

#[test]
fn unlit_triangle() {
    if let Some(mut b) = adapter_or_skip() {
        check_scene("unlit_triangle", &scene_unlit_triangle(&mut b, 0.0));
    }
}

#[test]
fn lit_quad_directional() {
    if let Some(mut b) = adapter_or_skip() {
        check_scene("lit_quad_directional", &scene_lit_quad(&mut b));
    }
}

#[test]
fn fog_gradient() {
    if let Some(mut b) = adapter_or_skip() {
        check_scene("fog_gradient", &scene_fog_gradient(&mut b));
    }
}

#[test]
fn morph_half() {
    if let Some(mut b) = adapter_or_skip() {
        check_scene("morph_half", &scene_morph_half(&mut b));
    }
}

// ---------------------------------------------------------------- tests: harness

#[test]
fn golden_action_all_combinations() {
    use GoldenAction::{Compare, Fail, Skip, Write};
    // (exists, update, require_goldens) -> action
    let table = [
        (false, false, false, Skip),
        (false, false, true, Fail),
        (false, true, false, Write),
        (false, true, true, Write),
        (true, false, false, Compare),
        (true, false, true, Compare),
        (true, true, false, Write),
        (true, true, true, Write),
    ];
    for (exists, update, require, want) in table {
        assert_eq!(
            golden_action(exists, update, require),
            want,
            "exists={exists} update={update} require={require}"
        );
    }
}

fn flat(rgba: [u8; 4]) -> Vec<u8> {
    rgba.iter().copied().cycle().take(W * W * 4).collect()
}

#[test]
fn compare_identical_and_within_tolerance_pass() {
    let a = flat([100, 100, 100, 255]);
    assert!(compare(&a, &a, (W, W), CHANNEL_TOLERANCE, MAX_BAD_FRACTION).is_ok());
    let b = flat([102, 98, 100, 255]);
    let s = compare(&a, &b, (W, W), CHANNEL_TOLERANCE, MAX_BAD_FRACTION).expect("within tol");
    assert_eq!(s.max_delta, 2);
}

#[test]
fn negative_control_perturbed_block_fails() {
    let golden = flat([100, 100, 100, 255]);
    let mut actual = golden.clone();
    // 8x8 block (1.6% of pixels) off by 40: far over the 0.5% budget.
    for y in 0..8 {
        for x in 0..8 {
            actual[(y * W + x) * 4] = 140;
        }
    }
    let err = compare(
        &actual,
        &golden,
        (W, W),
        CHANNEL_TOLERANCE,
        MAX_BAD_FRACTION,
    )
    .expect_err("perturbed block must be detected");
    assert!(err.diff.is_some());
    // A few pixels under the budget are tolerated (the budget is real, not zero).
    let mut few = golden.clone();
    few[0] = 140;
    assert!(compare(&few, &golden, (W, W), CHANNEL_TOLERANCE, MAX_BAD_FRACTION).is_ok());
    // A delta just over the channel tolerance on every pixel fails.
    let over = flat([103, 100, 100, 255]);
    assert!(compare(&over, &golden, (W, W), CHANNEL_TOLERANCE, MAX_BAD_FRACTION).is_err());
}

#[test]
fn compare_rejects_dimension_mismatch_without_panic() {
    let a = flat([1, 2, 3, 255]);
    let short = vec![0u8; 16];
    let err = compare(&a, &short, (W, W), CHANNEL_TOLERANCE, MAX_BAD_FRACTION)
        .expect_err("size mismatch");
    assert!(err.message.contains("size mismatch"), "{}", err.message);
    assert!(compare(&short, &a, (W, W), CHANNEL_TOLERANCE, MAX_BAD_FRACTION).is_err());
}

#[test]
fn negative_control_shifted_geometry_fails_on_gpu() {
    let Some(mut b) = adapter_or_skip() else {
        return;
    };
    let base = scene_unlit_triangle(&mut b, 0.0);
    // One pixel is 2/64 of clip space.
    let shifted = scene_unlit_triangle(&mut b, 2.0 / W as f32);
    let same = scene_unlit_triangle(&mut b, 0.0);
    assert!(
        compare(&same, &base, (W, W), CHANNEL_TOLERANCE, MAX_BAD_FRACTION).is_ok(),
        "software adapter render is not deterministic"
    );
    assert!(
        compare(&shifted, &base, (W, W), CHANNEL_TOLERANCE, MAX_BAD_FRACTION).is_err(),
        "a one-pixel geometry shift went undetected"
    );
}

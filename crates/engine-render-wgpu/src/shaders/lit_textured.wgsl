struct Frame { light_dir: vec4<f32>, light_color: vec4<f32>, ambient: vec4<f32>, fog: vec4<f32> };
struct Draw { mvp: mat4x4<f32>, model: mat4x4<f32>, normal: mat3x3<f32>, base_color: vec4<f32>, params: vec4<f32> };
@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> draw: Draw;
@group(2) @binding(0) var base_tex: texture_2d<f32>;
@group(2) @binding(1) var base_smp: sampler;
struct VsIn { @location(0) pos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) uv: vec2<f32> };
struct VsMorphIn { @location(0) pos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) uv: vec2<f32>, @location(3) mpos: vec3<f32>, @location(4) muv: vec2<f32> };
struct VsOut { @builtin(position) clip: vec4<f32>, @location(0) nrm: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) rel: vec3<f32> };
@vertex fn vs_main(i: VsIn) -> VsOut {
  var o: VsOut;
  o.clip = draw.mvp * vec4<f32>(i.pos, 1.0);
  o.nrm = draw.normal * i.nrm;
  o.uv = i.uv;
  o.rel = (draw.model * vec4<f32>(i.pos, 1.0)).xyz;
  return o;
}
// Blend toward the second position and uv by draw.params.x. The normal is not blended: it
// is the one carried by the vertex (the morph attributes hold no second normal).
@vertex fn vs_morph(i: VsMorphIn) -> VsOut {
  var o: VsOut;
  let t = draw.params.x;
  let p = mix(i.pos, i.mpos, t);
  o.clip = draw.mvp * vec4<f32>(p, 1.0);
  o.nrm = draw.normal * i.nrm;
  o.uv = mix(i.uv, i.muv, t);
  o.rel = (draw.model * vec4<f32>(p, 1.0)).xyz;
  return o;
}
@fragment fn fs_main(i: VsOut) -> @location(0) vec4<f32> {
  let n = normalize(i.nrm);
  let ndl = max(dot(n, -frame.light_dir.xyz), 0.0);
  let albedo = textureSample(base_tex, base_smp, i.uv) * draw.base_color;
  var color = vec4<f32>(albedo.rgb * (frame.ambient.xyz + frame.light_color.xyz * ndl), albedo.a);
  if (frame.fog.w > 0.0) {
    let d2 = dot(i.rel, i.rel);
    let k = frame.fog.w * frame.fog.w;
    let f = (d2 * k) / (1.0 + d2 * k);
    color = vec4<f32>(mix(color.rgb, frame.fog.rgb, f), color.a);
  }
  return color;
}

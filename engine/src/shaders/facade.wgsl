#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
}

// x: floor height, y: window column width, in world units.
@group(2) @binding(100) var<uniform> grid: vec4<f32>;

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.547);
}

// Soft-edged 1 inside [lo, hi], antialiased by the pixel footprint w.
fn band(x: f32, lo: f32, hi: f32, w: f32) -> f32 {
    return smoothstep(lo - w, lo + w, x) - smoothstep(hi - w, hi + w, x);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

    let n = in.world_normal;
    let wall = 1.0 - step(0.5, abs(n.y));
    // World coordinate along the wall; uv runs linearly with it, so their
    // derivative ratio is the wall's size without any per-building data.
    let along = select(in.world_position.x, in.world_position.z, abs(n.x) > abs(n.z));
    let height = length(vec2(dpdx(in.world_position.y), dpdy(in.world_position.y)))
        / max(length(vec2(dpdx(in.uv.y), dpdy(in.uv.y))), 1e-6);
    let width = length(vec2(dpdx(along), dpdy(along)))
        / max(length(vec2(dpdx(in.uv.x), dpdy(in.uv.x))), 1e-6);

    // Whole floors and columns per wall, so no window is cut at an edge.
    let floors = max(1.0, round(height / grid.x));
    let cols = max(1.0, round(width / grid.y));
    let cell = in.uv * vec2(cols, floors);
    let id = floor(cell);
    let f = fract(cell);
    let w = fwidth(cell);

    let ground = 1.0 - step(0.5, id.y);
    let x_lo = mix(0.2, 0.08, ground);
    let y_lo = mix(0.24, 0.06, ground);
    let y_hi = mix(0.8, 0.86, ground);
    let mask = band(f.x, x_lo, 1.0 - x_lo, w.x) * band(f.y, y_lo, y_hi, w.y);

    // Past a few pixels per cell the grid only aliases; fade to its average.
    let detail = 1.0 - smoothstep(0.2, 0.45, max(w.x, w.y));
    let glass = mix(0.42, mask, detail) * wall;

    let base = pbr_input.material.base_color.rgb;
    // Constant over a wall, so each wall gets its own scatter of lit rooms.
    let plane = dot(in.world_position.xz, n.xz);
    let tint = hash(id + vec2(plane * 1.7, plane * 0.31)) * detail;
    let pane = base * 0.22 + vec3(0.05, 0.07, 0.10) + vec3(0.10, 0.08, 0.05) * tint * tint;
    let facade = select(base * 0.82, base, wall > 0.5);
    pbr_input.material.base_color = vec4(mix(facade, pane, glass), pbr_input.material.base_color.a);
    pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.12, glass);
    pbr_input.material.reflectance = mix(pbr_input.material.reflectance, 0.9, glass);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}

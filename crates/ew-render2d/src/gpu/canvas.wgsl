// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

// The canvas shader of ew-render2d's GPU path (WP-S2.02 section 7.1): one instanced pipeline for the meshes of
// paths, glyph outlines and LOD bars.
//
// Colours are sRGB bytes as authored and are written to an Rgba8Unorm target as they are, so blending happens in
// gamma space, as in the CPU renderer's u8 pipeline, and egui-wgpu samples the texture as the gamma-space texture
// it expects.

struct View {
    // Where page micrometre (0, 0) lands, in physical pixels of the target.
    origin_px: vec2<f32>,
    // Physical pixels per page micrometre.
    px_per_um: f32,
    // The number of paint indices of the scene (the depth range).
    depth_count: f32,
    // The target's size in physical pixels.
    target_px: vec2<f32>,
    pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> view: View;

struct VertexIn {
    // Per vertex: the point on the path, the stroke normal and half-width (zero for fills), the colour and the
    // paint index within the mesh.
    @location(0) pos: vec2<f32>,
    @location(1) normal: vec2<f32>,
    @location(2) half_width: f32,
    @location(3) color: vec4<f32>,
    @location(4) index: u32,
    // Per instance: mesh coordinates to page micrometres (a, b, c, d and e, f), the clip rectangle in page
    // micrometres, the tint and the paint index of the mesh's first primitive.
    @location(5) m: vec4<f32>,
    @location(6) t: vec2<f32>,
    @location(7) clip: vec4<f32>,
    @location(8) tint: vec4<f32>,
    @location(9) depth: u32,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) page_um: vec2<f32>,
    @location(2) @interpolate(flat) clip: vec4<f32>,
};

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    // The instance's scale (its transforms are similarities: a turn or mirror and a uniform scale).
    let scale = sqrt(abs(in.m.x * in.m.w - in.m.y * in.m.z));
    // Every stroke is at least one physical pixel wide: half a pixel on each side of the path.
    var half_width = in.half_width;
    if half_width > 0.0 {
        let min_half = 0.5 / max(scale * view.px_per_um, 1e-20);
        half_width = max(half_width, min_half);
    }
    let local = in.pos + in.normal * half_width;
    let page = vec2<f32>(
        in.m.x * local.x + in.m.z * local.y + in.t.x,
        in.m.y * local.x + in.m.w * local.y + in.t.y,
    );
    let px = view.origin_px + page * view.px_per_um;
    let ndc = vec2<f32>(px.x / view.target_px.x * 2.0 - 1.0, 1.0 - px.y / view.target_px.y * 2.0);
    // Later in paint order is nearer: a smaller depth passes the Less test.
    let paint = f32(in.depth + in.index);
    let z = 1.0 - (paint + 1.0) / (view.depth_count + 2.0);
    var out: VertexOut;
    out.position = vec4<f32>(ndc, z, 1.0);
    out.color = in.color * in.tint;
    out.page_um = page;
    out.clip = in.clip;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    if in.page_um.x < in.clip.x || in.page_um.y < in.clip.y || in.page_um.x > in.clip.z || in.page_um.y > in.clip.w {
        discard;
    }
    return in.color;
}

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    var out: VertexOutput;
    // Full-screen triangle: vertices at (-1,-1), (3,-1), (-1,3)
    let pos = vec2f(
        f32(i32(idx) / 2) * 4.0 - 1.0,
        f32(i32(idx) % 2) * 4.0 - 1.0
    );
    out.position = vec4f(pos, 0.0, 1.0);
    out.uv = vec2f(
        (pos.x + 1.0) / 2.0,
        1.0 - (pos.y + 1.0) / 2.0  // Flip Y for NES coordinates
    );
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return textureSample(tex, samp, in.uv);
}

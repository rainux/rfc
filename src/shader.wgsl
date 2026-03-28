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

struct Globals {
    window_size: vec2f,
};

@group(0) @binding(2) var<uniform> globals: Globals;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let window_aspect = globals.window_size.x / globals.window_size.y;
    let nes_aspect = 256.0 / 240.0;

    var uv = in.uv;

    if window_aspect > nes_aspect {
        // Window is wider — pillarbox (black bars on left/right)
        let scale = nes_aspect / window_aspect;
        uv.x = (uv.x - 0.5) / scale + 0.5;
    } else {
        // Window is taller — letterbox (black bars on top/bottom)
        let scale = window_aspect / nes_aspect;
        uv.y = (uv.y - 0.5) / scale + 0.5;
    }

    // If UV is outside [0,1], render black
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
        return vec4f(0.0, 0.0, 0.0, 1.0);
    }

    return textureSample(tex, samp, uv);
}

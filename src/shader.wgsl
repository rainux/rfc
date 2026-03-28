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
    shader_mode: u32,
    _padding: u32,
};

@group(0) @binding(2) var<uniform> globals: Globals;

fn crt_effect(uv: vec2f) -> vec4f {
    let color = textureSample(tex, samp, uv);

    // Scanline effect: darken every other pixel row (based on screen position)
    let screen_y = uv.y * globals.window_size.y;
    let scanline = 0.75 + 0.25 * sin(screen_y * 3.14159 * 2.0);

    // Slight vignette (darker at edges)
    let center = uv - vec2f(0.5, 0.5);
    let vignette = 1.0 - dot(center, center) * 0.5;

    return vec4f(color.rgb * scanline * vignette, 1.0);
}

fn scanline_effect(uv: vec2f) -> vec4f {
    let color = textureSample(tex, samp, uv);

    // Map to NES pixel row (0-239)
    let nes_y = uv.y * 240.0;
    // Scale factor: how many screen pixels per NES pixel
    let scale = globals.window_size.y / 240.0;
    // Position within the scaled pixel (0.0-1.0)
    let frac = fract(nes_y);
    // Darken the bottom portion of each scanline gap
    let brightness = select(1.0, 0.5, frac > (1.0 - 1.0 / scale));

    return vec4f(color.rgb * brightness, 1.0);
}

fn smooth_effect(uv: vec2f) -> vec4f {
    // Bilinear interpolation by sampling at sub-pixel offsets
    let tex_size = vec2f(256.0, 240.0);
    let pixel = uv * tex_size;
    let frac_part = fract(pixel);
    let base = (floor(pixel) + 0.5) / tex_size;
    let step = 1.0 / tex_size;

    let tl = textureSample(tex, samp, base);
    let tr = textureSample(tex, samp, base + vec2f(step.x, 0.0));
    let bl = textureSample(tex, samp, base + vec2f(0.0, step.y));
    let br = textureSample(tex, samp, base + step);

    let top = mix(tl, tr, frac_part.x);
    let bottom = mix(bl, br, frac_part.x);
    return mix(top, bottom, frac_part.y);
}

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

    switch globals.shader_mode {
        case 1u: { return crt_effect(uv); }
        case 2u: { return smooth_effect(uv); }
        case 3u: { return scanline_effect(uv); }
        default: { return textureSample(tex, samp, uv); }
    }
}

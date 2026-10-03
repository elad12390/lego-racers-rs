#import bevy_pbr::forward_io::VertexOutput
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var original_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var original_sampler: sampler;

// Original geometry carries display-encoded colors, not PBR albedo. The image
// is sampled without automatic sRGB decoding, preserving original multiplication.
fn display_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(c / 12.92, pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c > vec3<f32>(0.04045));
}
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(original_texture, original_sampler, in.uv) * in.color;
    if color.a < 0.001 { discard; }
    return vec4<f32>(display_to_linear(color.rgb), color.a);
}

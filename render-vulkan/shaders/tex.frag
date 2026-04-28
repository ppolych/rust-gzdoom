#version 450

layout(location = 0) in vec2 in_uv;
layout(location = 1) in vec4 in_color;
layout(location = 2) in vec3 in_world_pos;
layout(location = 3) in vec3 in_normal;

layout(binding = 0) uniform sampler2D tex;

#define MAX_LIGHTS 16

layout(std140, binding = 1) uniform LightUniform {
    vec4 config;
    vec4 light_position_radius[MAX_LIGHTS];
    vec4 light_color_intensity[MAX_LIGHTS];
} lights;

layout(location = 0) out vec4 out_color;

const int DEBUG_LIT = 0;
const int DEBUG_SOLID = 1;
const int DEBUG_NORMALS = 2;
const int DEBUG_UV = 3;
const int DEBUG_LIGHT_ONLY = 4;
const int DEBUG_TEXTURE_ONLY = 5;

vec4 sample_texture_color(vec2 uv) {
    return texture(tex, uv);
}

vec3 compute_lighting(vec3 normal) {
    vec3 lighting = vec3(1.0);
    if (lights.config.z > 0.5) {
        lighting = vec3(max(lights.config.y, 0.0));
        int light_count = min(int(lights.config.x + 0.5), MAX_LIGHTS);
        for (int i = 0; i < light_count; i++) {
            vec3 light_pos = lights.light_position_radius[i].xyz;
            float radius = max(lights.light_position_radius[i].w, 0.001);
            vec3 to_light = light_pos - in_world_pos;
            float dist = length(to_light);
            if (dist < radius) {
                vec3 light_dir = to_light / max(dist, 0.001);
                float ndotl = max(dot(normal, light_dir), 0.0);
                float attenuation = max(1.0 - (dist / radius), 0.0);
                vec3 light_color = lights.light_color_intensity[i].rgb;
                float intensity = lights.light_color_intensity[i].a;
                lighting += light_color * intensity * attenuation * ndotl;
            }
        }
    }
    return lighting;
}

vec3 uv_debug_color(vec2 uv) {
    vec2 tiled = fract(uv);
    vec2 grid = step(vec2(0.96), tiled);
    float line = max(grid.x, grid.y);
    vec3 gradient = vec3(tiled.x, tiled.y, 1.0 - tiled.x);
    return mix(gradient, vec3(1.0), line);
}

void main() {
    vec3 normal = normalize(in_normal);
    vec3 lighting = compute_lighting(normal);
    int debug_mode = int(lights.config.w + 0.5);

    if (debug_mode == DEBUG_SOLID) {
        out_color = vec4(in_color.rgb, 1.0);
    } else if (debug_mode == DEBUG_NORMALS) {
        out_color = vec4(normal * 0.5 + 0.5, 1.0);
    } else if (debug_mode == DEBUG_UV) {
        out_color = vec4(uv_debug_color(in_uv), 1.0);
    } else if (debug_mode == DEBUG_LIGHT_ONLY) {
        out_color = vec4(clamp(lighting, 0.0, 1.0), 1.0);
    } else if (debug_mode == DEBUG_TEXTURE_ONLY) {
        vec4 tex_color = sample_texture_color(in_uv);
        if (tex_color.a < 0.1) {
            discard;
        }
        out_color = vec4(tex_color.rgb, tex_color.a);
    } else {
        vec4 tex_color = sample_texture_color(in_uv);
        if (tex_color.a < 0.1) {
            discard;
        }
        out_color = vec4(
            clamp(tex_color.rgb * in_color.rgb * lighting, 0.0, 1.0),
            tex_color.a * in_color.a
        );
    }
    if (out_color.a < 0.1) {
        discard;
    }
}

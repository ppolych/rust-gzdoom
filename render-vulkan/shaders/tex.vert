#version 450

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_color;
layout(location = 3) in vec3 in_world_pos;
layout(location = 4) in vec3 in_normal;

layout(location = 0) out vec2 out_uv;
layout(location = 1) out vec4 out_color;
layout(location = 2) out vec3 out_world_pos;
layout(location = 3) out vec3 out_normal;

void main() {
    gl_Position = vec4(in_pos, 1.0);
    out_uv = in_uv;
    out_color = in_color;
    out_world_pos = in_world_pos;
    out_normal = in_normal;
}

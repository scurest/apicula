#version 140

uniform sampler2D tex;

in vec2 v_texcoord;
in vec3 v_color;

out vec4 color;

void main() {
    vec4 sample = texture(tex, v_texcoord);
    if (sample.w == 0.0) discard;
    color = sample * vec4(v_color, 1.0);
}

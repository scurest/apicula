#version 140

uniform sampler2D tex;

in vec2 v_texcoord;
in vec4 v_color;

out vec4 color;

void main() {
    vec4 c = texture(tex, v_texcoord) * v_color;
    if (c.w == 0.0) discard;
    color = c;
}

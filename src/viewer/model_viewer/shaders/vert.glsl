#version 140

uniform mat4 matrix;
uniform float alpha;

in vec3 position;
in vec2 texcoord;
in vec3 color;

out vec2 v_texcoord;
out vec4 v_color;

void main() {
    v_texcoord = texcoord;
    v_color = vec4(color, alpha);
    gl_Position = matrix * vec4(position, 1.0);
}

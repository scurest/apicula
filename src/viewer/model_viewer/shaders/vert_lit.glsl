#version 140

uniform mat4 matrix;
uniform vec3 light_vec;
uniform vec3 light_color;

uniform float alpha;
uniform vec3 diffuse_color;
uniform vec3 emission_color;
uniform vec3 ambient_color;

in vec3 position;
in vec2 texcoord;
in vec3 normal;

out vec2 v_texcoord;
out vec4 v_color;

void main() {
    v_texcoord = texcoord;

    vec3 c;
    float diff_level = max(0.0, -dot(light_vec, normal));
    c = emission_color;
    c += diffuse_color * light_color * diff_level;
    //c += specular_color * light_color * shine_level;
    c += ambient_color * light_color;
    v_color = vec4(c, alpha);

    gl_Position = matrix * vec4(position, 1.0);
}

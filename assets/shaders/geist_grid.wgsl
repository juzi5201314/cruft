struct GeistGridMaterial {
    color: vec4<f32>,
    grid_color: vec4<f32>,
    spacing: f32,
    thickness: f32,
}

@group(1) @binding(0) var<uniform> material: GeistGridMaterial;

@fragment
fn fragment(
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    // We assume uv is [0, 1]. To keep grid square, we need the aspect ratio or absolute size.
    // However, for a subtle aesthetic, a fixed UV grid often works well too.
    // If we want it to be 40px, we should pass the Node size to the shader.
    // For this prototype, we'll just use UV-based spacing.
    
    let p = uv * (1.0 / material.spacing); // spacing is a value like 0.05
    let grid = fract(p + material.thickness * 0.5);
    let line_x = step(1.0 - material.thickness, grid.x);
    let line_y = step(1.0 - material.thickness, grid.y);
    let line = max(line_x, line_y);
    
    return mix(material.color, material.grid_color, line);
}

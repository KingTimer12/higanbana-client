//! Exemplo mínimo de shader do usuário: gradiente animado como fundo de uma `div`.
use super::Showcase;
use higanbana_gpui::prelude::*;

/// Paleta de cossenos que anda com o tempo; `param(0).x` é a velocidade.
const GRADIENT: &str = r#"
fn shade(p: ShadeInput) -> vec4<f32> {
    let t = p.time * param(0u).x;
    let rgb = 0.5 + 0.5 * cos(t + vec3<f32>(p.uv.x, p.uv.y, p.uv.x) * 3.0 + vec3<f32>(0.0, 2.0, 4.0));
    return vec4<f32>(rgb, 1.0);
}
"#;

/// Cria o shader uma vez (guardar o `Shader` evita traduzir e compilar a cada quadro).
pub fn gradient() -> Option<Shader> {
    Shader::wgsl(GRADIENT).map(|s| s.animated(true)).map_err(|e| eprintln!("shader: {e}")).ok()
}

/// Card com o gradiente de fundo e texto por cima; sem shader, nada.
pub fn card(shader: Option<&Shader>, theme: &Theme) -> Option<Element<Showcase>> {
    let s = shader?;
    Some(
        div()
            .w(300.0)
            .h(120.0)
            .p(16.0)
            .rounded(theme.radius)
            .shader(s, params![[1.0, 0.0, 0.0, 0.0]])
            .items_end()
            .child(text("Shader WGSL").text_size(18.0).font_bold().text_color(Color::WHITE)),
    )
}

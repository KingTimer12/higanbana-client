//! Componente `liquid_glass`: placa de vidro feita só com a API pública de shaders
//! (`Shader::wgsl`, `.backdrop(true)`, `.animated(true)`, `div().shader(...)`, `params!`).
//! O WGSL está em `liquid_glass.wgsl`.
use higanbana_gpui::prelude::*;

/// Aparência do vidro. Medidas em pixels lógicos.
#[derive(Debug, Clone, Copy)]
pub struct GlassParams {
    /// Quanto a borda curva desloca o que está atrás.
    pub refraction: f32,
    /// Raio do desfoque do fundo (o prelúdio limita a 64 px físicos).
    pub blur: f32,
    /// Largura da faixa curva junto da borda.
    pub bevel: f32,
    /// Cor da tinta; `a` é a força (0 = incolor).
    pub tint: Color,
    /// Intensidade do reflexo e do filete da borda (0..1).
    pub shine: f32,
    /// Raio dos cantos.
    pub radius: f32,
}

impl Default for GlassParams {
    fn default() -> Self {
        GlassParams { refraction: 32.0, blur: 8.0, bevel: 24.0, tint: Color::WHITE.alpha(0.08), shine: 1.0, radius: 24.0 }
    }
}

thread_local! {
    /// Traduzido uma vez por thread: o cache de GPU é por `Shader`, recriar a cada quadro recompilaria.
    static GLASS: Option<Shader> = match Shader::wgsl(include_str!("liquid_glass.wgsl")) {
        Ok(s) => Some(s.backdrop(true).animated(true)),
        Err(e) => {
            eprintln!("liquid_glass: {e}");
            None
        }
    };
}

/// Placa de vidro; o chamador define tamanho, posição e filhos.
/// Sem a feature `shaders` vira um painel translúcido comum.
pub fn liquid_glass<V: View>(p: GlassParams) -> Element<V> {
    let base = div().rounded(p.radius).shadow(Color::BLACK.alpha(0.35), 24.0);
    match GLASS.with(|s| s.clone()) {
        Some(s) => base.shader(&s, params![[p.refraction, p.blur, p.bevel, p.shine], p.tint]),
        None => base.bg(p.tint.alpha(0.35)).border(1.0, Color::WHITE.alpha(0.3)),
    }
}

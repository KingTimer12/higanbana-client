//! Tema do higanbana: as cores ficam em `themes/higanbana.json` e as telas leem
//! tudo de `cx.theme()` pelo papel da cor.
//!
//! Aqui ficam só os poucos papéis que o tema não tem:
//! - a tinta da marca para texto (vermelho claro legível no fundo escuro);
//! - as cores sobre mídia (vídeo, capas e banners), que não seguem o tema
//!   porque o fundo é a própria imagem.

use higanbana_gpui::{Color, Theme};

const THEME: &str = include_str!("../themes/higanbana.json");

/// Tema escuro do higanbana; cai no tema escuro da lib se o arquivo não ler.
pub fn load() -> Theme {
    Theme::from_json(THEME).unwrap_or_else(|e| {
        tracing::error!(erro = %e, "tema do higanbana inválido; usando o padrão");
        Theme::dark()
    })
}

/// Texto na cor da marca (rótulos ativos, número da faixa tocando): `base.red.light`.
pub fn primary_ink(t: &Theme) -> Color {
    t.base.red_light
}

/// Texto e ícones sobre vídeo, capa ou banner: sempre claros.
pub fn on_media() -> Color {
    Color::WHITE
}

/// Texto secundário sobre mídia.
pub fn on_media_muted() -> Color {
    Color::WHITE.alpha(0.72)
}

/// Véu escuro sobre mídia (`alpha` 0..1).
pub fn media_scrim(alpha: f32) -> Color {
    Color::BLACK.alpha(alpha)
}

/// Trilho claro sobre mídia (barra de progresso no vídeo).
pub fn media_track(alpha: f32) -> Color {
    Color::WHITE.alpha(alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tema_do_json_carrega() {
        let t = load();
        let dark = Theme::dark();
        assert_ne!(t.primary, dark.primary, "primary vem do arquivo");
        assert_eq!(t.background, Theme::from_json(THEME).unwrap().background);
        assert_ne!(primary_ink(&t), Color::TRANSPARENT);
    }
}

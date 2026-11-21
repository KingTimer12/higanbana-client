//! Imagens remotas das telas: pedidas só quando vão aparecer (`on_visible`),
//! com fade curto ao chegar. O cache com teto de memória é o de `state/images.rs`.

use higanbana_gpui::prelude::*;
use higanbana_gpui::ObjectFit;

use crate::motion::{transition, QUICK};
use crate::root::Higanbana;
use crate::state::Img;

/// Folga (px) para pedir a imagem antes de ela entrar na tela.
pub const PREFETCH: f32 = 300.0;

/// O que desenhar para uma imagem conforme o estado no cache.
#[derive(Clone)]
pub enum Look {
    /// Ainda não pedida: pedir quando aparecer.
    Request,
    /// Baixando/decodificando: fica o fundo do pai.
    Wait,
    Show(ImageSource),
    /// Falhou: fundo neutro (nunca esqueleto eterno).
    Neutral,
}

pub fn look(cached: Option<Img>) -> Look {
    match cached {
        None => Look::Request,
        Some(Img::Loading) => Look::Wait,
        Some(Img::Ready(src)) => Look::Show(src),
        Some(Img::Failed) => Look::Neutral,
    }
}

/// Cor dominante da capa no AniList (`#rrggbb`). É dado da obra, não do tema.
pub fn hex_color(c: Option<&str>) -> Option<Color> {
    let c = c?.trim_start_matches('#');
    (c.len() == 6).then(|| u32::from_str_radix(c, 16).ok()).flatten().map(rgb)
}

/// Imagem (`cover`, cantos `radius`) ocupando o pai inteiro; vazia até chegar.
pub fn remote_image(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, url: Option<&str>, radius: f32) -> Element<Higanbana> {
    let Some(url) = url else { return div().size_full() };
    match look(v.state.cached_image(url)) {
        Look::Show(src) => {
            let t = cx.animate_from(("img-in", url), 0.0, 1.0, transition(QUICK));
            img(src).size_full().object_fit(ObjectFit::Cover).rounded(radius).opacity(t)
        }
        Look::Wait | Look::Neutral => div().size_full(),
        Look::Request => {
            let url = url.to_owned();
            div().size_full().relative().child(on_visible(PREFETCH, move |v: &mut Higanbana, cx| {
                v.state.image(&url);
                cx.notify();
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imagem_falha_vira_neutro() {
        assert!(matches!(look(Some(Img::Failed)), Look::Neutral));
    }

    #[test]
    fn imagem_carregando_espera() {
        assert!(matches!(look(Some(Img::Loading)), Look::Wait));
        assert!(matches!(look(None), Look::Request));
    }

    #[test]
    fn cor_hex_da_capa() {
        assert_eq!(hex_color(Some("#ff0000")), Some(rgb(0xff0000)));
        assert_eq!(hex_color(Some("#fff")), None);
        assert_eq!(hex_color(None), None);
    }
}

//! Uma página: imagem pronta, esqueleto enquanto baixa, aviso com "Tentar de novo" se falhou.

use higanbana_core::reader::logic::Unit;
use higanbana_gpui::components::{Button, Component, ComponentSize, Skeleton, Variant};
use higanbana_gpui::prelude::*;
use higanbana_gpui::ObjectFit;

use crate::reader::state::page_key;
use crate::root::Higanbana;
use crate::state::Img;

/// Largura do esqueleto/aviso quando a página vai na altura da área (25 rem).
pub const PLACEHOLDER_W: f32 = 400.0;

/// Caixa da página: `h = Some(..)` encaixa a página em `w`×`h` (modos de página, wide strip);
/// `None` usa a largura `w` e a altura vem da proporção (long strip).
pub fn page_image(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, u: &Unit, pg: usize, w: f32, h: Option<f32>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let Some(r) = v.state.reader.as_ref() else { return div() };
    let key = page_key(u, pg);
    let ratio = r.ratios.get(&key).copied().unwrap_or(0.7);
    match r.pages.get(&key) {
        Some(Img::Ready(image)) => {
            img(image.clone()).object_fit(ObjectFit::Contain).w(w).h(h.unwrap_or(w / ratio))
        }
        Some(Img::Failed) => {
            let k = key.clone();
            let retry = Button::new(("tentar-pagina", key.as_str()), "Tentar de novo").variant(Variant::Outline).size(ComponentSize::Sm).on_click(move |v: &mut Higanbana, _, cx| {
                cx.stop_propagation();
                if let Some(r) = v.state.reader.as_mut() {
                    r.retry_page(&k);
                }
                v.state.pump_reader();
                cx.notify();
            });
            let d = div().min_h(240.0).flex_col().items_center().justify_center().gap(12.0).p(24.0).bg(t.muted).text_size(t.typography.sm).child(text("Esta página não carregou.")).child(retry.render(cx));
            match h {
                Some(h) => d.w(PLACEHOLDER_W.min(w)).h(h),
                None => d.w(w),
            }
        }
        _ => {
            let s = Skeleton::new(("pagina-esqueleto", key.as_str()));
            match h {
                Some(h) => s.w(PLACEHOLDER_W.min(w)).h(h).render(cx),
                None => s.w(w).h(w / ratio).render(cx),
            }
        }
    }
}

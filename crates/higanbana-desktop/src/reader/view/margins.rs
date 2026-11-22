//! Margens do leitor: lombada à esquerda (voltar, título vertical, capítulo) e,
//! à direita, número da página, opções e direção.

use higanbana_core::reader::logic::{chapter_label, Direction, ProgressStyle, ReadMode};
use higanbana_gpui::components::{Button, Component, Variant};
use higanbana_gpui::prelude::*;

use super::{close, panel_toggle, toggle_direction};
use crate::chrome::tip;
use crate::reader::state::Panel;
use crate::root::Higanbana;

/// Largura das margens (5,5 rem).
pub const MARGIN: f32 = 88.0;

fn icon_button(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, id: &'static str, name: &'static str, hint: &'static str, on: bool, f: impl Fn(&mut Higanbana, &mut EventCx) + 'static) -> Element<Higanbana> {
    let accent = cx.theme().accent;
    let b = Button::new(id, "").variant(Variant::Outline).icon(icon(name).size(16.0)).on_click(move |v: &mut Higanbana, _, cx| {
        cx.stop_propagation();
        f(v, cx);
        cx.notify();
    });
    let b = b.render(cx);
    let b = if on { b.bg(accent) } else { b };
    tip(v, cx, ("dica-leitor", id), b, hint)
}

/// Bloco clicável com realce no hover.
fn hot(cx: &mut RenderCx<'_, Higanbana>, id: &'static str) -> Element<Higanbana> {
    let (hover, radius) = (cx.theme().list_hover, cx.theme().radius);
    div().id(id).flex_col().items_center().rounded(radius).cursor_pointer().hover(move |s| s.bg(hover))
}

pub fn spine(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let Some(r) = v.state.reader.as_ref() else { return div() };
    let chapter = r.chapter();
    let label = chapter_label(&r.chapters.get(chapter).map(|u| u.name.clone()).unwrap_or_default());
    let num = label.number.clone().unwrap_or_else(|| (chapter + 1).to_string());
    let kind = if label.kind.is_empty() { "ARQ." } else { label.kind };
    let index_open = r.panel == Some(Panel::Index);
    let mut title = div().flex_1().min_h(0.0).overflow_hidden().flex_col().items_center().text_size(t.typography.lg).font_weight(700);
    for c in r.target.title.chars().take(28) {
        title = title.child(text(c.to_string()));
    }
    let back = icon_button(v, cx, "leitor-voltar", "chevron-left", "Voltar (Esc)", false, close);
    let ink = if index_open { t.primary } else { crate::theme::primary_ink(&t) };
    let number = hot(cx, "leitor-capitulo")
        .px(6.0)
        .py(4.0)
        .child(div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text(kind.to_uppercase())))
        .child(text(num.clone()).text_size(if num.len() > 3 { 30.0 } else { 44.0 }).font_weight(900).text_color(ink))
        .on_click(|v: &mut Higanbana, _, cx| panel_toggle(v, Panel::Index, cx));
    let col = div().w(MARGIN - 1.0).h_full().flex_col().items_center().gap(20.0).pt(20.0).pb(44.0).bg(t.group_box).child(back).child(title).child(number);
    div().h_full().flex_none().child(col).child(div().w(1.0).h_full().bg(t.border))
}

/// Texto do número: "07" ou "06–07" na página dupla; "—" sem páginas.
pub fn page_text(pages: &[usize]) -> String {
    if pages.is_empty() {
        return "—".into();
    }
    pages.iter().map(|p| format!("{:02}", p + 1)).collect::<Vec<_>>().join("–")
}

pub fn right(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let Some(r) = v.state.reader.as_ref() else { return div() };
    let total = r.total();
    let shown = if total == 0 {
        Vec::new()
    } else if r.prefs.mode == ReadMode::Double {
        r.spread()
    } else {
        vec![r.page()]
    };
    let label = page_text(&shown);
    let show_number = r.settings.progress != ProgressStyle::Hidden || r.stirred;
    let rtl = r.prefs.direction == Direction::Rtl;
    let options_open = r.panel == Some(Panel::Options);
    let of = if total > 0 { total.to_string() } else { "—".into() };
    let number = hot(cx, "leitor-pagina")
        .px(4.0)
        .opacity(if show_number { 1.0 } else { 0.0 })
        .child(text(label.clone()).text_size(if label.chars().count() > 3 { 22.0 } else { 40.0 }).font_weight(900))
        .child(div().mt(4.0).text_size(t.typography.xs).text_color(t.muted_foreground).child(text(format!("de {of}"))))
        .on_click(|v: &mut Higanbana, _, cx| panel_toggle(v, Panel::Pages, cx));
    let options = icon_button(v, cx, "leitor-opcoes", "sliders-horizontal", "Opções de leitura", options_open, |v, cx| panel_toggle(v, Panel::Options, cx));
    let (dir_icon, dir_hint) = if rtl { ("arrow-left", "Lendo da direita para a esquerda (D)") } else { ("arrow-right", "Lendo da esquerda para a direita (D)") };
    let direction = icon_button(v, cx, "leitor-direcao", dir_icon, dir_hint, false, |v, _| toggle_direction(v));
    let col = div().w(MARGIN - 1.0).h_full().flex_col().items_center().gap(14.0).pt(20.0).pb(44.0).bg(t.group_box).child(number).child(options).child(div().flex_1()).child(direction);
    div().h_full().flex_none().child(div().w(1.0).h_full().bg(t.border)).child(col)
}

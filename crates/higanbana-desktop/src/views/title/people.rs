//! Abas Personagens e Equipe: foto (ou iniciais), nome e papel, em colunas.

use higanbana_core::anilist::{text as anitext, types::{MediaDetail, Person}};
use higanbana_gpui::components::{Avatar, Component};
use higanbana_gpui::prelude::*;

use crate::root::Higanbana;
use crate::ui::cover::columns;
use crate::ui::image::remote_image;
use crate::ui::truncate;

fn is_placeholder(url: Option<&str>) -> bool {
    url.is_none_or(|u| u.ends_with("/default.jpg") || u.ends_with("/default.png"))
}

/// Foto de pessoa/personagem (48 px, redonda); iniciais quando não há imagem.
fn portrait(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, url: Option<&str>, name: Option<&str>) -> Element<Higanbana> {
    if is_placeholder(url) {
        return Avatar::new(name.unwrap_or("?").to_owned()).render(cx);
    }
    let muted = cx.theme().muted;
    div().w(48.0).h(48.0).flex_shrink_0().rounded_full().overflow_hidden().bg(muted).child(remote_image(v, cx, url, 9999.0))
}

/// Grade de `cols` colunas com espaço 32 entre elas.
fn grid(width: f32, min: f32, cells: Vec<Element<Higanbana>>) -> Element<Higanbana> {
    let cols = columns(width, min, 32.0);
    let cell_w = (width - 32.0 * (cols as f32 - 1.0)) / cols as f32;
    let mut g = div().flex_wrap().gap_x(32.0).items_start();
    for c in cells {
        g = g.child(div().w(cell_w).child(c));
    }
    g
}

fn row(t: &Theme) -> Element<Higanbana> {
    div().relative().items_center().gap(12.0).py(12.0).child(div().absolute().left(0.0).right(0.0).bottom(0.0).h(1.0).bg(t.border))
}

pub fn cast(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, d: &MediaDetail, width: f32) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let mut cells = Vec::with_capacity(d.characters.edges.len());
    for e in &d.characters.edges {
        let photo = portrait(v, cx, e.node.image.as_ref().and_then(|i| i.large.as_deref()), e.node.name.full.as_deref());
        let who = div().flex_col().flex_1().min_w(0.0).child(truncate(e.node.name.full.clone().unwrap_or_default()).text_size(t.typography.sm).font_weight(600)).child(div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text(anitext::character_role(Some(&e.role)))));
        let mut r = row(&t).child(photo).child(who);
        if let Some(va) = e.voice_actors.first() {
            let names = div().flex_col().flex_1().min_w(0.0).items_end().child(truncate(va.name.full.clone().unwrap_or_default()).text_size(t.typography.sm)).child(div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text(va.name.native.clone().unwrap_or_default())));
            let photo = portrait(v, cx, va.image.as_ref().and_then(|i| i.large.as_deref()), va.name.full.as_deref());
            r = r.child(div().flex_1().min_w(0.0).gap(12.0).items_center().child(names).child(photo));
        }
        cells.push(r);
    }
    grid(width, 320.0, cells)
}

pub fn staff(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, d: &MediaDetail, width: f32) -> Element<Higanbana> {
    let t = cx.theme().clone();
    // Agrupa papéis da mesma pessoa: "Direção, Criação original".
    let mut people: Vec<(&Person, Vec<String>)> = Vec::new();
    for e in &d.staff.edges {
        match people.iter_mut().find(|(p, _)| p.id == e.node.id) {
            Some((_, roles)) => roles.push(anitext::staff_role(&e.role)),
            None => people.push((&e.node, vec![anitext::staff_role(&e.role)])),
        }
    }
    let mut cells = Vec::with_capacity(people.len());
    for (p, roles) in people {
        let photo = portrait(v, cx, p.image.as_ref().and_then(|i| i.large.as_deref()), p.name.full.as_deref());
        let who = div().flex_col().min_w(0.0).child(truncate(p.name.full.clone().unwrap_or_default()).text_size(t.typography.sm).font_weight(600)).child(div().flex_col().text_size(t.typography.xs).text_color(t.muted_foreground).line_clamp(2).child(text(roles.join(", "))));
        cells.push(row(&t).child(photo).child(who));
    }
    grid(width, 256.0, cells)
}

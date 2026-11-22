//! "Continuar": o item mais recente em destaque e fileiras "Assistindo" e "Lendo".
//! O formato é recalculado do progresso salvo no máximo a cada 2 s (varre o mapa).

use std::time::{Duration, Instant};

use higanbana_core::anilist::MediaType;
use higanbana_core::continue_list::{reading, watching, ContinueItem};
use higanbana_core::library::{folder_title, parse_episode, strip_ext};
use higanbana_gpui::components::Carousel;
use higanbana_gpui::prelude::*;

use super::continue_card::{card, featured, CARD_H};
use super::eyebrow;
use crate::root::Higanbana;

/// Obras por fileira.
const ROW_LIMIT: usize = 12;
const RESHAPE: Duration = Duration::from_secs(2);
/// Largura de cada cartão das fileiras e espaço lateral para as setas.
pub const CARD_W: f32 = 272.0;
const ARROWS: f32 = 48.0;

/// Texto de progresso de um item: episódio e quanto falta, ou quanto já leu.
pub fn item_label(file_name: &str, item: &ContinueItem, reading: bool) -> String {
    if reading {
        let name = folder_title(strip_ext(file_name));
        return if item.done { format!("{name} · lido") } else { format!("{name} · {}% lido", (item.fraction * 100.).round()) };
    }
    let ep = parse_episode(file_name);
    let name = ep.number.map_or_else(|| ep.label.clone(), |n| format!("Ep {n}"));
    match item.remaining_secs {
        _ if item.done => format!("{name} · visto"),
        Some(s) if s < 60 => format!("{name} · falta menos de 1 min"),
        Some(s) => format!("{name} · faltam {} min", (s as f64 / 60.).round()),
        None => name,
    }
}

/// Seção do acervo de um caminho (para a busca de metadados).
pub fn media_of(path: &str) -> MediaType {
    if path.starts_with("Mangás/") { MediaType::Manga } else { MediaType::Anime }
}

#[derive(Clone, Default, PartialEq)]
pub struct Shape {
    pub featured: Option<(ContinueItem, bool)>,
    pub watching: Vec<ContinueItem>,
    pub reading: Vec<ContinueItem>,
}

/// Destaque = o mais recente entre vídeo e leitura; o resto vai para as fileiras.
pub fn shape_of(mut watch: Vec<ContinueItem>, mut read: Vec<ContinueItem>) -> Shape {
    let featured = match (watch.first().map(|i| i.updated_at), read.first().map(|i| i.updated_at)) {
        (Some(w), Some(r)) if r > w => Some((read.remove(0), true)),
        (Some(_), _) => Some((watch.remove(0), false)),
        (None, Some(_)) => Some((read.remove(0), true)),
        (None, None) => None,
    };
    watch.truncate(ROW_LIMIT);
    read.truncate(ROW_LIMIT);
    Shape { featured, watching: watch, reading: read }
}

fn current_shape(v: &mut Higanbana, now: Instant) -> Shape {
    if let Some((s, at)) = &v.pages.home().shape
        && now.saturating_duration_since(*at) < RESHAPE
    {
        return s.clone();
    }
    let watch = watching(&v.state.player_data.progress, ROW_LIMIT + 1);
    let read = reading(&v.state.core.reader().store.data().progress, ROW_LIMIT + 1);
    let shape = shape_of(watch, read);
    let ui = v.pages.home();
    ui.watch.set_len(shape.watching.len(), now);
    ui.read.set_len(shape.reading.len(), now);
    ui.shape = Some((shape.clone(), now));
    shape
}

fn row(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, label: &'static str, items: &[ContinueItem], is_reading: bool) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let mut cards = Vec::with_capacity(items.len());
    for (i, it) in items.iter().enumerate() {
        cards.push(card(v, cx, it, is_reading, i));
    }
    let ui = v.pages.home();
    let state = if is_reading { ui.read.clone() } else { ui.watch.clone() };
    let car = Carousel::new(("continuar-fileira", label), &state, move |v: &mut Higanbana| if is_reading { &mut v.pages.home().read } else { &mut v.pages.home().watch })
        .items(cards)
        .item_width(CARD_W)
        .gap(16.0)
        .controls(true)
        .render(cx);
    div()
        .flex_col()
        .gap(8.0)
        .child(div().text_size(t.typography.sm).font_weight(600).text_color(t.muted_foreground).child(text(label)))
        .child(div().px(ARROWS).h(CARD_H).child(car))
}

/// O bloco inteiro; `None` quando não há nada para continuar.
pub fn block(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Option<Element<Higanbana>> {
    let shape = current_shape(v, cx.now());
    let (item, is_reading) = shape.featured.clone()?;
    let t = cx.theme().clone();
    let mut col = div().w_full().flex_col().gap(16.0).child(div().h(36.0).items_center().child(eyebrow(&t, "play", "Continuar"))).child(featured(v, cx, &item, is_reading));
    if !shape.watching.is_empty() {
        col = col.child(row(v, cx, "Assistindo", &shape.watching, false));
    }
    if !shape.reading.is_empty() {
        col = col.child(row(v, cx, "Lendo", &shape.reading, true));
    }
    Some(col)
}

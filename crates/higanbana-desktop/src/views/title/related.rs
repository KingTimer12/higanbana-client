//! Abas Relacionadas (abrem no AniList, no navegador) e Detalhes (fichas e temas).

use higanbana_core::anilist::{text as anitext, types::MediaDetail, MediaType};
use higanbana_gpui::components::{Component, Tag};
use higanbana_gpui::prelude::*;
use higanbana_gpui::{Corners, ElementId};

use crate::root::Higanbana;
use crate::state::details::open_url;
use crate::theme::primary_ink;
use crate::ui::cover::columns;
use crate::ui::hover;
use crate::ui::image::{hex_color, remote_image};

pub fn related(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, d: &MediaDetail, width: f32) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let gap = 20.0;
    let cols = columns(width, 132.0, gap);
    let card_w = ((width - (cols as f32 - 1.0) * gap) / cols as f32).floor();
    let mut grid = div().flex_wrap().gap_x(gap).gap_y(32.0).items_start();
    for e in &d.relations.edges {
        let n = &e.node;
        let kind = n.media_type.map_or("anime", |t| if t == MediaType::Manga { "manga" } else { "anime" });
        let url = format!("https://anilist.co/{kind}/{}", n.id);
        let bg = n.cover_image.as_ref().and_then(|c| hex_color(c.color.as_deref())).unwrap_or(t.muted);
        let id = ElementId::from(("relacionada", n.id as usize));
        let ring = hover::layer(hover::amount(cx, id), Corners::all(t.radius_lg), None, Some(t.ring));
        let art = div().relative().mb(4.0).w(card_w).h(card_w * 1.5).rounded(t.radius_lg).overflow_hidden().bg(bg).child(remote_image(v, cx, n.cover_image.as_ref().and_then(|c| c.large.as_deref()), t.radius_lg)).child(ring);
        let year = n.start_date.and_then(|d| d.year).map(|y| y.to_string()).unwrap_or_default();
        let card = div()
            .w(card_w)
            .flex_col()
            .gap(4.0)
            .cursor_pointer()
            .child(art)
            .child(div().text_size(t.typography.xs).font_weight(600).text_color(primary_ink(&t)).child(text(anitext::relation(Some(&e.relation_type)))))
            .child(div().flex_col().text_size(t.typography.sm).font_weight(500).line_clamp(2).child(text(n.title.user_preferred.clone().unwrap_or_default())))
            .child(div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text(format!("{} {year}", anitext::format(n.format.as_deref())))))
            .on_click(move |_: &mut Higanbana, _, _| open_url(&url));
        grid = grid.child(hover::track(card, id));
    }
    div().flex_col().gap(20.0).child(div().text_size(t.typography.sm).text_color(t.muted_foreground).child(text("Abrem no AniList, no navegador."))).child(grid)
}

type FactGroup<'a> = (&'a str, Vec<(&'a str, Option<String>)>);

pub fn facts(cx: &mut RenderCx<'_, Higanbana>, d: &MediaDetail) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let site = if d.summary.is_mal_only() { "MyAnimeList" } else { "AniList" };
    let m = &d.summary;
    let groups: Vec<FactGroup> = vec![
        (
            "Obra",
            vec![
                ("Formato", m.format.as_deref().map(|f| anitext::format(Some(f)))),
                ("Origem", d.source.as_deref().map(|s| anitext::source(Some(s)))),
                ("Estúdio", Some(d.studios.nodes.iter().map(|s| s.name.clone()).collect::<Vec<_>>().join(", ")).filter(|s| !s.is_empty())),
                ("Episódios", m.episodes.map(|e| e.to_string())),
                ("Duração", d.duration.map(|x| format!("{x} min por episódio"))),
                ("Capítulos", m.chapters.map(|c| c.to_string())),
                ("Volumes", d.volumes.map(|v| v.to_string())),
            ],
        ),
        (
            "Exibição",
            vec![
                ("Situação", m.status.as_deref().map(|s| anitext::status(Some(s)))),
                ("Temporada", d.summary.season.as_deref().zip(m.season_year).map(|(s, y)| format!("{} de {y}", anitext::season(Some(s))))),
                ("Estreia", anitext::date(m.start_date)),
                ("Fim", anitext::date(d.end_date)),
            ],
        ),
        (
            site,
            vec![
                ("Nota média", m.average_score.map(|s| format!("{s}%"))),
                ("Popularidade", m.popularity.map(|p| format!("{} pessoas", anitext::number(p)))),
                ("Outros nomes", Some(m.synonyms.iter().take(4).cloned().collect::<Vec<_>>().join("; ")).filter(|s| !s.is_empty())),
            ],
        ),
    ];
    let mut cols = div().gap(32.0).items_start();
    for (g, rows) in groups {
        let rows: Vec<(&str, String)> = rows.into_iter().filter_map(|(k, v)| v.map(|v| (k, v))).collect();
        if rows.is_empty() {
            continue;
        }
        let mut col = div().flex_col().flex_1().min_w(0.0).child(div().mb(8.0).text_size(t.typography.sm).font_weight(600).child(text(g)));
        for (k, v) in rows {
            let line = div().relative().items_start().justify_between().gap(24.0).py(8.0).text_size(t.typography.sm).child(div().flex_shrink_0().text_color(t.muted_foreground).child(text(k))).child(div().flex_col().items_end().child(text(v))).child(div().absolute().left(0.0).right(0.0).bottom(0.0).h(1.0).bg(t.border));
            col = col.child(line);
        }
        cols = cols.child(col);
    }
    let tags: Vec<&str> = d.tags.iter().filter(|t| !t.spoiler && t.rank >= 60).take(12).map(|t| t.name.as_str()).collect();
    let mut out = div().flex_col().gap(32.0).child(cols);
    if !tags.is_empty() {
        let mut row = div().flex_wrap().gap(6.0);
        for tag in tags {
            row = row.child(Tag::secondary(tag.to_owned()).small().render(cx));
        }
        out = out.child(div().flex_col().gap(8.0).child(text("Temas").text_size(t.typography.sm).font_weight(600)).child(row));
    }
    out
}

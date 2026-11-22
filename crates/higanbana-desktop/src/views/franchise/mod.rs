//! Página de uma franquia: pasta cujas subpastas são obras datadas
//! (`[2004] Futari wa Precure [TV]`, `[2005.1] Movie 1 [FILME]`). Custa uma
//! listagem só (a da franquia); cada obra se apresenta pelo nome e pelo AniList/MAL.

use higanbana_core::anilist::MediaType;
use higanbana_core::franchise::{franchise_span, franchise_tabs};
use higanbana_core::library::{base_name, folder_title, join_path, natural_cmp, parse_work_name, raw_path};
use higanbana_gpui::components::{Button, Component, ComponentSize, TabBar, Variant};
use higanbana_gpui::prelude::*;
use higanbana_gpui::ElementId;

use crate::root::Higanbana;
use crate::ui::cover::{columns, content_width, cover, poster, want_lookup, PosterProps};
use crate::ui::paths::breadcrumbs;
use crate::ui::SHADOW_LG;
use crate::views::title::data::Model;

const GAP: f32 = 20.0;

pub fn page(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, m: &Model) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let width = content_width(v, cx, 1152.0);
    let cols = columns(width, 152.0, GAP);
    let card_w = ((width - (cols as f32 - 1.0) * GAP) / cols as f32).floor();
    let mut names: Vec<String> = m.title_dirs.iter().map(|d| d.name.clone()).collect();
    names.sort_by(|a, b| natural_cmp(a, b));
    let franchise = base_name(&m.title_path).to_owned();
    // Todas as obras de uma vez: o formato do AniList/MAL decide a aba (poucos lotes).
    v.state.want_lookup(&franchise, MediaType::Anime);
    for n in &names {
        v.state.want_lookup(n, MediaType::Anime);
    }
    v.state.set_inner_works_if_changed(&m.title_path, &names);
    let hero_media = v.state.lookup(&franchise, MediaType::Anime).flatten();
    let looked: Vec<_> = names.iter().map(|n| v.state.lookup(n, MediaType::Anime)).collect();
    let parsed: Vec<_> = names.iter().map(|n| parse_work_name(n)).collect();
    let input: Vec<_> = parsed.iter().zip(&looked).map(|(w, l)| (w, l.clone().flatten().and_then(|m| m.format.clone()))).collect();
    let input_ref: Vec<_> = input.iter().map(|(w, f)| (*w, f.as_deref())).collect();
    let tabs = franchise_tabs(&input_ref);
    let chosen = v.pages.title().franchise_tab;
    let active = chosen.and_then(|c| tabs.iter().position(|(tab, _)| *tab == c)).unwrap_or(0);
    let span = franchise_span(&parsed.iter().collect::<Vec<_>>());
    let raw = raw_path(&m.title_path);
    let folder = Button::new("franquia-pasta", "Ver pasta").variant(Variant::Ghost).size(ComponentSize::Compact).icon(icon("folder").size(14.0)).on_click(move |v: &mut Higanbana, _, cx| {
        v.state.go(raw.clone());
        cx.notify();
    });
    let names_col = div().flex_col().gap(4.0).child(div().flex_col().text_size(30.0).font_weight(700).child(text(folder_title(&franchise)))).child(div().text_size(t.typography.sm).text_color(t.muted_foreground).child(text(span)));
    let info = div().flex_col().flex_1().min_w(0.0).gap(12.0).child(names_col).child(div().child(folder.render(cx)));
    let art = div().flex_shrink_0().rounded(t.radius_lg).shadow(Color::BLACK.fade(0.25), SHADOW_LG).child(cover(v, cx, hero_media.as_deref(), &franchise, Some(144.0)));
    let hero = div().items_end().gap(24.0).child(art).child(info);
    let mut section = div().flex_col().gap(24.0);
    // Uma aba só dispensa a barra.
    if tabs.len() > 1 {
        let ids: Vec<_> = tabs.iter().map(|x| x.0).collect();
        let mut bar = TabBar::new("franquia-abas", active, move |v: &mut Higanbana, i, _| v.pages.title().franchise_tab = ids.get(i).copied()).underline();
        for (tab, ix) in &tabs {
            bar = bar.tab(tab.label(ix.len()));
        }
        section = section.child(bar.render(cx));
    }
    let shown = tabs.get(active).map(|x| x.1.clone()).unwrap_or_default();
    let mut grid = div().flex_wrap().gap_x(GAP).gap_y(32.0).items_start();
    for i in shown {
        let (name, work, meta) = (&names[i], &parsed[i], looked[i].clone().flatten());
        let pending = looked[i].is_none();
        let year = work.year.map(|y| y.to_string()).or_else(|| meta.as_ref().and_then(|m| m.year()).map(|y| y.to_string()));
        let episodes = meta.as_ref().and_then(|m| m.episodes).filter(|&e| e > 1).map(|e| format!("{e} ep"));
        let line = [year, episodes].into_iter().flatten().collect::<Vec<_>>().join(" · ");
        let target = join_path(&m.title_path, name);
        let card = poster(v, cx, PosterProps { id: ElementId::from(("obra", name.as_str())), name: name.clone(), media: meta, loading: pending, width: card_w, badge: false, dimmed: false, meta: Some(line) }).on_click(move |v: &mut Higanbana, _, cx| {
            v.state.go(target.clone());
            cx.notify();
        });
        let mut cell = div().relative().child(card);
        if pending {
            cell = cell.child(want_lookup(name.clone(), MediaType::Anime));
        }
        grid = grid.child(cell);
    }
    let body = div().w(width).flex_col().pt(24.0).pb(64.0).gap(32.0).child(breadcrumbs(cx, &m.title_path, false)).child(hero).child(section.child(grid));
    div().w_full().flex_col().items_center().child(body)
}

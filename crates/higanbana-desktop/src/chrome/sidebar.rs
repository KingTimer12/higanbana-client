//! Barra lateral: Início + seções do acervo, grupos com mais de um destino
//! (Mangás: Séries e Revistas), Nuvem, e rodapé com a conta.

use higanbana_core::library::{raw_path, split_path};
use higanbana_gpui::components::{Avatar, Button, Component, ComponentSize, Sidebar, SidebarGroup, SidebarItem, SidebarToggle, Variant};
use higanbana_gpui::prelude::*;

use super::{active_nav, is_library_section, nav_entries, section_label, tip, NavEntry};
use crate::root::Higanbana;
use crate::state::Img;
use crate::ui::truncate;

fn go(target: String) -> impl Fn(&mut Higanbana, &MouseEvent, &mut EventCx) + 'static {
    move |v, _, cx| {
        v.state.go(target.clone());
        cx.notify();
    }
}

fn entry_item(i: usize, e: &NavEntry, label: String, active: Option<usize>) -> SidebarItem<Higanbana> {
    SidebarItem::new(("nav", i), label).icon(e.icon).active(active == Some(i)).on_click(go(e.target.clone()))
}

fn footer(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, collapsed: bool) -> Element<Higanbana> {
    let user = v.state.user.clone();
    let name = user.as_ref().map(|u| u.name.clone()).unwrap_or_default();
    let avatar = user.and_then(|u| u.avatar).map(|url| v.state.image(&url));
    let image = match avatar {
        Some(Img::Ready(src)) => Some(src),
        _ => None,
    };
    // Foto 32 px (`size_8`); sem foto, iniciais no tamanho small do kit (24 px).
    let size = if image.is_some() { ComponentSize::Md } else { ComponentSize::Compact };
    let face = Avatar::new(name.clone()).image(image).size(size).render(cx);
    let settings = Button::new("settings", "").variant(Variant::Ghost).size(ComponentSize::Compact).icon(icon("settings-2").size(14.0)).on_click(|v: &mut Higanbana, _, cx| {
        v.settings_open = true;
        cx.notify();
    });
    let settings = settings.render(cx);
    let settings = tip(v, cx, "dica-settings", settings, "Configurações");
    if collapsed {
        return div().w_full().flex_col().items_center().gap(4.0).child(settings).child(face);
    }
    let logout = Button::new("logout", "").variant(Variant::Ghost).size(ComponentSize::Compact).icon(icon("log-out").size(14.0)).on_click(|v: &mut Higanbana, _, _| {
        v.state.logout();
    });
    let logout = logout.render(cx);
    let logout = tip(v, cx, "dica-logout", logout, "Sair da conta");
    let t = cx.theme();
    let info = div()
        .flex_col()
        .flex_1()
        .min_w(0.0)
        .child(truncate(name).text_size(t.typography.sm).font_weight(600))
        .child(div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text("Via Discord")));
    div().w_full().items_center().gap(8.0).child(face).child(info).child(settings).child(logout)
}

pub fn sidebar(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let collapsed = v.state.sidebar_collapsed();
    v.side.collapsed = collapsed;
    // `raw:Animes/...` continua dentro da seção Animes.
    let raw = raw_path("");
    let full = v.state.path().to_owned();
    let path = full.strip_prefix(raw.as_str()).unwrap_or(&full).to_owned();
    let current = split_path(&path).first().map(|s| s.to_string()).unwrap_or_default();
    let entries = nav_entries(&v.state.sections);
    let active = if current.is_empty() { None } else { active_nav(&entries, &path) };

    let mut library = SidebarGroup::new("Acervo").item(SidebarItem::new("inicio", "Início").icon("house").active(current.is_empty()).on_click(go(String::new())));
    let mut section_groups = Vec::new();
    let mut cloud = SidebarGroup::new("Nuvem");
    let mut has_cloud = false;
    let mut ix = 0;
    while ix < entries.len() {
        let e = &entries[ix];
        let kids: Vec<usize> = (ix..entries.len()).take_while(|j| entries[*j].section == e.section).collect();
        if kids.len() > 1 {
            // Dentro do grupo "Mangás", o destino principal se chama "Séries".
            let mut g = SidebarGroup::new(section_label(&e.section));
            for j in &kids {
                let k = &entries[*j];
                let label = if k.label == k.section { "Séries".to_owned() } else { k.label.clone() };
                g = g.item(entry_item(*j, k, label, active));
            }
            section_groups.push(g);
        } else if is_library_section(&e.section) {
            library = library.item(entry_item(ix, e, e.label.clone(), active));
        } else {
            has_cloud = true;
            cloud = cloud.item(entry_item(ix, e, e.label.clone(), active));
        }
        ix += kids.len().max(1);
    }

    let toggle = SidebarToggle::new("recolher", collapsed)
        .on_click(|v: &mut Higanbana, _, cx| {
            v.state.toggle_sidebar();
            cx.notify();
        })
        .render(cx);
    let t = cx.theme();
    let header = if collapsed {
        div().w_full().items_center().justify_center().child(toggle)
    } else {
        div()
            .w_full()
            .items_center()
            .justify_between()
            .child(div().pl(8.0).text_size(t.typography.xs).font_weight(600).text_color(t.muted_foreground).child(text("Navegação")))
            .child(toggle)
    };
    let footer = footer(v, cx, collapsed);
    let mut bar = Sidebar::new("nav", &v.side, |v: &mut Higanbana| &mut v.side).header(header).group(library);
    for g in section_groups {
        bar = bar.group(g);
    }
    if has_cloud {
        bar = bar.group(cloud);
    }
    bar.footer(footer).render(cx)
}

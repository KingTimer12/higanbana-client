//! Sidebar.
use super::preview;
use crate::Gallery;
use higanbana_gpui::components::*;
use higanbana_gpui::prelude::*;

const ITEMS: [(&str, &str); 4] = [("Início", "house"), ("Animes", "tv"), ("Mangás", "book-open"), ("OSTs", "music")];

pub fn sidebar(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let toggle = SidebarToggle::new("sb-toggle", v.side.collapsed)
        .on_click(|v: &mut Gallery, _, cx| {
            v.side.collapsed = !v.side.collapsed;
            cx.notify();
        })
        .render(cx);
    let mut group = SidebarGroup::new("Acervo");
    for (i, (label, ic)) in ITEMS.iter().enumerate() {
        group = group.item(SidebarItem::new(("sb-item", i), *label).icon(ic).active(v.side_active == i).on_click(move |v: &mut Gallery, _, cx| {
            v.side_active = i;
            cx.notify();
        }));
    }
    let footer = div().items_center().gap(8.0).child(Avatar::new("Higan Bana").size(ComponentSize::Compact).render(cx));
    let bar = Sidebar::new("sb", &v.side, |v: &mut Gallery| &mut v.side).header(toggle).group(group).footer(footer).render(cx);
    let frame = div().h(360.0).w(320.0).border(1.0, cx.theme().border).rounded(cx.theme().radius).overflow_hidden().child(bar);
    vec![preview(cx, "Recolhe para ícones", frame)]
}

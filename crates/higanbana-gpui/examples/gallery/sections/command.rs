//! Command (paleta de busca).
use super::preview;
use crate::Gallery;
use higanbana_gpui::components::*;
use higanbana_gpui::prelude::*;

const ANIMES: [&str; 4] = ["Shingeki no Kyojin", "Sousou no Frieren", "Mushishi", "Kaiju No. 8"];
const MANGAS: [&str; 3] = ["Berserk", "Vagabond", "Oyasumi Punpun"];

pub fn command(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let q = v.cmd.query().to_lowercase();
    let hit = |s: &&&str| s.to_lowercase().contains(&q);
    let mut c = Command::new("cmd", &v.cmd, |v: &mut Gallery| &mut v.cmd)
        .placeholder("Buscar animes, mangás, OSTs…")
        .max_h(280.0)
        .empty(text("Nada encontrado"))
        .on_query(|v: &mut Gallery, _, cx| {
            v.cmd.reset_selection();
            cx.notify();
        })
        .on_confirm(|v: &mut Gallery, (g, i), cx| {
            v.toasts.push(Toast::new(format!("Abrir {g}/{i}")));
            cx.notify();
        });
    for (label, items) in [("Animes", &ANIMES[..]), ("Mangás", &MANGAS[..])] {
        let mut g = CommandGroup::new(label);
        for (i, name) in items.iter().filter(hit).enumerate() {
            g = g.item(CommandItem::new(("cmd-item", label.len() * 100 + i)).child(icon("folder").size(16.0)).child(text(*name)));
        }
        c = c.group(g);
    }
    let frame = div().w(480.0).border(1.0, cx.theme().border).rounded(cx.theme().radius_lg).overflow_hidden().child(c.render(cx));
    vec![preview(cx, "Paleta", frame)]
}

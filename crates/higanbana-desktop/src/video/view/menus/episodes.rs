//! Menu "Episódios": um por linha, atual destacado, progresso e visto.

use higanbana_core::library::{join_path, parse_episode};
use higanbana_core::player::logic::{episode_number, video_files};
use higanbana_gpui::components::{Component, Progress, ProgressSize};
use higanbana_gpui::prelude::*;
use higanbana_gpui::ElementId;

use super::super::style::menu_heading;
use crate::root::Higanbana;
use crate::theme::primary_ink;
use crate::ui::truncate;

pub fn content(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let Some(p) = v.player.as_ref() else { return div() };
    let t = cx.theme().clone();
    let target = &p.session.target;
    let current = target.file.name.clone();
    let mut list = scroll("player-lista-episodios").w(384.0).max_h(384.0).flex_col().child(menu_heading(&t, "Episódios"));
    for f in video_files(&target.files) {
        let n = episode_number(&f.name);
        let prog = p.session.data.progress.get(&join_path(&target.dir, &f.name)).cloned();
        let is_current = f.name == current;
        let label = n.map_or_else(|| parse_episode(&f.name).label, |n| format!("Episódio {n}"));
        let id = ElementId::from(("player-ep", f.name.as_str()));
        let mut info = div().flex_col().flex_1().min_w(0.0).gap(4.0).child(truncate(label).text_size(t.typography.sm));
        if let Some(pr) = prog.as_ref().filter(|p| !p.watched) {
            let val = (pr.position / pr.duration.max(1.0) * 100.0).min(100.0) as f32;
            info = info.child(Progress::new(ElementId::from(("player-ep-prog", f.name.as_str()))).size(ProgressSize::XSmall).value(val).render(cx));
        }
        let num = div()
            .w(32.0)
            .flex_shrink_0()
            .justify_center()
            .text_size(t.typography.lg)
            .font_weight(600)
            .text_color(if is_current { primary_ink(&t) } else { t.muted_foreground })
            .child(text(n.map_or_else(|| "·".to_string(), |n| n.to_string())));
        let mut row = div()
            .id(id)
            .items_center()
            .gap(12.0)
            .px(8.0)
            .py(8.0)
            .rounded(t.radius)
            .cursor_pointer()
            .child(num)
            .child(info)
            .on_click(move |v: &mut Higanbana, _, cx| {
                if let Some(p) = v.player.as_mut() {
                    let a = p.session.switch_to(f.clone(), std::time::Instant::now());
                    p.run(a);
                }
                cx.notify();
            });
        row = if is_current { row.bg(t.list_active) } else { { let h = t.list_hover; row.hover(move |s| s.bg(h)) } };
        if prog.as_ref().is_some_and(|p| p.watched) {
            row = row.child(icon("check").size(14.0).text_color(t.success_foreground));
        }
        list = list.child(row);
    }
    list
}

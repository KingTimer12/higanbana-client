//! Tela do leitor: lombada, página e margem direita; nada flutua sobre a página
//! além dos painéis, do fio de progresso e do cartão de fim.

mod end;
mod margins;
mod page;
pub mod panels;
mod strip;
pub mod thread;

use higanbana_core::reader::logic::{Direction, ReadMode, Turn};
use higanbana_gpui::components::{Component, Skeleton, TitleBar};
use higanbana_gpui::prelude::*;

use super::keys::{char_action, escape_step, key_action, side_turn, EscapeStep, ReaderKey};
use super::nav::Step;
use super::state::Panel;
use super::{ReadTarget, ReaderUi};
use crate::motion;
use crate::root::Higanbana;
use crate::state::AppState;

pub use margins::MARGIN;

impl AppState {
    /// Abre o leitor (a janela passa a mostrar só ele).
    pub fn read(&mut self, target: ReadTarget) {
        let data = self.core.reader().store.data();
        self.reader = Some(ReaderUi::new(target, &data));
        self.pump_reader();
    }
}

/// Fecha: sai da tela cheia e solta tudo do leitor (páginas inclusive).
pub fn close(v: &mut Higanbana, cx: &mut EventCx) {
    if v.window_fullscreen {
        cx.window_command(WindowCommand::ExitFullscreen);
    }
    v.state.reader = None;
    cx.notify();
}

pub fn panel_toggle(v: &mut Higanbana, p: Panel, cx: &mut EventCx) {
    if let Some(r) = v.state.reader.as_mut() {
        r.toggle(p);
    }
    cx.notify();
}

fn save_prefs(v: &mut Higanbana) {
    if let Some(r) = v.state.reader.as_ref() {
        v.state.core.reader().store.set_prefs(&r.target.title_path, r.prefs);
    }
}

pub fn toggle_direction(v: &mut Higanbana) {
    if let Some(r) = v.state.reader.as_mut() {
        r.toggle_direction();
    }
    save_prefs(v);
}

pub fn set_mode(v: &mut Higanbana, m: ReadMode) {
    if let Some(r) = v.state.reader.as_mut() {
        r.set_mode(m);
    }
    save_prefs(v);
    v.state.pump_reader();
}

/// Aplica uma virada de página.
fn turn(v: &mut Higanbana, t: Turn, cx: &mut EventCx) {
    let Some(r) = v.state.reader.as_mut() else { return };
    let step = if t == Turn::Next { r.next() } else { r.prev() };
    match step {
        Step::Moved => v.state.pump_reader(),
        Step::Scroll(sign) => scroll_screen(v, sign, cx),
        Step::None => {}
    }
}

/// Faixa: rola 85% da área no sentido de leitura.
fn scroll_screen(v: &mut Higanbana, sign: f32, cx: &mut EventCx) {
    let Some(r) = v.state.reader.as_ref() else { return };
    let (offset, view) = r.strip_scroll;
    let long = r.prefs.mode == ReadMode::Long;
    let mut d = view * 0.85 * sign;
    if !long && r.prefs.direction == Direction::Rtl {
        d = -d;
    }
    cx.scroll_to(if long { strip::LONG } else { strip::WIDE }, (offset + d).max(0.0));
}

fn go_chapter(v: &mut Higanbana, delta: isize) {
    let Some(r) = v.state.reader.as_mut() else { return };
    let i = r.chapter() as isize + delta;
    if i >= 0 && (i as usize) < r.chapters.len() {
        r.go_chapter(i as usize, Some(0));
        v.state.pump_reader();
    }
}

fn apply(v: &mut Higanbana, k: ReaderKey, cx: &mut EventCx) {
    match k {
        ReaderKey::Escape => match escape_step(v.state.reader.as_ref().is_some_and(|r| r.panel.is_some()), v.window_fullscreen) {
            EscapeStep::ClosePanel => {
                if let Some(r) = v.state.reader.as_mut() {
                    r.panel = None;
                }
            }
            EscapeStep::ExitFullscreen => cx.window_command(WindowCommand::ExitFullscreen),
            EscapeStep::Close => return close(v, cx),
        },
        ReaderKey::Index => panel_toggle(v, Panel::Index, cx),
        ReaderKey::Direction => toggle_direction(v),
        ReaderKey::Mode => {
            let Some(r) = v.state.reader.as_mut() else { return };
            r.cycle_mode();
            save_prefs(v);
            v.state.pump_reader();
        }
        ReaderKey::Fullscreen => cx.window_command(WindowCommand::ToggleFullscreen),
        ReaderKey::PrevChapter => go_chapter(v, -1),
        ReaderKey::NextChapter => go_chapter(v, 1),
        ReaderKey::Scroll(px) => {
            let offset = v.state.reader.as_ref().map_or(0.0, |r| r.strip_scroll.0);
            cx.scroll_to(strip::LONG, (offset + px).max(0.0));
        }
        ReaderKey::Turn(t) => turn(v, t, cx),
    }
    cx.stop_propagation();
    cx.notify();
}

fn on_key(v: &mut Higanbana, ev: &KeyEvent, cx: &mut EventCx) {
    let Some(r) = v.state.reader.as_ref() else { return };
    // Digitando no filtro do índice: só o Esc vale.
    if r.filter_focused && ev.key != Key::Escape {
        return;
    }
    if let Some(k) = key_action(ev, r.prefs.mode == ReadMode::Long, r.prefs.direction) {
        apply(v, k, cx);
    }
}

fn on_char(v: &mut Higanbana, c: char, cx: &mut EventCx) {
    if v.state.reader.as_ref().is_some_and(|r| r.filter_focused) {
        return;
    }
    if let Some(k) = char_action(c) {
        apply(v, k, cx);
    }
}

/// Clique na área da página: fecha painel, ou vira pela lateral (modos de página).
fn on_area_click(v: &mut Higanbana, ev: &MouseEvent, window_w: f32, cx: &mut EventCx) {
    let Some(r) = v.state.reader.as_mut() else { return };
    if r.panel.is_some() {
        r.panel = None;
        cx.notify();
        return;
    }
    if r.is_strip() {
        return;
    }
    if let Some(t) = side_turn(ev.position.x, window_w, MARGIN, r.prefs.direction) {
        turn(v, t, cx);
        cx.notify();
    }
}

/// Página única ou dupla, centradas na área `w`×`h`.
fn pages_body(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, w: f32, h: f32) -> Element<Higanbana> {
    let Some(r) = v.state.reader.as_ref() else { return div() };
    let Some(u) = r.unit() else { return div() };
    let pages = if r.prefs.mode == ReadMode::Double { r.spread() } else { vec![r.page()] };
    let mut row = div().size_full().items_center().justify_center().px(16.0).pt(16.0).pb(44.0);
    if r.prefs.direction == Direction::Rtl && pages.len() > 1 {
        row = row.flex_row_reverse();
    }
    let (bw, bh) = (((w - 32.0) / pages.len() as f32).max(1.0), (h - 60.0).max(1.0));
    for pg in pages {
        row = row.child(page::page_image(v, cx, &u, pg, bw, Some(bh)));
    }
    row
}

pub fn view(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let focused = cx.is_focused("leitor-filtro");
    let Some(r) = v.state.reader.as_mut() else { return div() };
    r.filter_focused = focused;
    let viewport = cx.viewport();
    let area = cx.bounds("leitor-area").map_or((viewport.width - MARGIN * 2.0, viewport.height - 34.0), |b| (b.size.width, b.size.height));
    let fullscreen = v.window_fullscreen;
    let r = v.state.reader.as_ref().expect("checado acima");
    let (chapter, total, end, strip) = (r.chapter(), r.total(), r.end, r.is_strip());
    let title = r.target.title.clone();
    let failure = r.open_error().map(str::to_owned).or_else(|| r.empty_chapter().then(|| "O arquivo não tem páginas de imagem.".to_owned()));
    let body = match (failure, total) {
        (Some(msg), _) => end::open_failed(v, cx, msg),
        _ if strip => strip::view(v, cx, area.0, area.1),
        (None, 0) => div().size_full().items_center().justify_center().child(Skeleton::new("leitor-esqueleto").w(380.0).h(area.1 * 0.78).render(cx)),
        _ => pages_body(v, cx, area.0, area.1),
    };
    let window_w = viewport.width;
    let mut page_area = div()
        .id("leitor-area")
        .track_bounds()
        .relative()
        .flex_1()
        .min_w(0.0)
        .h_full()
        .overflow_hidden()
        .on_click(move |v: &mut Higanbana, ev, cx| on_area_click(v, ev, window_w, cx))
        .on_double_click(|_: &mut Higanbana, _, cx| cx.window_command(WindowCommand::ToggleFullscreen))
        .child(body);
    let r = v.state.reader.as_ref().expect("checado acima");
    let show_thread = match r.settings.progress {
        higanbana_core::reader::logic::ProgressStyle::Thread => true,
        higanbana_core::reader::logic::ProgressStyle::Hidden => r.stirred,
        _ => false,
    };
    if end && !strip {
        let fade = cx.animate_from(("leitor-fim", chapter), 0.0, 1.0, motion::transition(motion::BASE));
        let card = end::chapter_end(v, cx, chapter);
        page_area = page_area.child(div().absolute().inset(0.0).bg(t.background.fade(0.85)).items_center().justify_center().opacity(fade).child(card));
    }
    if show_thread {
        page_area = page_area.child(thread::thread(v, cx, area.0));
    }
    if let Some(p) = panels::panel(v, cx) {
        page_area = page_area.child(p);
    }
    let row = div().flex_1().min_h(0.0).w_full().child(margins::spine(v, cx)).child(page_area).child(margins::right(v, cx));
    let mut root = div()
        .id("leitor")
        .size_full()
        .flex_col()
        .bg(t.background)
        .text_color(t.foreground)
        .on_key_down(on_key)
        .on_char(on_char)
        .on_mouse_move_passive(|v: &mut Higanbana, _, cx| {
            // Só o progresso escondido reage ao mouse (nada a redesenhar nos outros estilos).
            if let Some(r) = v.state.reader.as_mut()
                && r.settings.progress == higanbana_core::reader::logic::ProgressStyle::Hidden
                && r.stir(cx)
            {
                cx.notify();
            }
        });
    if !fullscreen {
        let name = div().pl(12.0).text_size(t.typography.sm).text_color(t.muted_foreground).child(text(title));
        root = root.child(TitleBar::new("").kit().leading(name).render(cx));
    }
    root.child(row)
}

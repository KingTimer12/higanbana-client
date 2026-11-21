//! Raiz da janela: barra de título + barra lateral + tela atual (ou login).

use std::sync::Arc;

use higanbana_core::{Core, Event};
use higanbana_gpui::components::{Component, Dialog, Notifier, PopoverState, SidebarState, Toasts};
use higanbana_gpui::prelude::*;

use crate::chrome::{self, Tips};
use crate::login::{self, Login};
use crate::motion;
use crate::state::{AppEvent, AppState, Post, Tone};

/// Estado inteiro do app (a lib não guarda estado; tudo mora aqui).
pub struct Higanbana {
    pub state: AppState,
    pub tips: Tips,
    pub side: SidebarState,
    pub dl_pop: PopoverState,
    pub notes: Toasts,
    pub login: Login,
    /// Configurações (diálogo provisório até o Plano 4).
    pub settings_open: bool,
}

impl Higanbana {
    pub fn new(core: Arc<Core>, post: Post) -> Self {
        Self {
            state: AppState::new(core, post),
            tips: Tips::default(),
            side: SidebarState::default(),
            dl_pop: PopoverState::default(),
            notes: Toasts::new(),
            login: Login::default(),
            settings_open: false,
        }
    }

    /// Evento do núcleo vindo de outra thread (pela `Mailbox`).
    pub fn on_core_event(&mut self, e: Event, cx: &mut EventCx) {
        match &e {
            Event::AuthChanged(session) => {
                self.state.set_session(session.as_ref());
                self.login = Login::default();
            }
            Event::DownloadProgress(p) => self.state.on_download_progress(p),
            // Player de músicas e volta do login pelo navegador: Plano 4.
            Event::MusicState(_) | Event::LoginReturned => return,
        }
        cx.notify();
    }

    /// Abre a paleta (só logado) e foca o campo.
    pub fn open_search(&mut self, cx: &mut EventCx) {
        if self.state.user.is_some() && self.state.open_search() {
            cx.focus(chrome::palette_input());
        }
        cx.notify();
    }

    fn on_key(&mut self, ev: &KeyEvent, cx: &mut EventCx) {
        let m = ev.mods;
        match ev.key {
            Key::Char('k') | Key::Char('K') if m.ctrl && !m.alt => self.open_search(cx),
            Key::Left if m.alt => self.state.back(),
            Key::Right if m.alt => self.state.forward(),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// Avisos do estado viram notificações.
    fn drain_events(&mut self) {
        for e in self.state.take_events() {
            match e {
                AppEvent::Notify(Tone::Ok, m) => {
                    self.notes.push_success(m);
                }
                AppEvent::Notify(Tone::Error, m) => {
                    self.notes.push_error(m);
                }
                AppEvent::Navigated => {}
            }
        }
    }

    fn workspace(&mut self, cx: &mut RenderCx<'_, Self>) -> Element<Self> {
        self.state.load_sections();
        let path = self.state.path().to_owned();
        // A tela nova entra com fade e um deslize curto no sentido da navegação.
        let shift = if self.state.nav.went_back() { -motion::PAGE_SHIFT } else { motion::PAGE_SHIFT };
        let key = higanbana_gpui::ElementId::from(("pagina", path.as_str()));
        let t = cx.animate_from(key, 0.0, 1.0, motion::transition(motion::BASE));
        let page = crate::pages::page(self, cx);
        let body = div().id(key).w_full().opacity(t).ml(shift * (1.0 - t)).child(page);
        let main = scroll(("principal", path.as_str())).flex_1().min_w(0.0).child(body);
        let bar = chrome::sidebar(self, cx);
        div().size_full().child(bar).child(main)
    }
}

impl View for Higanbana {
    fn render(&mut self, cx: &mut RenderCx<'_, Self>) -> Element<Self> {
        self.drain_events();
        let t = cx.theme();
        let root = div()
            .size_full()
            .flex_col()
            .bg(t.background)
            .text_color(t.foreground)
            .on_key_down(|v: &mut Self, ev, cx| v.on_key(ev, cx))
            .on_navigate(|v: &mut Self, d, cx| {
                match d {
                    NavDirection::Back => v.state.back(),
                    NavDirection::Forward => v.state.forward(),
                }
                cx.notify();
            });
        if !self.state.session_ready {
            return root;
        }
        let notifier = Notifier::new("avisos", &self.notes, |v: &mut Self| &mut v.notes).render(cx);
        if self.state.user.is_none() {
            let brand = chrome::brand(cx);
            let bar = higanbana_gpui::components::TitleBar::new("").kit().leading(brand).render(cx);
            let page = login::view(self, cx);
            return root.child(bar).child(div().flex_1().min_h(0.0).child(page)).child(notifier);
        }
        let bar = chrome::title_bar(self, cx);
        let work = self.workspace(cx);
        let palette = chrome::palette(self, cx);
        let settings = Dialog::new("configuracoes", self.settings_open)
            .title("Configurações")
            .description("As configurações voltam quando esta tela for migrada.")
            .on_open_change(|v: &mut Self, open, cx| {
                v.settings_open = open;
                cx.notify();
            })
            .render(cx);
        root.child(bar).child(div().flex_1().min_h(0.0).child(work)).child(palette).child(settings).child(notifier)
    }
}

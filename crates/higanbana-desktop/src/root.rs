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
    /// Player aberto: a janela mostra só ele.
    pub player: Option<crate::video::PlayerView>,
    /// Destino de handlers atrasados depois de fechar o player.
    pub player_spare: crate::video::Spare,
    /// Pedido de abrir o player (criado no render, que tem o `Waker` da janela).
    pending_play: Option<crate::video::PlayTarget>,
    /// Espelho do estado da janela para os atalhos (o `EventCx` não lê a janela).
    pub window_fullscreen: bool,
    /// Estado da tela aberta do acervo (solto ao trocar de rota).
    pub pages: crate::views::Pages,
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
            player: None,
            player_spare: Default::default(),
            pending_play: dev_play_target(),
            window_fullscreen: false,
            pages: crate::views::Pages::default(),
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

    /// Fecha o player: sai da tela cheia, grava o progresso e relê os dados do player.
    pub fn close_player(&mut self, cx: &mut EventCx) {
        if let Some(mut p) = self.player.take() {
            p.close();
        }
        if self.window_fullscreen {
            cx.window_command(WindowCommand::ExitFullscreen);
        }
        self.state.player_data = self.state.core.player().data();
        cx.notify();
    }

    /// Cria o player pedido (precisa do `Waker` da janela).
    fn open_pending(&mut self, cx: &mut RenderCx<'_, Self>) {
        // Sem sessão a nuvem não serve o arquivo: espera estar logado.
        if !self.state.session_ready || self.state.user.is_none() {
            return;
        }
        let Some(target) = self.pending_play.take() else { return };
        if let Some(mut old) = self.player.take() {
            old.close();
        }
        let data = self.state.core.player().data();
        self.player = Some(crate::video::PlayerView::open(self.state.core.clone(), self.state.post(), target, data, cx.waker()));
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
                AppEvent::Play(t) => self.pending_play = Some(t),
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
        self.pages.sync(&path);
        let page = crate::pages::page(self, cx);
        let body = div().id(key).w_full().opacity(t).ml(shift * (1.0 - t)).child(page);
        // Catálogo e pasta rolam sozinhos (lista virtual); o resto rola aqui (volta ao topo ao navegar).
        let main = if crate::pages::own_scroll(&path) {
            div().flex_1().min_w(0.0).min_h(0.0).child(body.size_full())
        } else {
            scroll(("principal", path.as_str())).flex_1().min_w(0.0).child(body)
        };
        let bar = chrome::sidebar(self, cx);
        div().size_full().child(bar).child(main)
    }
}

impl View for Higanbana {
    fn render(&mut self, cx: &mut RenderCx<'_, Self>) -> Element<Self> {
        self.drain_events();
        self.window_fullscreen = cx.window_state().fullscreen;
        self.open_pending(cx);
        if self.player.is_some() {
            return crate::video::view(self, cx);
        }
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

/// Dev: `HIGANBANA_PLAY=<pasta>|<arquivo>|<bytes>` abre direto no player (conferência visual
/// sem as telas do Plano 4). Só o arquivo pedido; nada de listar a pasta na nuvem.
fn dev_play_target() -> Option<crate::video::PlayTarget> {
    let raw = std::env::var("HIGANBANA_PLAY").ok()?;
    let mut parts = raw.split('|');
    let (dir, name) = (parts.next()?, parts.next()?);
    let size = parts.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    let ext = name.rfind('.').map(|i| name[i..].to_owned());
    let file = higanbana_core::nuvem::FileEntry { name: name.to_owned(), is_directory: false, size, modified: String::new(), extension: ext };
    let title = dir.rsplit('/').next().unwrap_or(dir).to_owned();
    Some(crate::video::PlayTarget { dir: dir.to_owned(), files: vec![file.clone()], file, mal_id: None, is_anime: dir.starts_with("Animes"), anime_title: title })
}

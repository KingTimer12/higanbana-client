//! Raiz da janela: barra de título + barra lateral + tela atual, ou o
//! player/leitor ocupando a janela inteira.

use std::{rc::Rc, sync::Arc};

use futures::{StreamExt, channel::mpsc::UnboundedReceiver};
use gpui_kit::{
    AnyView, AnyWindowHandle, Context, Entity, FocusHandle, IntoElement, KeyBinding, MouseButton, NavigationDirection,
    ParentElement, Render, SharedString, Styled, Subscription, Window, actions, div, prelude::*, AnimationExt as _,
};
use gpui_kit::component::{
    ActiveTheme as _, WindowExt as _, h_flex, v_flex,
    notification::{Notification, NotificationType},
};
use higanbana_core::{Core, Event, library::{Place, classify}};

use crate::{
    chrome::{SearchPalette, sidebar, sort_sections, title_bar},
    music::bar::MusicBar,
    reader::{ReadTarget, ReaderEvent, ReaderView},
    state::{AppEvent, AppState, Tone, state},
    video::{PlayTarget, PlayerEvent, PlayerView, Surface},
    views::{catalog::CatalogView, folder::FolderView, home::HomeView, login::LoginView, shelf::ShelfView, title::TitleView},
};

actions!(higanbana, [OpenSearch, Back, Forward]);

pub fn bind_keys(cx: &mut gpui_kit::App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-k", OpenSearch, None),
        KeyBinding::new("alt-left", Back, None),
        KeyBinding::new("alt-right", Forward, None),
    ]);
}

pub struct AppView {
    state: Entity<AppState>,
    surface: Rc<Surface>,
    page: Option<(String, AnyView)>,
    login: Option<Entity<LoginView>>,
    palette: Entity<SearchPalette>,
    sections: Vec<String>,
    music: Entity<MusicBar>,
    player: Option<(Entity<PlayerView>, Subscription)>,
    reader: Option<(Entity<ReaderView>, Subscription)>,
    /// Foco da janela: sem ele nenhum atalho global (Ctrl+K, Alt+←) chega aqui.
    focus: FocusHandle,
    _subs: Vec<Subscription>,
}

impl AppView {
    pub fn new(core: Arc<Core>, surface: Rc<Surface>, mut events: UnboundedReceiver<Event>, window_handle: AnyWindowHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        let _ = core;
        let subs = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.subscribe_in(&state, window, |this, _, e: &AppEvent, window, cx| match e {
                AppEvent::Play(t) => this.play(t.clone(), window, cx),
                AppEvent::Read(t) => this.read(t.clone(), window, cx),
                AppEvent::Notify(tone, message) => {
                    let kind = match tone {
                        Tone::Info => NotificationType::Info,
                        Tone::Ok => NotificationType::Success,
                        Tone::Error => NotificationType::Error,
                    };
                    window.push_notification(Notification::new().message(message.clone()).with_type(kind), cx);
                }
                AppEvent::Navigated => {}
            }),
        ];

        // Eventos do núcleo (outras threads) viram mudanças de estado aqui.
        let st = state.clone();
        cx.spawn(async move |_, cx| {
            while let Some(e) = events.next().await {
                st.update(cx, |s, cx| match &e {
                    Event::AuthChanged(session) => s.set_session(session.as_ref(), cx),
                    Event::DownloadProgress(p) => s.on_download_progress(p, cx),
                    Event::MusicState(m) => s.on_music_state(m.clone(), cx),
                    Event::LoginReturned => {}
                });
                if matches!(e, Event::LoginReturned) {
                    let _ = window_handle.update(cx, |_, w, _| w.activate_window());
                }
            }
        })
        .detach();

        crate::views::load_listing(&state, String::new(), cx, |v: &mut Self, r, _| {
            let names = r.map(|f| f.into_iter().filter(|f| f.is_directory).map(|f| f.name).collect()).unwrap_or_default();
            v.sections = sort_sections(names);
        });

        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            focus,
            palette: cx.new(|cx| SearchPalette::new(window, cx)),
            music: cx.new(|cx| MusicBar::new(window, cx)),
            sections: Vec::new(),
            state,
            surface,
            page: None,
            login: None,
            player: None,
            reader: None,
            _subs: subs,
        }
    }

    pub fn play(&mut self, target: PlayTarget, window: &mut Window, cx: &mut Context<Self>) {
        let core = self.state.read(cx).core.clone();
        // Música e vídeo não tocam juntos.
        let _ = core.music().command("pause");
        let surface = self.surface.clone();
        crate::theme::set_video_backdrop(true, window, cx);
        self.state.update(cx, |s, cx| s.set_playing(true, cx));
        let player = cx.new(|cx| PlayerView::new(core, target, surface, window, cx));
        let sub = cx.subscribe_in(&player, window, |this, _, e: &PlayerEvent, window, cx| match e {
            PlayerEvent::Close => {
                crate::theme::set_video_backdrop(false, window, cx);
                this.player = None;
                window.focus(&this.focus, cx);
                this.state.update(cx, |s, cx| {
                    s.set_playing(false, cx);
                    s.refresh_player_data(cx);
                });
                cx.notify();
            }
        });
        self.player = Some((player, sub));
        cx.notify();
    }

    pub fn read(&mut self, target: ReadTarget, window: &mut Window, cx: &mut Context<Self>) {
        let reader = cx.new(|cx| ReaderView::new(target, window, cx));
        let sub = cx.subscribe_in(&reader, window, |this, _, e: &ReaderEvent, window, cx| match e {
            ReaderEvent::Close => {
                this.reader = None;
                window.focus(&this.focus, cx);
                cx.notify();
            }
        });
        self.reader = Some((reader, sub));
        cx.notify();
    }

    /// Tela da rota atual (recriada quando o caminho muda).
    fn page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyView {
        let path = self.state.read(cx).path().to_owned();
        if let Some((p, v)) = &self.page
            && *p == path
        {
            return v.clone();
        }
        let view: AnyView = match classify(&path) {
            Place::Home => cx.new(HomeView::new).into(),
            Place::Shelf { path, media } => cx.new(|cx| ShelfView::new(path, media, window, cx)).into(),
            Place::Title { title_path, rest, media, .. } => cx.new(|cx| TitleView::new(title_path, rest, media, cx)).into(),
            Place::Folder { path } | Place::Raw { path } => cx.new(|cx| FolderView::new(path, cx)).into(),
            Place::Catalog { letter } => cx.new(|cx| CatalogView::new(letter, window, cx)).into(),
        };
        self.page = Some((path, view.clone()));
        view
    }

    fn render_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = self.page(window, cx);
        let has_music = self.state.read(cx).music.as_ref().is_some_and(|m| m.album.is_some());
        let path_key = self.state.read(cx).path().to_owned();
        // O catálogo rola sozinho: o índice lateral fica fixo e pula até uma letra.
        let own_scroll = matches!(classify(&path_key), Place::Catalog { .. });
        // A tela nova aparece com um fade curto: deixa claro que o conteúdo trocou.
        let body = div().when(own_scroll, |d| d.size_full()).child(page).with_animation(
            SharedString::from(format!("enter:{path_key}")),
            crate::motion::enter(crate::motion::BASE),
            |d, t| d.opacity(t),
        );
        let main = div().id(SharedString::from(format!("main:{path_key}"))).flex_1().min_w_0();
        // Uma área rolável por rota: a posição volta ao topo ao navegar.
        let main = if own_scroll { main.min_h_0() } else { main.overflow_y_scroll() };
        let st = self.state.clone();
        let nav = move |back: bool| {
            let st = st.clone();
            move |_: &gpui_kit::MouseUpEvent, _: &mut Window, cx: &mut gpui_kit::App| st.update(cx, |s, cx| if back { s.back(cx) } else { s.forward(cx) })
        };
        v_flex()
            .size_full()
            .on_mouse_up(MouseButton::Navigate(NavigationDirection::Back), nav(true))
            .on_mouse_up(MouseButton::Navigate(NavigationDirection::Forward), nav(false))
            .child(title_bar(&self.state, &self.palette, cx))
            .child(
                h_flex()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .child(sidebar(&self.state, &self.sections, cx))
                    .child(main.child(body)),
            )
            // A barra de música aparece com fade quando um álbum começa a tocar.
            .when(has_music, |d| {
                d.child(div().child(self.music.clone()).with_animation("music-bar-in", crate::motion::enter(crate::motion::BASE), |e, t| {
                    e.opacity(t)
                }))
            })
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div()
            .track_focus(&self.focus)
            .size_full()
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|v, _: &OpenSearch, window, cx| {
                if v.player.is_none() && v.reader.is_none() && v.state.read(cx).user.is_some() {
                    SearchPalette::open(&v.palette, window, cx);
                }
            }))
            .on_action(cx.listener(|v, _: &Back, _, cx| v.state.update(cx, |s, cx| s.back(cx))))
            .on_action(cx.listener(|v, _: &Forward, _, cx| v.state.update(cx, |s, cx| s.forward(cx))));
        if let Some((player, _)) = &self.player {
            // Sem fundo: o vídeo aparece pelo que o GPUI deixa transparente.
            return root.child(player.clone()).into_any_element();
        }
        if let Some((reader, _)) = &self.reader {
            return root.child(reader.clone()).into_any_element();
        }
        let (ready, logged) = {
            let s = self.state.read(cx);
            (s.session_ready, s.user.is_some())
        };
        if !ready {
            return root.into_any_element();
        }
        if !logged {
            let login = self.login.get_or_insert_with(|| cx.new(LoginView::new)).clone();
            return root
                .child(
                    v_flex()
                        .size_full()
                        .child(gpui_kit::component::TitleBar::new().child(div().font_weight(gpui_kit::FontWeight::BOLD).text_color(cx.theme().primary).child("higanbana")))
                        .child(div().flex_1().min_h_0().child(login)),
                )
                .into_any_element();
        }
        self.login = None;
        root.child(self.render_workspace(window, cx)).into_any_element()
    }
}

//! Leitor de mangá. Os controles moram nas margens (lombada à esquerda, número
//! da página à direita, fio de progresso embaixo); nada flutua sobre a página.

use crate::interact::{hover_layer, radios};
use crate::interact::Button;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use gpui_kit::{
    AnyElement, App, ClickEvent, Context, Div, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, IntoElement,
    KeyDownEvent, Keystroke, ObjectFit, ParentElement, Render, ScrollHandle, SharedString, Stateful, Styled, Subscription,
    Task, Window, AnimationExt as _, assets::IconName, div, img, point, prelude::*, px, relative, rems,
};
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _, TitleBar, h_flex, v_flex,
    button::ButtonVariants as _,
    input::{Input, InputEvent, InputState},
    kbd::Kbd,
    radio::RadioGroup,
    separator::Separator as Divider,
    skeleton::Skeleton,
    spinner::Spinner,
    switch::Switch,
    Selectable as _,
};
use higanbana_core::{
    Core,
    library::join_path,
    reader::{
        PageRef,
        api::{OpenResult, PageInfo},
        logic::{
            ChapterLabel, Direction, ProgressStyle, ReadMode, Turn, Unit, base_unit, build_spreads, chapter_label, comic_files,
            expand_units, key_action, label_text, pages_of_group, sort_pages, spread_of, strip_progress,
        },
        store::{ReaderSettings, TitlePrefs},
    },
};

use super::ReadTarget;
use crate::{
    motion,
    state::{AppState, DlStatus, Img, state},
    theme::ProductColors as _,
    ui::truncate,
};

const MODES: [ReadMode; 4] = [ReadMode::Single, ReadMode::Double, ReadMode::Long, ReadMode::Wide];
/// Páginas carregadas antes de chegar nelas (modos de página).
const PRELOAD: usize = 3;
/// Acima disso o fio vira só linha, sem um estame por capítulo.
const MAX_STAMENS: usize = 120;

pub enum ReaderEvent {
    Close,
}

#[derive(Clone)]
enum Opened {
    Ok(OpenResult),
    Error(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Index,
    Pages,
    Options,
}

/// Um zip aberto: o arquivo, ou o `.cbz` guardado dentro dele.
fn open_key(u: &Unit) -> String {
    match &u.inner {
        Some(i) => format!("{}::{i}", u.path),
        None => u.path.clone(),
    }
}

fn page_key(u: &Unit, pg: usize) -> String {
    format!("page:{}:{pg}", u.key)
}

pub struct ReaderView {
    core: Arc<Core>,
    state: Entity<AppState>,
    target: ReadTarget,
    focus: FocusHandle,
    /// Arquivos com vários capítulos dentro viram várias unidades quando abertos.
    expanded: HashMap<String, Vec<Unit>>,
    opened: HashMap<String, Opened>,
    loading: HashSet<String>,
    cur_key: String,
    /// Página no capítulo atual; `None` = ir para a última quando carregar.
    page: Option<usize>,
    prefs: TitlePrefs,
    settings: ReaderSettings,
    /// Proporção (largura/altura) das páginas já baixadas.
    ratios: HashMap<String, f32>,
    pages_loading: HashSet<String>,
    /// Capítulos na faixa (long/wide strip), em sequência.
    strip: Vec<String>,
    scroll: ScrollHandle,
    pending_scroll: Option<(String, usize)>,
    end: bool,
    panel: Option<Panel>,
    stirred: bool,
    stir_task: Option<Task<()>>,
    filter: Entity<InputState>,
    /// Imagens desta leitura (apagadas ao fechar).
    page_keys: Vec<String>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<ReaderEvent> for ReaderView {}

impl Focusable for ReaderView {
    fn focus_handle(&self, _: &gpui_kit::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ReaderView {
    pub fn new(target: ReadTarget, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        let core = state.read(cx).core.clone();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let data = core.reader().store.data();
        let prefs = data.prefs.get(&target.title_path).copied().unwrap_or_default();
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Buscar capítulo ou volume"));
        let subs = vec![cx.subscribe(&filter, |_, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        })];
        let start = join_path(&target.dir, &target.file.name);
        let mut this = Self {
            core,
            state,
            focus,
            expanded: HashMap::new(),
            opened: HashMap::new(),
            loading: HashSet::new(),
            cur_key: start.clone(),
            page: Some(0),
            prefs,
            settings: data.settings,
            ratios: HashMap::new(),
            pages_loading: HashSet::new(),
            strip: vec![start.clone()],
            scroll: ScrollHandle::new(),
            pending_scroll: None,
            end: false,
            panel: None,
            stirred: false,
            stir_task: None,
            filter,
            page_keys: Vec::new(),
            _subs: subs,
            target,
        };
        let i = this.index_of(&start).unwrap_or(0);
        let resume = this.resume_at(this.chapters().get(i));
        this.go_chapter(i, resume, cx);
        this
    }

    // --- Capítulos ---

    fn chapters(&self) -> Vec<Unit> {
        comic_files(&self.target.files)
            .into_iter()
            .flat_map(|f| {
                let path = join_path(&self.target.dir, &f.name);
                self.expanded.get(&path).cloned().unwrap_or_else(|| vec![base_unit(&path, &f)])
            })
            .collect()
    }

    fn index_of(&self, key: &str) -> Option<usize> {
        self.chapters().iter().position(|u| u.key == key)
    }

    fn chapter(&self) -> usize {
        self.index_of(&self.cur_key).unwrap_or(0)
    }

    fn unit(&self) -> Option<Unit> {
        self.chapters().get(self.chapter()).cloned()
    }

    fn label(&self, i: usize) -> ChapterLabel {
        chapter_label(&self.chapters().get(i).map(|u| u.name.clone()).unwrap_or_default())
    }

    fn list_of(&self, u: &Unit) -> Option<Vec<PageInfo>> {
        let Some(Opened::Ok(r)) = self.opened.get(&open_key(u)) else { return None };
        // Arquivo inteiro que virou vários capítulos: as páginas são dos capítulos, não dele.
        if u.inner.is_none() && u.group.is_none() && self.expanded.contains_key(&u.path) {
            return None;
        }
        Some(sort_pages(&pages_of_group(&r.pages, u.group.as_deref())))
    }

    fn list(&self) -> Vec<PageInfo> {
        self.unit().and_then(|u| self.list_of(&u)).unwrap_or_default()
    }

    fn total(&self) -> usize {
        self.list().len()
    }

    fn page(&self) -> usize {
        self.page.unwrap_or(0)
    }

    fn is_strip(&self) -> bool {
        matches!(self.prefs.mode, ReadMode::Long | ReadMode::Wide)
    }

    /// Onde abrir um capítulo: a página salva, se ainda não terminou.
    fn resume_at(&self, u: Option<&Unit>) -> Option<usize> {
        let p = u.and_then(|u| self.core.reader().store.data().progress.get(&u.key).cloned());
        Some(p.filter(|p| !p.read && p.unit.is_none()).map_or(0, |p| p.page))
    }

    fn ensure(&mut self, u: Option<Unit>, cx: &mut Context<Self>) {
        let Some(u) = u else { return };
        let key = open_key(&u);
        if self.opened.contains_key(&key) || !self.loading.insert(key.clone()) {
            return;
        }
        let core = self.core.clone();
        let (path, size, inner) = (u.path.clone(), u.file.size, u.inner.clone());
        let task = self.core.spawn(async move { core.reader().open(&path, size, inner.as_deref()).await });
        cx.spawn(async move |this, cx| {
            let r = task.await.map_err(|e| e.to_string()).and_then(|r| r);
            let _ = this.update(cx, |v, cx| {
                v.loading.remove(&key);
                match r {
                    Ok(result) => {
                        if u.inner.is_none() && u.group.is_none()
                            && let Some(parts) = expand_units(&u.path, &u.file, &result)
                        {
                            v.expanded.insert(u.path.clone(), parts);
                            v.split_current(&u.path, cx);
                        }
                        v.opened.insert(key, Opened::Ok(result));
                    }
                    Err(e) => {
                        v.opened.insert(key, Opened::Error(e));
                    }
                }
                v.after_change(cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Arquivo que se dividiu em capítulos: troca a unidade inteira pelo capítulo certo.
    fn split_current(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(parts) = self.expanded.get(path).cloned() else { return };
        // Na faixa, o arquivo emendado entra pelo primeiro capítulo.
        for k in self.strip.iter_mut() {
            if k == path {
                *k = parts[0].key.clone();
            }
        }
        if self.cur_key != path {
            return;
        }
        let data = self.core.reader().store.data();
        let (pick, pg) = if self.page.is_none() {
            (parts.last().unwrap().clone(), None)
        } else if let Some(unit) = data.progress.get(path).and_then(|p| p.unit.clone()) {
            let at = parts.iter().position(|u| u.key == unit).unwrap_or(0);
            let done = data.progress.get(&parts[at].key).is_some_and(|p| p.read);
            let pick = parts[if done && at + 1 < parts.len() { at + 1 } else { at }].clone();
            let pg = self.resume_at(Some(&pick));
            (pick, pg)
        } else {
            (parts[0].clone(), Some(0))
        };
        for k in self.strip.iter_mut() {
            if *k == parts[0].key || k == path {
                *k = pick.key.clone();
            }
        }
        self.cur_key = pick.key.clone();
        self.page = pg;
        if self.is_strip() {
            self.pending_scroll = Some((pick.key.clone(), pg.unwrap_or(0)));
        }
        self.ensure(Some(pick), cx);
    }

    fn go_chapter(&mut self, i: usize, pg: Option<usize>, cx: &mut Context<Self>) {
        let Some(u) = self.chapters().get(i).cloned() else { return };
        self.cur_key = u.key.clone();
        self.page = pg;
        self.end = false;
        self.panel = None;
        self.strip = vec![u.key.clone()];
        if self.is_strip() {
            self.pending_scroll = Some((u.key.clone(), pg.unwrap_or(0)));
        }
        self.ensure(Some(u), cx);
        self.after_change(cx);
        cx.notify();
    }

    /// Página "-1" resolvida, pré-carga e progresso salvo.
    fn after_change(&mut self, cx: &mut Context<Self>) {
        let total = self.total();
        if self.page.is_none() && total > 0 {
            self.page = Some(total - 1);
        }
        let Some(u) = self.unit() else { return };
        if total == 0 {
            return;
        }
        let pg = self.page();
        if !self.is_strip() {
            let list = self.list();
            for k in 0..=PRELOAD {
                if let Some(p) = list.get(pg + k) {
                    self.want_page(&u, pg + k, p.index, cx);
                }
            }
            if pg + PRELOAD >= total {
                let next = self.chapters().get(self.chapter() + 1).cloned();
                self.ensure(next, cx);
            }
        }
        // Salva onde parou (na página dupla, conta a página mais adiantada do par).
        let last_visible = if self.prefs.mode == ReadMode::Double { self.spread().last().copied().unwrap_or(pg) } else { pg };
        let store = &self.core.reader().store;
        store.set_progress(&u.key, last_visible.min(total - 1), total);
        if let Some(parts) = self.expanded.get(&u.path) {
            let index = parts.iter().position(|p| p.key == u.key).unwrap_or(0);
            let read = store.data().progress.get(&u.key).is_some_and(|p| p.read);
            store.set_summary(&u.path, index, parts.len(), read, &u.key, &u.name);
        }
    }

    // --- Imagens ---

    fn want_page(&mut self, u: &Unit, pg: usize, index: usize, cx: &mut Context<Self>) {
        let key = page_key(u, pg);
        if self.pages_loading.contains(&key) || self.state.read(cx).cached_image(&key).is_some() {
            return;
        }
        self.pages_loading.insert(key.clone());
        self.page_keys.push(key.clone());
        self.state.update(cx, |s, _| s.mark_image_loading(&key));
        let core = self.core.clone();
        let page = PageRef { size: u.file.size, index, inner: u.inner.clone(), path: u.path.clone() };
        let task = self.core.spawn(async move { core.reader().page(&page).await });
        let state = self.state.clone();
        cx.spawn(async move |this, cx| {
            let r = task.await.ok().and_then(Result::ok);
            let ratio = r.as_ref().and_then(|(b, _)| {
                let (w, h) = image::ImageReader::new(std::io::Cursor::new(b)).with_guessed_format().ok()?.into_dimensions().ok()?;
                (h > 0).then(|| w as f32 / h as f32)
            });
            state.update(cx, |s, cx| match r {
                Some((bytes, _)) => s.put_image(key.clone(), bytes, cx),
                None => s.fail_image(&key, cx),
            });
            let _ = this.update(cx, |v, cx| {
                v.pages_loading.remove(&key);
                if let Some(r) = ratio {
                    v.ratios.insert(key, r);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn retry_page(&mut self, key: &str, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| s.forget_image(key, cx));
        cx.notify();
    }

    // --- Página dupla ---

    fn spreads(&self) -> Vec<Vec<usize>> {
        let Some(u) = self.unit() else { return Vec::new() };
        build_spreads(self.total(), |pg| self.ratios.get(&page_key(&u, pg)).copied().unwrap_or(0.) > 1.)
    }

    fn spread(&self) -> Vec<usize> {
        let s = self.spreads();
        s.get(spread_of(&s, self.page())).cloned().unwrap_or_default()
    }

    // --- Navegação ---

    fn has_next(&self) -> bool {
        self.chapter() + 1 < self.chapters().len()
    }

    fn next(&mut self, cx: &mut Context<Self>) {
        if self.is_strip() {
            return self.scroll_by(1.);
        }
        if self.end {
            if self.has_next() {
                self.go_chapter(self.chapter() + 1, Some(0), cx);
            }
            return;
        }
        let total = self.total();
        let target = if self.prefs.mode == ReadMode::Double {
            let s = self.spreads();
            s.get(spread_of(&s, self.page()) + 1).and_then(|x| x.first().copied()).unwrap_or(total)
        } else {
            self.page() + 1
        };
        if target < total {
            self.page = Some(target);
            self.after_change(cx);
        } else if self.settings.seam && self.has_next() {
            self.go_chapter(self.chapter() + 1, Some(0), cx);
        } else {
            self.end = true;
        }
        cx.notify();
    }

    fn prev(&mut self, cx: &mut Context<Self>) {
        if self.is_strip() {
            return self.scroll_by(-1.);
        }
        if self.end {
            self.end = false;
            cx.notify();
            return;
        }
        let target = if self.prefs.mode == ReadMode::Double {
            let s = self.spreads();
            spread_of(&s, self.page()).checked_sub(1).and_then(|i| s.get(i)).and_then(|x| x.first().copied())
        } else {
            self.page().checked_sub(1)
        };
        match target {
            Some(t) => {
                self.page = Some(t);
                self.after_change(cx);
            }
            None if self.chapter() > 0 => self.go_chapter(self.chapter() - 1, None, cx),
            None => {}
        }
        cx.notify();
    }

    fn scroll_by(&mut self, sign: f32) {
        let b = self.scroll.bounds();
        let o = self.scroll.offset();
        if self.prefs.mode == ReadMode::Long {
            let amount = f32::from(b.size.height) * 0.85 * sign;
            self.scroll.set_offset(point(o.x, o.y - px(amount)));
        } else {
            let amount = f32::from(b.size.width) * 0.85 * sign;
            let amount = if self.prefs.direction == Direction::Rtl { -amount } else { amount };
            self.scroll.set_offset(point(o.x - px(amount), o.y));
        }
    }

    fn set_mode(&mut self, m: ReadMode, cx: &mut Context<Self>) {
        self.prefs.mode = m;
        self.core.reader().store.set_prefs(&self.target.title_path, self.prefs);
        self.strip = vec![self.cur_key.clone()];
        self.end = false;
        if self.is_strip() {
            self.pending_scroll = Some((self.cur_key.clone(), self.page()));
        }
        self.after_change(cx);
        cx.notify();
    }

    fn set_direction(&mut self, d: Direction, cx: &mut Context<Self>) {
        self.prefs.direction = d;
        self.core.reader().store.set_prefs(&self.target.title_path, self.prefs);
        cx.notify();
    }

    fn set_settings(&mut self, f: impl FnOnce(&mut ReaderSettings), cx: &mut Context<Self>) {
        f(&mut self.settings);
        self.core.reader().store.set_settings(self.settings);
        cx.notify();
    }

    fn toggle(&mut self, p: Panel) {
        self.panel = if self.panel == Some(p) { None } else { Some(p) };
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.is_fullscreen() {
            window.toggle_fullscreen();
        }
        // As páginas ficam na memória só enquanto o leitor está aberto.
        let keys = std::mem::take(&mut self.page_keys);
        self.state.update(cx, |s, cx| keys.iter().for_each(|k| s.forget_image(k, cx)));
        cx.emit(ReaderEvent::Close);
    }

    fn stir(&mut self, cx: &mut Context<Self>) {
        if !self.stirred {
            self.stirred = true;
            cx.notify();
        }
        self.stir_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(2500)).await;
            let _ = this.update(cx, |v, cx| {
                v.stirred = false;
                cx.notify();
            });
        }));
    }

    fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let m = &e.keystroke.modifiers;
        if m.control || m.alt || m.platform {
            return;
        }
        let key = e.keystroke.key.as_str();
        if key == "escape" {
            if self.panel.is_some() {
                self.panel = None;
            } else if window.is_fullscreen() {
                window.toggle_fullscreen();
            } else {
                self.close(window, cx);
            }
            cx.notify();
            return;
        }
        if self.panel == Some(Panel::Index) && self.filter.read(cx).focus_handle(cx).is_focused(window) {
            return;
        }
        match key {
            "g" => self.toggle(Panel::Index),
            "d" => {
                let d = if self.prefs.direction == Direction::Rtl { Direction::Ltr } else { Direction::Rtl };
                self.set_direction(d, cx);
            }
            "m" => {
                let i = MODES.iter().position(|m| *m == self.prefs.mode).unwrap_or(0);
                self.set_mode(MODES[(i + 1) % MODES.len()], cx);
            }
            "f" => window.toggle_fullscreen(),
            "[" => {
                if let Some(i) = self.chapter().checked_sub(1) {
                    self.go_chapter(i, Some(0), cx);
                }
            }
            "]" => self.go_chapter(self.chapter() + 1, Some(0), cx),
            "up" | "down" if self.prefs.mode == ReadMode::Long => {
                let o = self.scroll.offset();
                let d = if key == "down" { -80. } else { 80. };
                self.scroll.set_offset(point(o.x, o.y + px(d)));
            }
            _ => match key_action(key, self.prefs.direction) {
                Some(Turn::Next) => self.next(cx),
                Some(Turn::Prev) => self.prev(cx),
                None => return,
            },
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// Página atual da faixa pelo item no topo da área visível; emenda o próximo perto do fim.
    fn track_strip(&mut self, items: &[(String, usize)], cx: &mut Context<Self>) -> usize {
        if let Some((key, pg)) = self.pending_scroll.clone()
            && let Some(ix) = items.iter().position(|(k, p)| *k == key && *p == pg)
        {
            if self.scroll.bounds_for_item(ix).is_some() {
                self.scroll.scroll_to_top_of_item(ix);
                self.pending_scroll = None;
                cx.notify();
            } else {
                // Ainda não desenhado: tenta no próximo quadro.
                cx.notify();
            }
            return ix;
        }
        let center = self.scroll.bounds().center();
        let top = (0..items.len()).find(|&i| self.scroll.bounds_for_item(i).is_some_and(|b| b.contains(&center))).unwrap_or(0);
        if let Some((key, pg)) = items.get(top).cloned()
            && (key != self.cur_key || Some(pg) != self.page)
        {
            self.cur_key = key;
            self.page = Some(pg);
            self.after_change(cx);
            cx.notify();
        }
        let (o, max) = (self.scroll.offset(), self.scroll.max_offset());
        let b = self.scroll.bounds();
        let remaining = if self.prefs.mode == ReadMode::Long {
            f32::from(max.y) + f32::from(o.y)
        } else {
            f32::from(max.x) - f32::from(o.x).abs()
        };
        let span = if self.prefs.mode == ReadMode::Long { f32::from(b.size.height) } else { f32::from(b.size.width) };
        let chapters = self.chapters();
        let last = self.strip.last().and_then(|k| chapters.iter().position(|u| &u.key == k));
        if remaining < span * 2.
            && let Some(last) = last
            && let Some(next) = chapters.get(last + 1).cloned()
        {
            self.ensure(Some(next.clone()), cx);
            if self.settings.seam && self.list_of(&chapters[last]).is_some() && !self.strip.contains(&next.key) {
                self.strip.push(next.key);
                cx.notify();
            }
        }
        top
    }
}


// --- Desenho ---

/// Largura das margens (lombada e número da página), em rem.
const MARGIN_REM: f32 = 5.5;

/// Cartão flutuante dos painéis: mesma superfície dos popovers do kit.
fn panel(id: &'static str, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    v_flex()
        .id(id)
        .occlude()
        .absolute()
        .bg(theme.popover)
        .text_color(theme.popover_foreground)
        .border_1()
        .border_color(theme.border)
        .rounded(theme.radius_lg)
        .shadow_lg()
        .on_click(|_, _, cx| cx.stop_propagation())
}

fn panel_heading(text: &'static str, cx: &App) -> Div {
    div().mb_2().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(cx.theme().foreground).child(text)
}

fn kbd(key: &'static str) -> Kbd {
    Kbd::new(Keystroke::parse(key).expect("atalho válido"))
}

impl ReaderView {
    fn page_image(&self, u: &Unit, pg: usize, fit_height: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let key = page_key(u, pg);
        let found = self.state.read(cx).cached_image(&key);
        let ratio = self.ratios.get(&key).copied().unwrap_or(0.7);
        match found {
            Some(Img::Ready(image)) => {
                let i = img(image).object_fit(ObjectFit::Contain);
                if fit_height { i.h_full().max_w_full().into_any_element() } else { i.w_full().into_any_element() }
            }
            Some(Img::Failed) => {
                let k = key.clone();
                v_flex()
                    .min_h(rems(15.))
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .p_6()
                    .bg(theme.muted)
                    .text_sm()
                    .when(fit_height, |d| d.h_full().w(rems(25.)))
                    .when(!fit_height, |d| d.w_full())
                    .child("Esta página não carregou.")
                    .child(Button::new(SharedString::from(format!("retry:{key}"))).outline().small().label("Tentar de novo").on_click(cx.listener(
                        move |v, _, _, cx| {
                            cx.stop_propagation();
                            v.retry_page(&k, cx);
                        },
                    )))
                    .into_any_element()
            }
            _ => {
                if fit_height {
                    Skeleton::new().h_full().w(rems(25.)).into_any_element()
                } else {
                    // A altura vem da proporção da página, não de uma medida fixa.
                    Skeleton::new().w_full().h(px(800. / ratio)).into_any_element()
                }
            }
        }
    }

    fn strip_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let long = self.prefs.mode == ReadMode::Long;
        let rtl_wide = !long && self.prefs.direction == Direction::Rtl;
        let chapters = self.chapters();
        let strip = self.strip.clone();

        enum Item {
            Page(Box<Unit>, usize, usize),
            Loading,
            End(usize),
            Seam(usize),
        }
        let mut items: Vec<((String, usize), Item)> = Vec::new();
        for key in &strip {
            let Some(ch) = chapters.iter().position(|u| &u.key == key) else { continue };
            let u = chapters[ch].clone();
            let list = self.list_of(&u).unwrap_or_default();
            for (pg, info) in list.iter().enumerate() {
                items.push(((u.key.clone(), pg), Item::Page(Box::new(u.clone()), pg, info.index)));
            }
            if list.is_empty() {
                items.push(((u.key.clone(), 0), Item::Loading));
            }
            let last = (u.key.clone(), list.len().saturating_sub(1));
            if !self.settings.seam || ch + 1 >= chapters.len() {
                items.push((last, Item::End(ch)));
            } else if strip.contains(&chapters[ch + 1].key) {
                items.push((last, Item::Seam(ch)));
            }
        }
        // Na wide strip da direita para a esquerda, a primeira página fica à direita.
        if rtl_wide {
            items.reverse();
        }
        let keys: Vec<(String, usize)> = items.iter().map(|(k, _)| k.clone()).collect();
        let center = self.track_strip(&keys, cx);

        let mut children: Vec<AnyElement> = Vec::new();
        for (ix, (_, item)) in items.into_iter().enumerate() {
            let el = match item {
                Item::Page(u, pg, index) => {
                    // Só baixa o que está perto da área visível.
                    if ix.abs_diff(center) <= 6 {
                        self.want_page(&u, pg, index, cx);
                    }
                    let el = self.page_image(&u, pg, !long, cx);
                    if long { div().w(rems(50.)).max_w_full().flex_none().child(el).into_any_element() } else { div().h_full().flex_none().child(el).into_any_element() }
                }
                Item::Loading => div().p_10().child(Spinner::new()).into_any_element(),
                Item::End(ch) => self.chapter_end(ch, cx),
                Item::Seam(ch) => div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .map(|d| if long { d.py_10() } else { d.w(rems(10.)).h_full() })
                    .child(label_text(&self.label(ch + 1)))
                    .into_any_element(),
            };
            children.push(el);
        }
        let container = div().id("strip").size_full().track_scroll(&self.scroll);
        if long {
            container.overflow_y_scroll().flex().flex_col().items_center().children(children).into_any_element()
        } else {
            container.overflow_x_scroll().flex().h_full().children(children).into_any_element()
        }
    }

    /// Cartão de fim de arquivo, com o próximo (ou o fim da pasta).
    fn chapter_end(&self, ch: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let next = ch + 1;
        let has = next < self.chapters().len();
        let mut card = v_flex()
            .items_center()
            .justify_center()
            .gap_4()
            .p_10()
            .when(self.prefs.mode == ReadMode::Wide, |d| d.h_full().w(rems(20.)).flex_none())
            .child(div().text_sm().text_color(theme.muted_foreground).child(format!("Fim de {}", label_text(&self.label(ch)))));
        if has {
            let label = label_text(&self.label(next));
            card = card.child(div().text_3xl().font_weight(FontWeight::BOLD).child(label.clone())).child(
                h_flex()
                    .gap_2()
                    .child(Button::new(SharedString::from(format!("read-next:{ch}"))).primary().label(format!("Ler {label}")).on_click(cx.listener(
                        move |v, _, _, cx| {
                            cx.stop_propagation();
                            let resume = v.resume_at(v.chapters().get(next));
                            v.go_chapter(next, resume, cx);
                        },
                    )))
                    .when(self.end, |d| {
                        d.child(Button::new("stay").outline().label("Ficar aqui").on_click(cx.listener(|v, _, _, cx| {
                            cx.stop_propagation();
                            v.end = false;
                            cx.notify();
                        })))
                    }),
            );
        } else {
            card = card.child(div().text_xl().font_weight(FontWeight::BOLD).child("Último arquivo da pasta.")).child(
                Button::new(SharedString::from(format!("back-title:{ch}"))).primary().label("Voltar ao título").on_click(cx.listener(|v, _, window, cx| v.close(window, cx))),
            );
        }
        card.into_any_element()
    }

    /// Fio de progresso: uma linha pontilhada com um estame por capítulo.
    fn thread(&self, cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let chapters = self.chapters();
        let count = chapters.len().max(1);
        let current = self.chapter();
        let rtl = self.prefs.direction == Direction::Rtl;
        let fraction = strip_progress(current, chapters.len(), self.page(), self.total()) as f32;
        let progress = self.core.reader().store.data().progress;
        let side = |d: Div, at: gpui_kit::DefiniteLength| if rtl { d.right(at) } else { d.left(at) };
        let mut t = div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .h(rems(2.5))
            .child(div().absolute().left_0().right_0().bottom(rems(0.875)).border_t_1().border_dashed().border_color(theme.border))
            .child(side(div().absolute().bottom(rems(0.8)).h(rems(0.125)).bg(theme.primary).w(relative(fraction)), relative(0.)));
        if chapters.len() <= MAX_STAMENS {
            for (i, u) in chapters.iter().enumerate() {
                let at = relative((i as f32 + 0.5) / count as f32);
                let read = progress.get(&u.key).is_some_and(|p| p.read);
                let (h, color) = if i == current {
                    (1.25, theme.primary_ink())
                } else if read {
                    (0.75, theme.primary)
                } else {
                    (0.75, theme.muted_foreground.opacity(0.5))
                };
                let dot = if i == current { 0.45 } else { 0.3 };
                t = t.child(side(div().absolute().bottom(rems(0.875)).w(px(1.)).h(rems(h)).bg(color), at)).child(side(
                    div().absolute().bottom(rems(0.875 + h - 0.2)).ml(rems(-0.125)).size(rems(dot)).rounded_full().bg(color),
                    at,
                ));
            }
        }
        t
    }

    fn index_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = cx.theme().clone();
        let q = self.filter.read(cx).value().trim().to_lowercase();
        let chapters = self.chapters();
        let progress = self.core.reader().store.data().progress;
        let current = self.chapter();
        let kinds: HashSet<&str> = chapters.iter().map(|u| chapter_label(&u.name)).filter(|l| l.number.is_some()).map(|l| l.kind).collect::<Vec<_>>().into_iter().collect();
        let n = chapters.len();
        let word = match kinds.iter().next().copied().filter(|_| kinds.len() == 1) {
            Some("Vol.") => if n == 1 { "volume" } else { "volumes" },
            Some("Cap.") => if n == 1 { "capítulo" } else { "capítulos" },
            _ => if n == 1 { "arquivo" } else { "arquivos" },
        };
        let items: Vec<(usize, Unit)> = chapters
            .into_iter()
            .enumerate()
            .filter(|(_, u)| q.is_empty() || u.name.to_lowercase().contains(&q) || label_text(&chapter_label(&u.name)).to_lowercase().contains(&q))
            .collect();
        panel("index-panel", cx)
            .bottom(rems(3.))
            .left_4()
            .w(rems(19.))
            .max_h(rems(32.))
            .py_3()
            .child(h_flex().mx_4().mb_2().items_baseline().justify_between().child(panel_heading("Índice", cx).mb_0()).child(div().text_xs().text_color(theme.muted_foreground).child(format!("{n} {word}"))))
            .child(div().mx_4().mb_2().child(Input::new(&self.filter).small().prefix(Icon::new(IconName::Search).small()).cleanable(true)))
            .child(
                v_flex()
                    .id("index-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_2()
                    .when(items.is_empty(), |d| d.child(div().px_2().py_6().text_sm().text_color(theme.muted_foreground).child("Nada com esse nome.")))
                    .children(items.into_iter().map(|(i, u)| {
                        let l = chapter_label(&u.name);
                        let here = i == current;
                        let read = progress.get(&u.key).is_some_and(|p| p.read);
                        h_flex()
                            .id(SharedString::from(format!("chapter:{}", u.key)))
                            .gap_2()
                            .px_2()
                            .py_1p5()
                            .rounded(theme.radius)
                            .relative()
                            .cursor_pointer()
                            .map(|d| if here { d.bg(theme.list_active) } else { d.child(hover_layer("hover").fill(theme.list_hover).radius(theme.radius)) })
                            .child(match &l.number {
                                Some(num) => h_flex()
                                    .items_baseline()
                                    .gap_1p5()
                                    .flex_1()
                                    .child(div().text_xs().text_color(theme.muted_foreground).child(l.kind))
                                    .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(if here { theme.primary_ink() } else { theme.foreground }).child(num.clone())),
                                None => truncate(l.name.clone().unwrap_or_default()).flex_1().text_sm(),
                            })
                            .when(read, |d| d.child(h_flex().gap_1().text_xs().text_color(theme.muted_foreground).child(Icon::new(IconName::Check).xsmall()).child("Lido")))
                            .on_click(cx.listener(move |v, _, _, cx| {
                                let resume = v.resume_at(v.chapters().get(i));
                                v.go_chapter(i, resume, cx);
                            }))
                    })),
            )
    }

    fn pages_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let total = self.total();
        let current = self.page();
        panel("pages-panel", cx)
            .top_4()
            .right_4()
            .w(rems(20.))
            .max_h(rems(30.))
            .overflow_y_scroll()
            .p_4()
            .child(panel_heading("Ir para a página", cx))
            .child(div().grid().grid_cols(6).gap_1().children((0..total).map(|pg| {
                Button::new(SharedString::from(format!("page:{pg}")))
                    .ghost()
                    .small()
                    .selected(pg == current)
                    .label((pg + 1).to_string())
                    .on_click(cx.listener(move |v, _, _, cx| {
                        v.panel = None;
                        if v.is_strip() {
                            v.strip = vec![v.cur_key.clone()];
                            v.pending_scroll = Some((v.cur_key.clone(), pg));
                        } else {
                            v.page = Some(pg);
                            v.after_change(cx);
                        }
                        v.end = false;
                        cx.notify();
                    }))
            })))
    }

    fn options_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = cx.theme().clone();
        let mode_label = |m: ReadMode| match m {
            ReadMode::Single => (IconName::RectangleVertical, "Página única"),
            ReadMode::Double => (IconName::Columns2, "Página dupla"),
            ReadMode::Long => (IconName::ArrowDown, "Long strip"),
            ReadMode::Wide => (IconName::ArrowRight, "Wide strip"),
        };
        const STYLES: [(ProgressStyle, &str, &str); 3] = [
            (ProgressStyle::Thread, "Fio", "Linha com um estame em cada capítulo."),
            (ProgressStyle::Number, "Só o número", "Número da página na margem, sem linha."),
            (ProgressStyle::Hidden, "Escondido", "Leitura limpa. Aparece ao mover o mouse."),
        ];
        let style_ix = STYLES.iter().position(|(p, _, _)| *p == self.settings.progress).unwrap_or(0);
        let dir_ix = if self.prefs.direction == Direction::Rtl { 0 } else { 1 };
        let hint = |text: &'static str| div().mt_2().text_xs().text_color(theme.muted_foreground).child(text);
        panel("options-panel", cx)
            .top_4()
            .right_4()
            .w(rems(47.5))
            .p_6()
            .gap_6()
            .child(
                h_flex()
                    .items_start()
                    .gap_6()
                    .child(
                        v_flex()
                            .flex_1()
                            .child(panel_heading("Modo", cx))
                            .child(h_flex().gap_2().children(MODES.map(|m| {
                                let (ic, label) = mode_label(m);
                                Button::new(SharedString::from(format!("mode:{label}")))
                                    .outline()
                                    .small()
                                    .icon(ic)
                                    .label(label)
                                    .selected(self.prefs.mode == m)
                                    .on_click(cx.listener(move |v, _, _, cx| v.set_mode(m, cx)))
                            })))
                            .child(
                                v_flex().mt_5().gap_1().child(Switch::new("seam").cursor_pointer().label("Emendar capítulos").checked(self.settings.seam).on_change(cx.listener(
                                    |v, on: &bool, _, cx| {
                                        let on = *on;
                                        v.set_settings(|s| s.seam = on, cx);
                                    },
                                ))).child(div().pl_7().text_xs().text_color(theme.muted_foreground).child("Ao terminar, o próximo vem logo em seguida, sem sair do leitor.")),
                            ),
                    )
                    .child(
                        v_flex()
                            .w(rems(12.5))
                            .child(panel_heading("Direção", cx))
                            .child(RadioGroup::vertical("direction").children(radios(["Direita → esquerda", "Esquerda → direita"])).selected_index(Some(dir_ix)).on_change(cx.listener(
                                |v, ix: &usize, _, cx| v.set_direction(if *ix == 0 { Direction::Rtl } else { Direction::Ltr }, cx),
                            )))
                            .child(hint("Lembrada para este título. Títulos novos começam da direita para a esquerda.")),
                    )
                    .child(
                        v_flex()
                            .w(rems(13.75))
                            .child(panel_heading("Progresso", cx))
                            .child(RadioGroup::vertical("progress-style").children(radios(STYLES.map(|(_, label, _)| label))).selected_index(Some(style_ix)).on_change(cx.listener(
                                |v, ix: &usize, _, cx| {
                                    let p = STYLES[*ix].0;
                                    v.set_settings(|s| s.progress = p, cx);
                                },
                            )))
                            .child(hint(STYLES[style_ix].2)),
                    ),
            )
            .child(Divider::horizontal())
            .child(h_flex().flex_wrap().gap_x_5().gap_y_2().text_xs().text_color(theme.muted_foreground).children(
                [
                    (vec!["left", "right"], "virar a página"),
                    (vec!["[", "]"], "capítulo anterior / próximo"),
                    (vec!["g"], "índice"),
                    (vec!["d"], "trocar direção"),
                    (vec!["m"], "trocar modo"),
                    (vec!["f"], "tela cheia"),
                    (vec!["escape"], "fechar"),
                ]
                .into_iter()
                .map(|(keys, what)| h_flex().gap_1p5().children(keys.into_iter().map(kbd)).child(what)),
            ))
    }
}

impl Render for ReaderView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let margin = rems(MARGIN_REM);
        let chapter = self.chapter();
        let label = self.label(chapter);
        let total = self.total();
        let unit = self.unit();
        let current = unit.as_ref().and_then(|u| self.opened.get(&open_key(u)).cloned());
        let rtl = self.prefs.direction == Direction::Rtl;
        let page_text = if total == 0 {
            "—".to_owned()
        } else {
            let s = if self.prefs.mode == ReadMode::Double { self.spread() } else { vec![self.page()] };
            s.iter().map(|p| format!("{:02}", p + 1)).collect::<Vec<_>>().join("–")
        };
        let show_number = self.settings.progress != ProgressStyle::Hidden || self.stirred;
        let show_thread = self.settings.progress == ProgressStyle::Thread || (self.settings.progress == ProgressStyle::Hidden && self.stirred);
        let fullscreen = window.is_fullscreen();
        let num = label.number.clone().unwrap_or_else(|| (chapter + 1).to_string());

        let body: AnyElement = match (&current, total) {
            (Some(Opened::Error(msg)), _) => {
                let msg = msg.clone();
                let download = match unit.as_ref().and_then(|u| self.state.read(cx).download(&u.path)).map(|d| d.status) {
                    Some(DlStatus::Done) => "Baixado",
                    Some(DlStatus::Running | DlStatus::Queued) => "Baixando…",
                    _ => "Baixar",
                };
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .child(
                        v_flex()
                            .max_w(rems(24.))
                            .items_center()
                            .gap_3()
                            .child(div().text_lg().font_weight(FontWeight::BOLD).child("Não deu para abrir este arquivo."))
                            .child(div().text_sm().text_color(theme.muted_foreground).child(msg))
                            .child(
                                h_flex()
                                    .mt_2()
                                    .gap_2()
                                    .child(Button::new("retry-open").primary().label("Tentar de novo").on_click(cx.listener(|v, _, _, cx| {
                                        cx.stop_propagation();
                                        if let Some(u) = v.unit() {
                                            v.opened.remove(&open_key(&u));
                                            v.ensure(Some(u), cx);
                                        }
                                    })))
                                    .child(Button::new("download-file").outline().icon(IconName::Download).label(download).on_click(cx.listener(|v, _, _, cx| {
                                        cx.stop_propagation();
                                        if let Some(u) = v.unit() {
                                            v.state.update(cx, |s, cx| s.start_downloads(std::slice::from_ref(&u.path), cx));
                                        }
                                    }))),
                            ),
                    )
                    .into_any_element()
            }
            (_, 0) => div().size_full().flex().items_center().justify_center().child(Skeleton::new().w(rems(23.75)).h(relative(0.78))).into_any_element(),
            _ if self.is_strip() => self.strip_view(cx),
            _ => {
                let u = unit.clone().unwrap();
                let pages: Vec<usize> = if self.prefs.mode == ReadMode::Double { self.spread() } else { vec![self.page()] };
                let mut row = div().size_full().flex().items_center().justify_center().px_4().pt_4().pb_11();
                if rtl && pages.len() > 1 {
                    row = row.flex_row_reverse();
                }
                let half = pages.len() > 1;
                row.children(pages.into_iter().map(|pg| {
                    div()
                        .h_full()
                        .flex()
                        .items_center()
                        .when(half, |d| d.max_w(relative(0.5)))
                        .child(self.page_image(&u, pg, true, cx))
                }))
                .into_any_element()
            }
        };

        let spine_title: Vec<char> = self.target.title.chars().take(28).collect();
        let kind = if label.kind.is_empty() { "ARQ." } else { label.kind };
        v_flex()
            .id("reader")
            .track_focus(&self.focus)
            .key_context("Reader")
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_move(cx.listener(|v, _, _, cx| v.stir(cx)))
            .when(!fullscreen, |d| d.child(TitleBar::new().child(div().pl_3().text_sm().text_color(theme.muted_foreground).child(self.target.title.clone()))))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    // Lombada: voltar, título vertical, número do capítulo
                    .child(
                        v_flex()
                            .w(margin)
                            .flex_none()
                            .border_r_1()
                            .border_color(theme.border)
                            .bg(theme.group_box)
                            .pt_5()
                            .pb_11()
                            .items_center()
                            .gap_5()
                            .child(Button::new("back").outline().icon(IconName::ChevronLeft).tooltip("Voltar (Esc)").on_click(cx.listener(|v, _, window, cx| v.close(window, cx))))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_hidden()
                                    .items_center()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .line_height(relative(1.15))
                                    .children(spine_title.into_iter().map(|c| div().child(c.to_string()))),
                            )
                            .child(
                                v_flex()
                                    .id("chapter-number")
                                    .items_center()
                                    .px_1p5()
                                    .py_1()
                                    .rounded(theme.radius)
                                    .relative()
                                    .cursor_pointer()
                                    .child(hover_layer("hover").fill(theme.list_hover).radius(theme.radius))
                                    .child(div().text_xs().text_color(theme.muted_foreground).child(kind.to_uppercase()))
                                    .child(
                                        div()
                                            .text_size(rems(if num.len() > 3 { 1.875 } else { 2.75 }))
                                            .line_height(relative(1.))
                                            .font_weight(FontWeight::BLACK)
                                            .text_color(if self.panel == Some(Panel::Index) { theme.primary } else { theme.primary_ink() })
                                            .child(num),
                                    )
                                    .on_click(cx.listener(|v, _, _, cx| {
                                        v.toggle(Panel::Index);
                                        cx.notify();
                                    })),
                            ),
                    )
                    // Página
                    .child(
                        div()
                            .id("page-area")
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .on_click(cx.listener(move |v, e: &ClickEvent, window, cx| {
                                if v.panel.is_some() {
                                    v.panel = None;
                                    cx.notify();
                                    return;
                                }
                                if e.click_count() == 2 {
                                    window.toggle_fullscreen();
                                    return;
                                }
                                if v.is_strip() {
                                    return;
                                }
                                // Clique nas laterais vira a página (respeitando a direção); no meio, nada.
                                let edge = MARGIN_REM * f32::from(window.rem_size());
                                let w = f32::from(window.viewport_size().width);
                                let x = (f32::from(e.position().x) - edge) / (w - edge * 2.).max(1.);
                                let rtl = v.prefs.direction == Direction::Rtl;
                                if x < 0.4 {
                                    if rtl { v.next(cx) } else { v.prev(cx) }
                                } else if x > 0.6 {
                                    if rtl { v.prev(cx) } else { v.next(cx) }
                                }
                            }))
                            .child(body)
                            .when(self.end && !self.is_strip(), |d| {
                                d.child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .bg(theme.background.opacity(0.85))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(self.chapter_end(chapter, cx))
                                        .with_animation(SharedString::from(format!("end-in:{chapter}")), motion::enter(motion::BASE), |e, t| e.opacity(t)),
                                )
                            })
                            .when(show_thread, |d| d.child(self.thread(cx)))
                            // Painéis aparecem com fade curto.
                            .when(self.panel == Some(Panel::Index), |d| {
                                d.child(self.index_panel(cx).with_animation("index-in", motion::enter(motion::QUICK), |e, t| e.opacity(t)))
                            })
                            .when(self.panel == Some(Panel::Pages) && total > 0, |d| {
                                d.child(self.pages_panel(cx).with_animation("pages-in", motion::enter(motion::QUICK), |e, t| e.opacity(t)))
                            })
                            .when(self.panel == Some(Panel::Options), |d| {
                                d.child(self.options_panel(cx).with_animation("options-in", motion::enter(motion::QUICK), |e, t| e.opacity(t)))
                            }),
                    )
                    // Margem direita: número da página, opções, direção
                    .child(
                        v_flex()
                            .w(margin)
                            .flex_none()
                            .border_l_1()
                            .border_color(theme.border)
                            .bg(theme.group_box)
                            .pt_5()
                            .pb_11()
                            .items_center()
                            .gap_3p5()
                            .child(
                                v_flex()
                                    .id("page-number")
                                    .items_center()
                                    .px_1()
                                    .rounded(theme.radius)
                                    .relative()
                                    .cursor_pointer()
                                    .child(hover_layer("hover").fill(theme.list_hover).radius(theme.radius))
                                    .when(!show_number, |d| d.opacity(0.))
                                    .child(div().text_size(rems(if page_text.len() > 3 { 1.375 } else { 2.5 })).line_height(relative(1.)).font_weight(FontWeight::BLACK).child(page_text))
                                    .child(div().mt_1().text_xs().text_color(theme.muted_foreground).child(format!("de {}", if total > 0 { total.to_string() } else { "—".into() })))
                                    .on_click(cx.listener(|v, _, _, cx| {
                                        v.toggle(Panel::Pages);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("options")
                                    .outline()
                                    .icon(IconName::SlidersHorizontal)
                                    .selected(self.panel == Some(Panel::Options))
                                    .tooltip("Opções de leitura")
                                    .on_click(cx.listener(|v, _, _, cx| {
                                        v.toggle(Panel::Options);
                                        cx.notify();
                                    })),
                            )
                            .child(div().flex_1())
                            .child(
                                Button::new("direction")
                                    .outline()
                                    .icon(if rtl { IconName::ArrowLeft } else { IconName::ArrowRight })
                                    .tooltip(if rtl { "Lendo da direita para a esquerda (D)" } else { "Lendo da esquerda para a direita (D)" })
                                    .on_click(cx.listener(|v, _, _, cx| {
                                        let d = if v.prefs.direction == Direction::Rtl { Direction::Ltr } else { Direction::Rtl };
                                        v.set_direction(d, cx);
                                    })),
                            ),
                    ),
            )
    }
}

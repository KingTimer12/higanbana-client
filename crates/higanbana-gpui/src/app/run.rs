//! Laço principal: bloqueia esperando evento (ou até o próximo prazo de timer,
//! animação ou piscar do cursor); reconstrói/relayout/repinta só se sujo.
use super::measurer::GpuMeasurer;
use super::{overlay, App, View};
use crate::anim::Animator;
use crate::cx::{Inbox, RenderCx, Tasks, Timers};
use crate::error::Result;
use crate::geometry::Size;
use crate::gfx::{enumerate, BackendInfo, Context, DrawList, FontSpec, Window, WindowOptions, WindowState, HitRegion};
use crate::paint::{GpuCanvas, ImageCache};
use crate::style::TextStyle;
use crate::theme::Theme;
use crate::tree::Tree;
use std::time::Instant;

/// Quanto trabalho o próximo frame precisa (ordem crescente de custo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Dirty {
    Clean,
    Repaint,
    Relayout,
    Rebuild,
}

pub(super) struct Runner<'c, V> {
    pub ctx: &'c Context,
    pub win: Window<'c>,
    pub view: V,
    pub tree: Tree<V>,
    images: ImageCache<'c>,
    pool: Option<DrawList<'static>>,
    pub theme: Theme,
    backends: Vec<BackendInfo>,
    pub scale: f32,
    pub viewport: Size,
    pub dirty: Dirty,
    show_stats: bool,
    pub vsync: bool,
    animator: Animator,
    pub timers: Timers,
    /// Tarefas em segundo plano aguardando resultado.
    pub tasks: Tasks,
    /// Mensagens de outras threads (`App::mailbox`).
    pub inbox: Option<Inbox<V>>,
    /// Rolagens pedidas por handlers, aplicadas depois do layout do frame.
    pub scrolls: Vec<crate::cx::ScrollRequest>,
    /// Reconstrução pedida para um instante (animação, `request_redraw_in`).
    pub wake: Option<Instant>,
    /// Próxima troca do cursor piscando (só repinta).
    pub blink: Option<Instant>,
    /// Maximizada/fullscreen/foco (para a `TitleBar`).
    pub window_state: WindowState,
    /// Últimas regiões publicadas (só republica se mudarem).
    pub regions: Vec<HitRegion>,
}

pub(super) fn run<V: View>(app: App, options: WindowOptions, view: V) -> Result<()> {
    let ctx = Context::new(app.backend)?;
    let win = Window::new(&ctx, &options)?;
    if let Some(f) = app.on_window {
        f(win.native_handle());
    }
    let mut inbox = app.inbox.and_then(|b| b.downcast::<Inbox<V>>().ok()).map(|b| *b);
    if let Some(i) = inbox.as_mut() {
        i.attach(win.waker());
    }
    let mut r = Runner {
        ctx: &ctx,
        scale: win.scale(),
        viewport: win.logical_size(),
        win,
        view,
        tree: Tree::new(),
        images: ImageCache::new(&ctx),
        pool: Some(DrawList::new()),
        theme: app.theme,
        backends: enumerate(),
        dirty: Dirty::Rebuild,
        show_stats: app.show_stats,
        vsync: app.vsync,
        animator: Animator::default(),
        timers: Timers::default(),
        tasks: Tasks::default(),
        inbox,
        scrolls: Vec::new(),
        wake: None,
        blink: None,
        window_state: WindowState::default(),
        regions: Vec::new(),
    };
    r.window_state = r.win.state();
    loop {
        r.frame();
        let mut next = match r.deadline() {
            None => Some(r.win.wait_event()?),
            Some(t) => r.win.wait_event_timeout(t.saturating_duration_since(Instant::now()))?,
        };
        while let Some(ev) = next {
            if !r.handle(ev) {
                return Ok(());
            }
            next = r.win.poll_event();
        }
        if !r.tick(Instant::now()) {
            return Ok(());
        }
    }
}

impl<V: View> Runner<'_, V> {
    pub fn mark(&mut self, d: Dirty) {
        self.dirty = self.dirty.max(d);
    }

    fn base_text(&self) -> TextStyle {
        let t = &self.theme.typography;
        TextStyle { font: FontSpec::new(t.family, 400, false), size: t.sm, color: self.theme.foreground, wrap: true }
    }

    /// Chama `View::render` com foco e animações; agenda o próximo frame se algo anima.
    fn render(&mut self, now: Instant, measurer: &mut GpuMeasurer<'_>) {
        let base = self.base_text();
        self.animator.begin();
        let mut cx = RenderCx::new(&self.theme, self.ctx.backend(), &self.backends, self.ctx.stats(), &mut self.animator)
            .with_focus(self.tree.focus_info());
        cx.now = now;
        cx.window = self.window_state;
        cx.viewport = self.viewport;
        cx.waker = self.win.waker();
        cx.scroll = self.tree.scroll_states();
        let root = self.view.render(&mut cx);
        let wake = cx.wake;
        let waker = cx.waker.clone();
        for req in std::mem::take(&mut cx.tasks) {
            self.tasks.submit(req, &waker);
        }
        self.tree.rebuild(root, &base, self.viewport, measurer);
        let active = self.animator.is_active();
        self.animator.end();
        if let Some(w) = wake {
            self.wake = Some(self.wake.map_or(w, |x| x.min(w)));
        }
        if active {
            self.request_frame(now);
        }
    }

    /// Publica as regiões da barra customizada se mudaram desde o último layout.
    fn publish_regions(&mut self) {
        let mut fresh = Vec::with_capacity(self.regions.len());
        self.tree.hit_regions(&mut fresh);
        if fresh != self.regions {
            self.win.set_hit_regions(&fresh);
            self.regions = fresh;
        }
    }

    /// Faz o mínimo necessário para o estado sujo e apresenta.
    fn frame(&mut self) {
        if self.dirty == Dirty::Clean || self.viewport.width < 1.0 || self.viewport.height < 1.0 {
            return;
        }
        let now = Instant::now();
        let mut measurer = GpuMeasurer { ctx: self.ctx, scale: self.scale };
        match self.dirty {
            Dirty::Rebuild => self.render(now, &mut measurer),
            Dirty::Relayout => self.tree.relayout(self.viewport, &mut measurer),
            _ => {}
        }
        for r in std::mem::take(&mut self.scrolls) {
            self.tree.apply_scroll(r);
        }
        if self.dirty != Dirty::Repaint {
            self.publish_regions();
        }
        self.dirty = Dirty::Clean;
        self.blink = self.tree.next_blink(now);
        self.tree.resolve_icons(self.scale);
        self.images.prepare(&self.tree);
        let mut list = self.pool.take().unwrap_or_default().recycle();
        {
            let mut canvas = GpuCanvas::new(&mut list, self.ctx, &self.images, self.scale);
            self.tree.paint_at(&mut canvas, now);
            if self.show_stats {
                overlay::paint(&mut canvas, self.ctx, &self.theme, self.viewport);
            }
            if let Some(e) = canvas.error {
                eprintln!("higanbana-gpui: frame: {e}");
            }
        }
        if let Err(e) = self.win.present(self.theme.background, &list, self.vsync) {
            eprintln!("higanbana-gpui: present: {e}");
        }
        self.pool = Some(list.recycle());
        crate::paint::cache::end_frame();
    }
}

//! Laço principal: bloqueia esperando evento; reconstrói/relayout/repinta só se sujo.
use super::measurer::GpuMeasurer;
use super::{overlay, App, View};
use crate::cx::RenderCx;
use crate::error::Result;
use crate::geometry::Size;
use crate::gfx::{enumerate, BackendInfo, Context, DrawList, FontSpec, Window, WindowOptions};
use crate::paint::{GpuCanvas, ImageCache};
use crate::style::TextStyle;
use crate::theme::Theme;
use crate::tree::Tree;

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
    vsync: bool,
}

pub(super) fn run<V: View>(app: App, options: WindowOptions, view: V) -> Result<()> {
    let ctx = Context::new(app.backend)?;
    let win = Window::new(&ctx, &options)?;
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
    };
    loop {
        r.frame();
        let mut next = Some(r.win.wait_event()?);
        while let Some(ev) = next {
            if !r.handle(ev) {
                return Ok(());
            }
            next = r.win.poll_event();
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

    /// Faz o mínimo necessário para o estado sujo e apresenta.
    fn frame(&mut self) {
        if self.dirty == Dirty::Clean || self.viewport.width < 1.0 || self.viewport.height < 1.0 {
            return;
        }
        let mut measurer = GpuMeasurer { ctx: self.ctx, scale: self.scale };
        match self.dirty {
            Dirty::Rebuild => {
                let mut cx = RenderCx::new(&self.theme, self.ctx.backend(), &self.backends, self.ctx.stats());
                let root = self.view.render(&mut cx);
                let base = self.base_text();
                self.tree.rebuild(root, &base, self.viewport, &mut measurer);
            }
            Dirty::Relayout => self.tree.relayout(self.viewport, &mut measurer),
            _ => {}
        }
        self.dirty = Dirty::Clean;
        self.images.prepare(&self.tree);
        let mut list = self.pool.take().unwrap_or_default().recycle();
        {
            let mut canvas = GpuCanvas::new(&mut list, self.ctx, &self.images, self.scale);
            self.tree.paint(&mut canvas);
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
    }
}

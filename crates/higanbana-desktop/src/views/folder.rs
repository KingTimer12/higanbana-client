//! Pasta comum. Só pastas de nome curto (ex.: `Letra A`, `0-9`) viram índice em
//! blocos; pasta com músicas vira encarte; o resto é uma lista virtual de arquivos
//! (`VirtualList` do kit), que só desenha as linhas visíveis — pastas da nuvem
//! como as de revistas têm centenas de arquivos.
//!
//! A tela rola sozinha (não a da área de trabalho): a lista é a região rolável e
//! o cabeçalho fica fixo acima dela.

use crate::interact::hover_layer;
use std::rc::Rc;

use gpui_kit::{
    Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Pixels, Render, SharedString, Size, Styled,
    Subscription, Window, assets::IconName, div, prelude::*, px,
};
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{ActiveTheme as _, v_flex, v_virtual_list};
use higanbana_core::{
    library::{natural_cmp, split_path},
    nuvem::FileEntry,
};

use crate::{
    components::{FileAction, FileRows, breadcrumbs_in, columns, content_width, empty_folder, file_list_frame, folder_link, section_label},
    music::album::{AlbumEvent, AlbumView, looks_like_album},
    reader::ReadTarget,
    state::{AppState, state},
    ui::empty_state,
    video::PlayTarget,
    views::load_listing,
};

/// Como a pasta aparece, decidido uma vez quando a listagem chega.
enum Body {
    /// Pastas de nome curto (letras): blocos.
    Index(Vec<SharedString>),
    /// Lista de arquivos e subpastas.
    Files(Rc<FileRows>),
}

pub struct FolderView {
    state: Entity<AppState>,
    path: String,
    /// Aberta crua (`raw:`): as subpastas e migalhas continuam cruas.
    raw: bool,
    listing: Option<Result<Vec<FileEntry>, String>>,
    body: Option<Body>,
    /// Alturas das linhas da lista virtual, pelo tamanho do `rem` em que foram medidas.
    heights: Option<(Pixels, Rc<Vec<Size<Pixels>>>)>,
    album: Option<Entity<AlbumView>>,
    /// O encarte avisou que não era álbum: mostra a lista.
    not_album: bool,
    _subs: Vec<Subscription>,
}

impl FolderView {
    pub fn new(path: String, raw: bool, cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        let subs = vec![cx.observe(&state, |_, _, cx| cx.notify())];
        load_listing(&state, path.clone(), cx, |v: &mut Self, r, cx| {
            v.listing = Some(r);
            v.build_body();
            v.ensure_album(cx);
        });
        Self { state, path, raw, listing: None, body: None, heights: None, album: None, not_album: false, _subs: subs }
    }

    /// Monta o corpo uma vez por listagem: nada de copiar a pasta a cada quadro.
    fn build_body(&mut self) {
        let Some(Ok(files)) = &self.listing else { return };
        let dirs: Vec<&FileEntry> = files.iter().filter(|f| f.is_directory).collect();
        let is_index = dirs.len() >= 6 && dirs.len() == files.len() && dirs.iter().all(|d| d.name.chars().count() <= 16);
        self.body = Some(if is_index {
            let mut names: Vec<&str> = dirs.iter().map(|d| d.name.as_str()).collect();
            names.sort_by(|a, b| natural_cmp(a, b));
            Body::Index(names.into_iter().map(|n| SharedString::from(n.to_owned())).collect())
        } else {
            Body::Files(Rc::new(self.file_rows(files)))
        });
        self.heights = None;
    }

    fn file_rows(&self, files: &[FileEntry]) -> FileRows {
        let (dir, all) = (self.path.clone(), Rc::new(files.to_vec()));
        let st = self.state.clone();
        let anime_title = self.anime_title();
        let on_play: FileAction = Rc::new({
            let (dir, all, st) = (dir.clone(), all.clone(), st.clone());
            move |f, _, cx| {
                let t = PlayTarget { dir: dir.clone(), file: f.clone(), files: (*all).clone(), mal_id: None, is_anime: false, anime_title: anime_title.clone() };
                st.update(cx, |s, cx| s.play(t, cx));
            }
        });
        let title = self.segs().last().cloned().unwrap_or_default();
        let on_read: FileAction = Rc::new({
            let dir = dir.clone();
            move |f, _, cx| {
                let t = ReadTarget { dir: dir.clone(), file: f.clone(), files: (*all).clone(), title_path: dir.clone(), title: title.clone() };
                st.update(cx, |s, cx| s.read(t, cx));
            }
        });
        FileRows::new(&dir, files, self.raw, Some(on_play), Some(on_read))
    }

    /// Pasta com músicas vira encarte (se não for álbum, o encarte avisa e volta à lista).
    fn ensure_album(&mut self, cx: &mut Context<Self>) {
        let Some(Ok(files)) = &self.listing else { return };
        if self.album.is_some() || !looks_like_album(files) {
            return;
        }
        let segs = self.segs();
        let (path, files, work) = (self.path.clone(), files.clone(), segs.len().checked_sub(2).map(|i| segs[i].clone()));
        let view = cx.new(|cx| AlbumView::new(path, files, work, cx));
        self._subs.push(cx.subscribe(&view, |v, _, e: &AlbumEvent, cx| {
            if matches!(e, AlbumEvent::NotAlbum) {
                v.not_album = true;
                cx.notify();
            }
        }));
        self.album = Some(view);
    }

    fn segs(&self) -> Vec<String> {
        split_path(&self.path).into_iter().map(str::to_owned).collect()
    }

    /// Nome da obra para o player: a pasta de cima quando esta é "Temporada"/release.
    fn anime_title(&self) -> String {
        let segs = self.segs();
        let last = segs.last().cloned().unwrap_or_default();
        let lower = last.to_lowercase();
        match segs.len().checked_sub(2).map(|i| segs[i].clone()) {
            Some(up) if lower.starts_with("temporada") || lower.starts_with("season") || last.starts_with('[') || last.starts_with('(') => up,
            _ => last,
        }
    }

    /// Alturas das linhas, refeitas só quando o `rem` muda (zoom).
    fn heights(&mut self, rows: &FileRows, rem: Pixels) -> Rc<Vec<Size<Pixels>>> {
        match &self.heights {
            Some((at, h)) if *at == rem => h.clone(),
            _ => {
                let h = Rc::new(rows.heights(rem));
                self.heights = Some((rem, h.clone()));
                h
            }
        }
    }

    fn index_grid(&self, names: &[SharedString], width: f32, rem: f32, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let gap = 0.75 * rem;
        let cols = columns(width, 6. * rem, gap);
        let side = px(((width - (cols as f32 - 1.) * gap) / cols as f32).floor());
        div()
            .grid()
            .grid_cols(cols as u16)
            .gap_3()
            .children(names.iter().map(|name| {
                let short = name.trim_start_matches("Letra ").trim_start_matches('[').trim_end_matches(']').to_owned();
                let big = short.chars().count() <= 3;
                let target = folder_link(&self.path, name, self.raw);
                let st = self.state.clone();
                div()
                    .id(ElementId::Name(format!("tile:{name}").into()))
                    .w(side)
                    .h(side)
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_2()
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.group_box)
                    .font_weight(FontWeight::SEMIBOLD)
                    .relative()
                    .cursor_pointer()
                    .map(|d| if big { d.text_2xl() } else { d.text_sm() })
                    .child(short)
                    .child(hover_layer("ring").border(theme.ring).radius(theme.radius_lg))
                    .on_click(move |_, _, cx| st.update(cx, |s, cx| s.go(target.clone(), cx)))
            }))
            .into_any_element()
    }
}

#[hotpath::measure_all]
impl Render for FolderView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rem_px = window.rem_size();
        let rem = f32::from(rem_px);
        let width = content_width(window, cx, 80. * rem);
        let segs = self.segs();
        let title = if segs.len() == 1 { section_label(&segs[0]) } else { segs.last().cloned().unwrap_or_default() };
        let album_active = self.album.is_some() && !self.not_album;

        let header = v_flex()
            .gap_6()
            .child(breadcrumbs_in(&self.path, self.raw, cx))
            .when(!album_active, |d| d.child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(title)));
        let page = v_flex().w(px(width)).mx_auto().py_6().gap_6();

        // Lista de arquivos: cabeçalho fixo e a lista virtual ocupando o resto da altura.
        if !album_active && let Some(Body::Files(rows)) = &self.body {
            let rows = rows.clone();
            if rows.is_empty() {
                return div().id("folder").size_full().overflow_y_scroll().child(page.child(header).child(empty_folder(cx))).into_any_element();
            }
            let heights = self.heights(&rows, rem_px);
            let list_rows = rows.clone();
            let list = v_virtual_list(cx.entity(), "folder-files", heights, move |_, range, _, cx| {
                let reading = list_rows.reading(cx);
                range.map(|i| list_rows.row(i, &reading, Some(list_rows.row_height(i, rem_px)), cx)).collect::<Vec<_>>()
            });
            return div()
                .size_full()
                .child(page.h_full().child(header).children(rows.shared_note(cx)).child(file_list_frame(cx).flex_1().min_h_0().child(list)))
                .into_any_element();
        }

        let body = match (&self.listing, &self.body) {
            (Some(Err(e)), _) => empty_state(IconName::WifiOff, "Não foi possível abrir esta pasta", Some(e.clone().into()), cx).into_any_element(),
            _ if album_active => self.album.clone().unwrap().into_any_element(),
            (_, Some(Body::Index(names))) => {
                let names = names.clone();
                self.index_grid(&names, width, rem, cx)
            }
            _ => Skeleton::new().h_72().rounded(theme.radius_lg).into_any_element(),
        };
        div().id("folder").size_full().overflow_y_scroll().child(page.child(header).child(body)).into_any_element()
    }
}

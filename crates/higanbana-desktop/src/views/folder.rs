//! Pasta comum. Só pastas de nome curto (ex.: `Letra A`, `0-9`) viram índice em
//! blocos; pasta com músicas vira encarte.

use crate::interact::hover_layer;
use std::rc::Rc;

use gpui_kit::{
    Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    assets::IconName, div, prelude::*, px,
};
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use higanbana_core::{
    library::{natural_cmp, split_path},
    nuvem::FileEntry,
};

use crate::{
    components::{breadcrumbs_in, columns, content_width, file_list_in, folder_link, section_label, FileAction},
    music::album::{AlbumEvent, AlbumView, looks_like_album},
    reader::ReadTarget,
    state::{AppState, state},
    ui::empty_state,
    video::PlayTarget,
    views::load_listing,
};

pub struct FolderView {
    state: Entity<AppState>,
    path: String,
    /// Aberta crua (`raw:`): as subpastas e migalhas continuam cruas.
    raw: bool,
    listing: Option<Result<Vec<FileEntry>, String>>,
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
            v.ensure_album(cx);
        });
        Self { state, path, raw, listing: None, album: None, not_album: false, _subs: subs }
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
}

impl Render for FolderView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rem = f32::from(window.rem_size());
        let width = content_width(window, 80. * rem);
        let segs = self.segs();
        let title = if segs.len() == 1 { section_label(&segs[0]) } else { segs.last().cloned().unwrap_or_default() };
        let files = match &self.listing {
            Some(Ok(f)) => Some(f.clone()),
            _ => None,
        };
        let album_active = self.album.is_some() && !self.not_album;

        let body = match (&self.listing, files) {
            (None, _) | (Some(Ok(_)), None) => Skeleton::new().h_72().rounded(theme.radius_lg).into_any_element(),
            (Some(Err(e)), _) => empty_state(IconName::WifiOff, "Não foi possível abrir esta pasta", Some(e.clone().into()), cx).into_any_element(),
            (_, Some(_)) if album_active => self.album.clone().unwrap().into_any_element(),
            (_, Some(files)) => {
                let dirs: Vec<&FileEntry> = files.iter().filter(|f| f.is_directory).collect();
                let is_index = dirs.len() >= 6 && dirs.len() == files.len() && dirs.iter().all(|d| d.name.chars().count() <= 16);
                if is_index {
                    let mut names: Vec<String> = dirs.iter().map(|d| d.name.clone()).collect();
                    names.sort_by(|a, b| natural_cmp(a, b));
                    let gap = 0.75 * rem;
                    let cols = columns(width, 6. * rem, gap);
                    let side = px(((width - (cols as f32 - 1.) * gap) / cols as f32).floor());
                    div()
                        .grid()
                        .grid_cols(cols as u16)
                        .gap_3()
                        .children(names.into_iter().map(|name| {
                            let short = name.trim_start_matches("Letra ").trim_start_matches('[').trim_end_matches(']').to_owned();
                            let big = short.chars().count() <= 3;
                            let target = folder_link(&self.path, &name, self.raw);
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
                } else {
                    let (dir, all) = (self.path.clone(), files.clone());
                    let st = self.state.clone();
                    let anime_title = self.anime_title();
                    let on_play: FileAction = Rc::new({
                        let (dir, all, st) = (dir.clone(), all.clone(), st.clone());
                        move |f, _, cx| {
                            let t = PlayTarget { dir: dir.clone(), file: f.clone(), files: all.clone(), mal_id: None, is_anime: false, anime_title: anime_title.clone() };
                            st.update(cx, |s, cx| s.play(t, cx));
                        }
                    });
                    let title = segs.last().cloned().unwrap_or_default();
                    let on_read: FileAction = Rc::new(move |f, _, cx| {
                        let t = ReadTarget { dir: dir.clone(), file: f.clone(), files: all.clone(), title_path: dir.clone(), title: title.clone() };
                        st.update(cx, |s, cx| s.read(t, cx));
                    });
                    file_list_in(&self.path, &files, self.raw, Some(on_play), Some(on_read), cx).into_any_element()
                }
            }
        };

        v_flex()
            .w(px(width))
            .mx_auto()
            .py_6()
            .gap_6()
            .child(breadcrumbs_in(&self.path, self.raw, cx))
            .when(!album_active, |d| d.child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(title)))
            .child(body)
    }
}

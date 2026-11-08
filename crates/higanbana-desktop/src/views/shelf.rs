//! Pasta cujos filhos são títulos (ex.: `Animes/Letra B`) → grade de pôsteres.

use crate::interact::Button;
use gpui_kit::{
    Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    assets::IconName, div, prelude::*, px,
};
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Selectable as _, Sizable as _, h_flex, v_flex,
    button::ButtonVariants as _,
    input::{Input, InputEvent, InputState},
};
use higanbana_core::{
    anilist::MediaType,
    library::{base_name, join_path, natural_cmp, parent_path},
    nuvem::FileEntry,
};

use crate::{
    components::{breadcrumbs, columns, content_width, title_card},
    state::{AppState, state},
    ui::empty_state,
    views::load_listing,
};

pub struct ShelfView {
    state: Entity<AppState>,
    path: String,
    media: MediaType,
    listing: Option<Result<Vec<FileEntry>, String>>,
    siblings: Option<Vec<FileEntry>>,
    filter: Entity<InputState>,
    _subs: Vec<Subscription>,
}

impl ShelfView {
    pub fn new(path: String, media: MediaType, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        let name = base_name(&path).to_owned();
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder(format!("Filtrar em {name}")));
        let subs = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.subscribe(&filter, |_, _, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        load_listing(&state, path.clone(), cx, |v: &mut Self, r, _| v.listing = Some(r));
        load_listing(&state, parent_path(&path), cx, |v: &mut Self, r, _| v.siblings = r.ok());
        Self { state, path, media, listing: None, siblings: None, filter, _subs: subs }
    }
}

impl Render for ShelfView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rem = f32::from(window.rem_size());
        let width = content_width(window, 80. * rem);
        let name = base_name(&self.path).to_owned();
        let query = self.filter.read(cx).value().trim().to_owned();
        let q = query.to_lowercase();
        let titles: Vec<FileEntry> = match &self.listing {
            Some(Ok(files)) => {
                let mut t: Vec<FileEntry> =
                    files.iter().filter(|f| f.is_directory && (q.is_empty() || f.name.to_lowercase().contains(&q))).cloned().collect();
                t.sort_by(|a, b| natural_cmp(&a.name, &b.name));
                t
            }
            _ => Vec::new(),
        };
        let gap = 1.25 * rem;
        let cols = columns(width, 10. * rem, gap);
        let card_w = px(((width - (cols as f32 - 1.) * gap) / cols as f32).floor());
        let parent = parent_path(&self.path);

        // Pastas irmãs: o índice A–Z numa linha e as coleções avulsas noutra,
        // cada grupo com a atual selecionada.
        let jump = |sib: &FileEntry, label: String| {
            let target = join_path(&parent, &sib.name);
            let st = self.state.clone();
            Button::new(ElementId::Name(format!("sibling:{}", sib.name).into()))
                .ghost()
                .small()
                .label(label)
                .selected(sib.name == name)
                .on_click(move |_, _, cx| st.update(cx, |s, cx| s.go(target.clone(), cx)))
        };
        let (letters, collections): (Vec<&FileEntry>, Vec<&FileEntry>) = self
            .siblings
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter(|f| f.is_directory)
            .partition(|f| is_index_letter(&f.name));
        let index = (!letters.is_empty()).then(|| {
            h_flex().flex_wrap().gap_0p5().children(letters.into_iter().map(|s| jump(s, s.name.trim_start_matches("Letra ").to_owned())))
        });
        let collections = (!collections.is_empty()).then(|| {
            h_flex()
                .flex_wrap()
                .items_center()
                .gap_1()
                .child(div().mr_2().text_xs().text_color(theme.muted_foreground).child("Coleções"))
                .children(collections.into_iter().map(|s| jump(s, collection_label(&s.name))))
        });

        let grid = match &self.listing {
            None => div().grid().grid_cols(cols as u16).gap_x_5().gap_y_8().children((0..cols * 2).map(|_| Skeleton::new().w(card_w).h(card_w * 1.5).rounded(theme.radius_lg))),
            Some(Err(e)) => empty_state(IconName::WifiOff, "Não foi possível abrir esta pasta", Some(e.clone().into()), cx),
            Some(Ok(_)) if titles.is_empty() => empty_state(IconName::Search, format!("Nenhum título com “{query}” em {name}"), None, cx),
            Some(Ok(_)) => div().grid().grid_cols(cols as u16).gap_x_5().gap_y_8().children(titles.into_iter().map(|t| {
                let id = ElementId::Name(format!("title:{}", t.name).into());
                title_card(id, &t.name, join_path(&self.path, &t.name), self.media, card_w, cx)
            })),
        };
        let count = match &self.listing {
            Some(Ok(f)) => Some(f.iter().filter(|f| f.is_directory).count()),
            _ => None,
        };

        v_flex()
            .w(px(width))
            .mx_auto()
            .py_6()
            .gap_6()
            .child(breadcrumbs(&self.path, cx))
            .child(
                h_flex()
                    .flex_wrap()
                    .items_end()
                    .justify_between()
                    .gap_4()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(name.clone()))
                            .children(count.map(|n| div().text_sm().text_color(theme.muted_foreground).child(format!("{n} títulos")))),
                    )
                    .child(div().w_72().child(Input::new(&self.filter).prefix(Icon::new(IconName::Search).small()).cleanable(true))),
            )
            .when(index.is_some() || collections.is_some(), |d| d.child(v_flex().gap_2().children(index).children(collections)))
            .child(grid)
    }
}

/// "[Colecao de Animes X]" → "Animes X": o grupo já diz que é coleção.
fn collection_label(name: &str) -> String {
    let inner = name.trim_start_matches('[').trim_end_matches(']');
    let inner = inner.strip_prefix("Colecao ").or_else(|| inner.strip_prefix("Coleção ")).unwrap_or(inner);
    inner.strip_prefix("de ").unwrap_or(inner).to_owned()
}

/// "Letra A" e "0-9" formam o índice; o resto são coleções.
fn is_index_letter(name: &str) -> bool {
    name == "0-9" || name.strip_prefix("Letra ").is_some_and(|l| l.chars().count() == 1)
}

#[cfg(test)]
mod tests {
    use super::{collection_label, is_index_letter};

    #[test]
    fn collection_names_without_the_prefix() {
        assert_eq!(collection_label("[Colecao de Animes com Dublagem Portuguesa]"), "Animes com Dublagem Portuguesa");
        assert_eq!(collection_label("[Colecao Curtas]"), "Curtas");
        assert_eq!(collection_label("Extras"), "Extras");
    }

    #[test]
    fn index_letters_apart_from_collections() {
        assert!(is_index_letter("Letra C"));
        assert!(is_index_letter("0-9"));
        assert!(!is_index_letter("[Colecao Curtas]"));
        assert!(!is_index_letter("Letra Grande"));
    }
}

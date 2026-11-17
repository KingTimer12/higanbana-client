//! Peças pequenas compartilhadas pelas telas, em cima dos componentes do GPUI Kit.

use std::borrow::Cow;

use gpui_kit::{
    AssetSource, Div, ParentElement, SharedString, Styled, div,
    assets::{IconName, icon_assets},
};

// Ícones do Lucide que as telas usam além dos que os componentes já trazem.
icon_assets!(
    ProductIcons,
    [
        ArrowDown, ArrowLeft, ArrowRight, AudioLines, BookOpen, Captions, Check, ChevronLeft, ChevronRight, Compass, Flame, Sparkles, Trophy,
        ChevronsRight, CircleAlert, Clapperboard, Columns2, Copy, Disc3, Download, ExternalLink, FilePlay, Folder,
        FolderCheck, FolderOpen, House, Images, Inbox, Info, ListVideo, LoaderCircle, LogIn, LogOut, Maximize,
        Minimize, Music, Palette, Pause, Play, RectangleVertical, RefreshCw, Repeat, Repeat1, ScrollText, Search,
        Settings2, Shuffle, SkipBack, SkipForward, SlidersHorizontal, Star, TriangleAlert, Tv, Volume1, Volume2,
        VolumeX, WifiOff, X, ZoomIn, ZoomOut
    ]
);

/// Ícones dos componentes do kit + os do produto.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = ProductIcons.load(path)? {
            return Ok(Some(bytes));
        }
        // O pacote padrão dá erro para caminho desconhecido; para nós é só "não tem".
        Ok(gpui_kit::assets::Assets.load(path).ok().flatten())
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        let mut all = ProductIcons.list(path)?;
        all.extend(gpui_kit::assets::Assets.list(path)?);
        all.sort();
        all.dedup();
        Ok(all)
    }
}

/// Texto em uma linha só, cortado com reticências.
pub fn truncate(text: impl Into<SharedString>) -> Div {
    div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(text.into())
}

/// Bloco de estado vazio ou de erro: ícone, título, explicação e uma ação opcional.
pub fn empty_state(icon: IconName, title: impl Into<SharedString>, detail: Option<SharedString>, cx: &gpui_kit::App) -> Div {
    use gpui_kit::component::{ActiveTheme as _, Icon as KitIcon, v_flex};
    v_flex()
        .items_center()
        .gap_2()
        .py_12()
        .px_6()
        .text_center()
        .child(KitIcon::new(icon).size_8().text_color(cx.theme().muted_foreground))
        .child(div().font_weight(gpui_kit::FontWeight::MEDIUM).child(title.into()))
        .children(detail.map(|d| div().max_w_96().text_sm().text_color(cx.theme().muted_foreground).child(d)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_and_component_icons_load() {
        assert!(Assets.load("icons/audio-lines.svg").unwrap().is_some());
        assert!(Assets.load(&IconName::Disc3.path()).unwrap().is_some());
        // Ícone dos componentes (pacote padrão).
        assert!(Assets.load("icons/chevron-down.svg").unwrap().is_some());
        assert!(Assets.load("icons/nao-existe.svg").unwrap().is_none());
    }
}

thread_local! {
    static INTERNED: std::cell::RefCell<rustc_hash::FxHashMap<Box<str>, SharedString>> = Default::default();
}

/// `SharedString` de um texto que se repete a cada quadro (nome e caminho de obra,
/// id de card): a primeira vez aloca, as seguintes só consultam o mapa e copiam o
/// ponteiro. Só para textos do acervo, que são finitos; nunca para texto digitado.
pub fn interned(text: &str) -> SharedString {
    INTERNED.with(|map| {
        if let Some(s) = map.borrow().get(text) {
            return s.clone();
        }
        let s = SharedString::from(text.to_owned());
        map.borrow_mut().insert(text.into(), s.clone());
        s
    })
}

#[cfg(test)]
mod interned_tests {
    use super::*;

    #[test]
    fn same_text_shares_one_allocation() {
        let a = interned("Animes/Letra B/B - The Beginning");
        let b = interned("Animes/Letra B/B - The Beginning");
        assert_eq!(a, b);
        assert!(std::ptr::eq(a.as_ref().as_ptr(), b.as_ref().as_ptr()));
    }
}

thread_local! {
    static INTERNED_IDS: std::cell::RefCell<rustc_hash::FxHashMap<&'static str, rustc_hash::FxHashMap<Box<str>, gpui_kit::ElementId>>> = Default::default();
}

/// `ElementId` `"<prefixo>:<chave>"` sem montar a string a cada quadro (ver [`interned`]).
pub fn interned_id(prefix: &'static str, key: &str) -> gpui_kit::ElementId {
    INTERNED_IDS.with(|map| {
        if let Some(id) = map.borrow().get(prefix).and_then(|m| m.get(key)) {
            return id.clone();
        }
        let id = gpui_kit::ElementId::Name(format!("{prefix}:{key}").into());
        map.borrow_mut().entry(prefix).or_default().insert(key.into(), id.clone());
        id
    })
}

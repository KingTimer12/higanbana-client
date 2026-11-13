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

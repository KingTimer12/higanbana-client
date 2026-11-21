//! Tela da rota atual. Até o Plano 4 portar as telas, cada rota mostra um
//! aviso provisório com o caminho (a moldura já é a final).

use higanbana_gpui::prelude::*;

use crate::chrome::section_label;
use crate::root::Higanbana;
use crate::ui::empty_state;

pub fn page(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let path = v.state.path();
    let title = if path.is_empty() { "Início".to_owned() } else { path.split('/').map(section_label).collect::<Vec<_>>().join(" / ") };
    empty_state(cx.theme(), "compass", title, Some("Esta tela ainda está sendo migrada para a nova interface.".into()))
}

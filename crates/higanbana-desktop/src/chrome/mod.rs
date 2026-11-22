//! Moldura da janela: barra de título, busca (paleta), downloads e barra lateral.

mod downloads;
mod search;
mod sections;
mod sidebar;
mod title_bar;
mod update;

use std::borrow::Cow;
use std::collections::HashMap;

use higanbana_gpui::components::{Component, Tooltip, TooltipState};
use higanbana_gpui::prelude::*;
use higanbana_gpui::ElementId;

use crate::root::Higanbana;

pub use search::{input_id as palette_input, palette};
pub use sections::{active_nav, format_bytes, is_library_section, nav_entries, section_label, sort_sections, NavEntry};
pub use sidebar::sidebar;
pub use title_bar::{brand, title_bar};

/// Estados das dicas (uma por botão com tooltip), criados sob demanda.
#[derive(Default)]
pub struct Tips(HashMap<ElementId, TooltipState>);

/// Envolve `trigger` numa dica do kit (embaixo, depois do atraso).
pub fn tip(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, id: impl Into<ElementId>, trigger: Element<Higanbana>, label: impl Into<Cow<'static, str>>) -> Element<Higanbana> {
    let id = id.into();
    let state = v.tips.0.get(&id).copied().unwrap_or_default();
    Tooltip::new(id, &state, move |v: &mut Higanbana| v.tips.0.entry(id).or_default(), trigger, label).kit().render(cx)
}

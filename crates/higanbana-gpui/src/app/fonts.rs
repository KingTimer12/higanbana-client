//! Fontes de arquivo do app: somadas ao contexto antes da primeira janela.

use std::path::PathBuf;

use crate::gfx::{font_files_in, Context};
use crate::theme::Theme;

/// Família de reserva quando a do tema não existe (arquivo apagado, Windows sem ela).
pub const FALLBACK_FAMILY: &str = "Segoe UI";

/// Soma as fontes das pastas (falha de um arquivo só avisa) e, se a família do
/// tema não existir depois disso, troca pela de reserva.
pub(crate) fn install(ctx: &Context, dirs: &[PathBuf], theme: &mut Theme) {
    for f in dirs.iter().flat_map(|d| font_files_in(d)) {
        if let Err(e) = ctx.add_font_file(&f) {
            eprintln!("higanbana-gpui: fonte {} falhou: {e}", f.display());
        }
    }
    theme.typography.family = pick_family(theme.typography.family, |f| ctx.has_font_family(f));
}

pub(crate) fn pick_family(wanted: &'static str, exists: impl Fn(&str) -> bool) -> &'static str {
    if exists(wanted) { wanted } else { FALLBACK_FAMILY }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn familia_ausente_vira_reserva() {
        assert_eq!(pick_family("Noto Sans Georgian", |_| false), FALLBACK_FAMILY);
        assert_eq!(pick_family("Noto Sans Georgian", |f| f == "Noto Sans Georgian"), "Noto Sans Georgian");
    }
}

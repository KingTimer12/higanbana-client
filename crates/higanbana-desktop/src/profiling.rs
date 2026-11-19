//! Perfilador [hotpath](https://hotpath.rs), ligado pela feature `hotpath`.
//!
//! Sem a feature, `#[hotpath::measure]`, `#[hotpath::measure_all]` e
//! `hotpath::measure_block!` não geram código nenhum e a guarda é vazia: custo zero.
//! Com ela, cada função marcada entra na tabela de tempos (chamadas, média, P95,
//! total e % do tempo de vida do app), escrita quando o app fecha.
//!
//! O relatório não pode depender de `main` terminar: no macOS o `cx.quit()` passa
//! por `NSApp terminate:`, que encerra o processo sem soltar as variáveis de `main`.
//! Por isso a guarda é solta num observador de saída do GPUI (`on_app_quit`).

use std::{path::Path, sync::Arc};

use gpui_kit::App;
use higanbana_core::{
    Core,
    anilist::{AniListResult, Lookup, LookupRequest, MediaType, Priority, types::MediaSummary},
    fanart::FanartResult,
    reader::{OpenResult, PageRef},
};

/// Começa a medir. Chamar uma vez, no começo de `main` (o hotpath aceita uma
/// guarda só por processo).
pub fn start(cache: &Path) -> hotpath::HotpathGuard {
    let mut builder = hotpath::HotpathGuardBuilder::new("higanbana")
        .percentiles(&[50.0, 95.0, 99.0])
        // App de interface: muitas funções medidas; 40 linhas cobrem as que pesam.
        .functions_limit(40);
    // Release no Windows não tem console: o relatório vai para um arquivo no cache.
    // `HOTPATH_OUTPUT_PATH` continua valendo por cima disso.
    if cfg!(all(windows, not(debug_assertions))) {
        builder = builder.output_path(cache.join("hotpath.txt"));
    }
    builder.build()
}

/// Escreve o relatório quando o app sai (inclusive pelo `terminate:` do macOS).
pub fn report_on_quit(guard: hotpath::HotpathGuard, cx: &App) {
    let mut guard = Some(guard);
    cx.on_app_quit(move |_| {
        // Soltar a guarda escreve o relatório (sem a feature, não faz nada).
        let _ = guard.take();
        async {}
    })
    .detach();
}

// --- Chamadas pesadas ao núcleo, medidas daqui (o core é compartilhado com o app
// mobile e fica sem a dependência) ---

/// Lista a raiz do acervo e indexa todas as letras (a varredura da biblioteca).
#[hotpath::measure]
pub async fn scan_library(core: Arc<Core>, media: MediaType) -> Result<(), String> {
    core.section_index(media).root(core.cloud()).await?;
    core.section_index(media).fill_all(core.cloud()).await
}

/// Indexa uma letra do acervo (prioridade do que está na tela).
#[hotpath::measure]
pub async fn index_letter(core: Arc<Core>, media: MediaType, letter: String) -> Result<(), String> {
    core.section_index(media).letter(core.cloud(), &letter).await.map(|_| ())
}

/// Lote de buscas no AniList (com o fallback do MAL).
#[hotpath::measure]
pub async fn anilist_lookup(core: Arc<Core>, items: Vec<LookupRequest>, priority: Priority) -> AniListResult<Vec<Lookup>> {
    core.anilist().lookup_with(&items, priority).await
}

/// Capa do fanart: busca, baixa e regrava em WebP/JPEG.
#[hotpath::measure]
pub async fn fanart_cover(core: Arc<Core>, media: MediaSummary) -> FanartResult<Option<std::path::PathBuf>> {
    core.fanart().cover(&media).await
}

/// Abre um capítulo do leitor (índice do zip/rar).
#[hotpath::measure]
pub async fn reader_open(core: Arc<Core>, path: String, size: u64, inner: Option<String>) -> Result<OpenResult, String> {
    core.reader().open(&path, size, inner.as_deref()).await
}

/// Bytes de uma página do leitor.
#[hotpath::measure]
pub async fn reader_page(core: Arc<Core>, page: PageRef) -> Result<(Vec<u8>, String), String> {
    core.reader().page(&page).await
}

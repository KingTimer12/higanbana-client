//! Logs do app: terminal (quando há um) e arquivo diário em `<cache>/logs`.
//!
//! O filtro vem de `RUST_LOG` quando definido; senão, `info` para tudo e `debug`
//! para o próprio app (chamadas feitas, páginas, imagens).

use std::path::Path;

use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _};

const DEFAULT_FILTER: &str = "info,higanbana=debug,higanbana_core=debug,wgpu=warn,naga=warn";
/// Quantos dias de log ficam guardados.
const KEEP_DAYS: usize = 7;

/// Liga os logs. Guarde o retorno até o fim do `main`: ao ser descartado, grava o
/// que ainda estiver na fila do arquivo.
pub fn init(cache_dir: &Path) -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    let dir = cache_dir.join("logs");
    // O appender lê a pasta para apagar logs antigos antes de criá-la.
    let _ = std::fs::create_dir_all(&dir);
    let file = rolling::Builder::new()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix("higanbana")
        .filename_suffix("log")
        .max_log_files(KEEP_DAYS)
        .build(&dir)
        .ok();
    let (file_layer, guard) = match file {
        Some(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            (Some(fmt::layer().with_ansi(false).with_writer(writer)), Some(guard))
        }
        None => (None, None),
    };
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(file_layer)
        .init();
    // Pânico vai para o log também (no release não há terminal para ver).
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(%info, "pânico");
        default_hook(info);
    }));
    if guard.is_some() {
        tracing::info!(dir = %dir.display(), "logs em arquivo");
    } else {
        tracing::warn!(dir = %dir.display(), "não deu para abrir o arquivo de log; só no terminal");
    }
    guard
}

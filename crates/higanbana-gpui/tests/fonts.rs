//! Fontes de arquivo: somar ao contexto e achar a família pelo nome.
use higanbana_gpui::prelude::*;
use higanbana_gpui::{font_files_in, Context, FontSpec};
use std::path::PathBuf;

fn fonts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../higanbana-desktop/fonts")
}

#[test]
fn pasta_lista_so_fontes_em_ordem() {
    let files = font_files_in(&fonts_dir());
    assert_eq!(files.len(), 6, "seis pesos do Noto Sans Georgian, sem o OFL.txt");
    assert!(files.windows(2).all(|w| w[0] < w[1]));
    assert!(font_files_in(&PathBuf::from("C:/nao/existe")).is_empty());
}

#[test]
fn contexto_soma_arquivo_e_acha_familia() {
    let ctx = Context::new(BackendKind::D3D11).expect("contexto");
    assert!(!ctx.has_font_family("Noto Sans Georgian"));
    for f in font_files_in(&fonts_dir()) {
        ctx.add_font_file(&f).expect("fonte");
    }
    assert!(ctx.has_font_family("Noto Sans Georgian"));
    let bold = ctx.font(&FontSpec::new("Noto Sans Georgian", 700, false)).unwrap();
    let light = ctx.font(&FontSpec::new("Noto Sans Georgian", 300, false)).unwrap();
    let wb = ctx.measure_text(bold, 16.0, "Higanbana", 0.0).unwrap().width;
    let wl = ctx.measure_text(light, 16.0, "Higanbana", 0.0).unwrap().width;
    assert!(wb > wl, "negrito {wb} mais largo que light {wl}: os pesos vieram do arquivo");
    assert!(ctx.add_font_file(&fonts_dir().join("OFL.txt")).is_err());
}

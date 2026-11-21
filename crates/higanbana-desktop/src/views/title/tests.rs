use super::{tabs, Section, TitleUi};

#[test]
fn aba_sem_conteudo_nao_aparece() {
    let m = super::data::Model { title_path: "x".into(), rest: vec![], media: higanbana_core::anilist::MediaType::Manga, title: None, title_dirs: vec![], franchise: false, season: None, work_split: Default::default() };
    let w = super::Work { model: m, summary: None, detail: None, media: None, heading: String::new(), files_path: None, files: None, episodes: None, release: None };
    let t = tabs(&w, higanbana_core::anilist::MediaType::Manga);
    assert_eq!(t.len(), 1, "sem detalhe: só a aba de arquivos");
    assert!(t[0].0 == Section::Files && t[0].1 == "Arquivos" && t[0].2.is_none());
    assert!(TitleUi::default().tab == Section::Files);
}

use super::Pages;

#[test]
fn trocar_tela_solta_estado() {
    let mut p = Pages::default();
    let s = |_| crate::state::CatalogChoice::default();
    assert!(p.sync("", s));
    assert!(p.home.is_some(), "início cria o estado dele");
    assert!(!p.sync("", s), "mesma rota mantém o estado");
    assert!(p.sync("Animes", s));
    assert!(p.catalog.is_some() && p.home.is_none());
    assert!(p.sync("Doramas", s));
    assert!(p.home.is_none(), "saiu do início: estado solto");
    assert_eq!(p.route(), Some("Doramas"));
}

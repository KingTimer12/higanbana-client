use super::Pages;

#[test]
fn trocar_tela_solta_estado() {
    let mut p = Pages::default();
    assert!(p.sync("Animes/Letra F/Frieren"));
    assert!(!p.sync("Animes/Letra F/Frieren"), "mesma rota mantém o estado");
    assert!(p.sync("Doramas"));
    assert_eq!(p.route(), Some("Doramas"));
}

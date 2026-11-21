use super::Pages;

#[test]
fn trocar_tela_solta_estado() {
    let mut p = Pages::default();
    assert!(p.sync(""));
    assert!(p.home.is_some(), "início cria o estado dele");
    assert!(!p.sync(""), "mesma rota mantém o estado");
    assert!(p.sync("Doramas"));
    assert!(p.home.is_none(), "saiu do início: estado solto");
    assert_eq!(p.route(), Some("Doramas"));
}

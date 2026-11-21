//! Sem libmpv: erro legível, sem pânico (processo separado: a DLL é carregada uma vez).
use higanbana_gpui::video::Player;
use higanbana_gpui::Waker;

#[test]
fn missing_dll_is_error() {
    // SAFETY: teste de processo próprio, antes de qualquer outra thread ler o ambiente.
    unsafe { std::env::set_var("HIGANBANA_MPV_DLL", "C:/nao/existe/libmpv-2.dll") };
    let err = Player::new(Waker::noop()).err().expect("deveria falhar");
    assert!(err.contains("libmpv"), "{err}");
}

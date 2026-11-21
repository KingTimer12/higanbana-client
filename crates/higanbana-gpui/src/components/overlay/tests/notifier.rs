//! Notifier: notificações no visual do kit (ícone por tom, topo-direito, 5 s).
use super::Harness;
use crate::components::form::test_util::paint;
use crate::components::{Component, Notifier, Toast, ToastTone, Toasts};
use crate::prelude::*;
use crate::Tree;
use std::time::Duration;

#[derive(Default)]
struct V {
    toasts: Toasts,
}

fn frame(h: &mut Harness, t: &mut Tree<V>, v: &V) {
    h.frame(t, |cx| div().size_full().child(Notifier::new("avisos", &v.toasts, |v: &mut V| &mut v.toasts).render(cx)));
}

#[test]
fn push_success_usa_tom_e_cinco_segundos() {
    let mut q = Toasts::new();
    q.push_success("Download concluído");
    let e = q.items.last().unwrap();
    assert_eq!(e.toast.tone, Some(ToastTone::Success));
    assert_eq!(e.toast.duration, Duration::from_secs(5));
    assert_eq!(e.toast.title, "Download concluído");
}

#[test]
fn push_error_usa_tom_de_erro() {
    let mut q = Toasts::new();
    q.push_error("Falhou");
    assert_eq!(q.items.last().unwrap().toast.tone, Some(ToastTone::Error));
}

#[test]
fn icone_e_cor_por_tom() {
    let th = Theme::dark();
    assert_eq!(ToastTone::Success.look(&th), ("circle-check", th.success));
    assert_eq!(ToastTone::Error.look(&th), ("circle-x", th.destructive));
    assert_eq!(ToastTone::Info.look(&th), ("info", th.info));
    assert_eq!(ToastTone::Warning.look(&th), ("triangle-alert", th.warning));
}

#[test]
fn cartao_no_topo_direito_com_fundo_popover() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    v.toasts.push_at(Toast::new("Salvo").tone(ToastTone::Success).duration(Duration::from_secs(5)), h.t0);
    frame(&mut h, &mut t, &v);
    h.advance(500);
    frame(&mut h, &mut t, &v);
    let rec = paint(&t);
    assert!(rec.texts.iter().any(|s| s == "Salvo"));
    let card = rec.quads.iter().find(|(_, c)| *c == Theme::dark().popover).expect("cartão popover").0;
    assert_eq!(card.size.width, 382.0);
    assert_eq!(card.origin.x + card.size.width, 800.0 - 16.0, "16 px da direita");
    assert_eq!(card.origin.y, 34.0 + 16.0, "abaixo da barra de título");
}

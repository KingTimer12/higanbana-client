use super::mailbox::{inbox, Mailbox};

#[test]
fn mensagem_roda_na_view() {
    let (tx, mut rx) = inbox::<u32>();
    std::thread::spawn(move || {
        assert!(tx.send(|v: &mut u32, _| *v += 1));
    })
    .join()
    .unwrap();
    let mut v = 0u32;
    assert!(rx.drain(&mut v).is_some());
    assert_eq!(v, 1);
}

#[test]
fn ordem_preservada() {
    let (tx, mut rx) = inbox::<Vec<u32>>();
    for i in 1..=3 {
        tx.send(move |v: &mut Vec<u32>, _| v.push(i));
    }
    let mut v = Vec::new();
    rx.drain(&mut v);
    assert_eq!(v, vec![1, 2, 3]);
}

#[test]
fn envio_apos_fechar_devolve_false() {
    let (tx, rx) = inbox::<u32>();
    drop(rx);
    assert!(!tx.send(|v: &mut u32, _| *v += 1));
}

#[test]
fn envio_antes_da_janela_fica_guardado() {
    let (tx, mut rx): (Mailbox<u32>, _) = inbox();
    tx.send(|v: &mut u32, _| *v = 7);
    // A janela só "abre" (recebe o waker) depois.
    rx.attach(crate::gfx::Waker::noop());
    let mut v = 0;
    rx.drain(&mut v);
    assert_eq!(v, 7);
}

#[test]
fn sem_mensagem_nao_devolve_cx() {
    let (_tx, mut rx) = inbox::<u32>();
    assert!(rx.drain(&mut 0).is_none());
}

//! Carousel: navegação circular, auto-avanço pelo relógio (sem timer) e deslocamento.
use super::{target_offset, Carousel, CarouselState};
use crate::anim::Animator;
use crate::components::form::test_util::with_cx;
use crate::prelude::*;
use std::time::{Duration, Instant};

struct V {
    c: CarouselState,
}

#[test]
fn next_circular() {
    let t = Instant::now();
    let mut s = CarouselState::new(3).looping(true);
    s.next(t);
    s.next(t);
    s.next(t);
    assert_eq!(s.current(t), 0);
}

#[test]
fn sem_loop_para_no_fim() {
    let t = Instant::now();
    let mut s = CarouselState::new(2);
    assert!(s.next(t));
    assert!(!s.next(t));
    assert_eq!(s.current(t), 1);
    assert!(!s.has_next(t));
}

#[test]
fn prev_do_zero_vai_pro_fim_com_loop() {
    let t = Instant::now();
    let mut s = CarouselState::new(4).looping(true);
    s.prev(t);
    assert_eq!(s.current(t), 3);
}

#[test]
fn len_0_nao_panica() {
    let t = Instant::now();
    let mut s = CarouselState::new(0).looping(true).auto_advance(Duration::from_secs(1));
    assert!(!s.next(t));
    assert!(!s.prev(t));
    assert_eq!(s.current(t + Duration::from_secs(9)), 0);
}

#[test]
fn len_1_next_fica_em_0() {
    let t = Instant::now();
    let mut s = CarouselState::new(1).looping(true);
    s.next(t);
    assert_eq!(s.current(t), 0);
}

#[test]
fn auto_avanco_troca_indice_apos_intervalo() {
    let t = Instant::now();
    let s = CarouselState::new(3).looping(true).auto_advance(Duration::from_secs(5)).started(t);
    assert_eq!(s.current(t + Duration::from_millis(4999)), 0);
    assert_eq!(s.current(t + Duration::from_secs(5)), 1);
    assert_eq!(s.current(t + Duration::from_secs(16)), 0);
}

#[test]
fn pausado_nao_avanca_e_retoma_de_onde_parou() {
    let t = Instant::now();
    let mut s = CarouselState::new(3).looping(true).auto_advance(Duration::from_secs(5)).started(t);
    s.set_paused(true, t + Duration::from_secs(3));
    assert_eq!(s.current(t + Duration::from_secs(60)), 0);
    s.set_paused(false, t + Duration::from_secs(60));
    assert_eq!(s.current(t + Duration::from_secs(61)), 0);
    assert_eq!(s.current(t + Duration::from_secs(62)), 1);
}

#[test]
fn set_len_menor_prende_indice() {
    let t = Instant::now();
    let mut s = CarouselState::new(5);
    s.go(4, t);
    s.set_len(2, t);
    assert_eq!(s.current(t), 1);
}

#[test]
fn deslocamento_preso_ao_fim() {
    // 10 itens de 100 + espaço 10, janela de 330: máximo = 1090 - 330 = 760.
    assert_eq!(target_offset(0, 10, Some(100.0), 10.0, 330.0), 0.0);
    assert_eq!(target_offset(2, 10, Some(100.0), 10.0, 330.0), 220.0);
    assert_eq!(target_offset(9, 10, Some(100.0), 10.0, 330.0), 760.0);
    // Item da largura da janela (banner): um por vez.
    assert_eq!(target_offset(2, 3, None, 0.0, 800.0), 1600.0);
}

#[test]
fn render_agenda_proxima_troca_so_com_auto_avanco() {
    let mut anim = Animator::default();
    let now = Instant::now();
    let v = V { c: CarouselState::new(3).looping(true).auto_advance(Duration::from_secs(5)).started(now) };
    let wake = with_cx::<V, _>(&mut anim, |cx| {
        cx.now = now + Duration::from_secs(2);
        let _ = Carousel::new("c", &v.c, |v: &mut V| &mut v.c).items((0..3).map(|_| div()).collect()).render(cx);
        cx.wake
    });
    assert_eq!(wake, Some(now + Duration::from_secs(5)));
    let parado = V { c: CarouselState::new(3) };
    let wake = with_cx::<V, _>(&mut anim, |cx| {
        let _ = Carousel::new("c", &parado.c, |v: &mut V| &mut v.c).items((0..3).map(|_| div()).collect()).render(cx);
        cx.wake
    });
    assert_eq!(wake, None);
}

use std::time::{Duration, Instant};

use higanbana_core::nuvem::FileEntry;
use higanbana_core::player::aniskip::{Interval, SkipTimes};
use higanbana_core::player::store::{PlayerData, Progress};
use higanbana_gpui::video::{Chapter, PlayerState, Status};

use super::options;
use super::session::{Action, PlayerSession};
use super::target::PlayTarget;

fn file(name: &str) -> FileEntry {
    FileEntry { name: name.into(), is_directory: false, size: 100, modified: String::new(), extension: Some(".mkv".into()) }
}

fn target() -> PlayTarget {
    let files = vec![file("Show - 01.mkv"), file("Show - 02.mkv")];
    PlayTarget { dir: "Animes/Show".into(), file: files[0].clone(), files, mal_id: Some(1), is_anime: true, anime_title: "Show".into() }
}

fn playing(pos: f64, dur: f64) -> PlayerState {
    let mut s = PlayerState::default();
    s.status = Status::Playing;
    s.position = pos;
    s.duration = dur;
    s
}

fn failed(n: u32) -> PlayerState {
    let mut s = PlayerState::default();
    s.status = Status::Error("rede".into());
    s.end_error = n;
    s
}

fn saves(a: &[Action]) -> usize {
    a.iter().filter(|a| matches!(a, Action::SaveProgress { .. })).count()
}

fn session() -> (PlayerSession, Instant) {
    let t0 = Instant::now();
    let mut s = PlayerSession::new(target(), PlayerData::default(), t0);
    s.start();
    (s, t0)
}

#[test]
fn salva_no_max_a_cada_5s() {
    let (mut s, t0) = session();
    let mut n = 0;
    for i in 0..10 {
        n += saves(&s.on_state(&playing(10.0 + i as f64, 1400.0), t0 + Duration::from_secs(i)));
    }
    assert_eq!(n, 1, "só aos 5 s");
}

#[test]
fn pausa_forca_salvar() {
    let (mut s, t0) = session();
    s.on_state(&playing(10.0, 1400.0), t0);
    let mut p = playing(11.0, 1400.0);
    p.status = Status::Paused;
    assert_eq!(saves(&s.on_state(&p, t0 + Duration::from_millis(500))), 1);
}

#[test]
fn retoma_se_nao_assistido() {
    let mut data = PlayerData::default();
    data.progress.insert("Animes/Show/Show - 01.mkv".into(), Progress { position: 300.0, duration: 1400.0, watched: false, updated_at: 0 });
    let mut s = PlayerSession::new(target(), data, Instant::now());
    match s.start() {
        Action::Load { path, start, .. } => {
            assert_eq!(path, "Animes/Show/Show - 01.mkv");
            assert_eq!(start, Some(300.0));
        }
        a => panic!("{a:?}"),
    }
}

#[test]
fn skip_por_capitulo_abertura() {
    let (mut s, t0) = session();
    let mut st = playing(5.0, 1400.0);
    st.chapters = vec![Chapter { time: 0.0, title: "Opening".into() }, Chapter { time: 90.0, title: "Parte A".into() }];
    let acts = s.on_state(&st, t0);
    assert!(!acts.iter().any(|a| matches!(a, Action::ResolveSkip { .. })), "capítulo resolve sem AniSkip");
    assert!(s.show_skip());
    assert_eq!(s.skip_intro(), Some(90.0));
}

#[test]
fn sem_capitulo_pede_aniskip() {
    let (mut s, t0) = session();
    let acts = s.on_state(&playing(5.0, 1400.0), t0);
    assert!(acts.iter().any(|a| matches!(a, Action::ResolveSkip { mal: 1, ep: 1, .. })));
}

#[test]
fn botao_skip_some_1s_antes_do_fim() {
    let (mut s, t0) = session();
    s.on_state(&playing(5.0, 1400.0), t0);
    s.set_skip("Show - 01.mkv", SkipTimes { op: Some(Interval { start: 0.0, end: 90.0 }), ed: None });
    s.on_state(&playing(88.9, 1400.0), t0);
    assert!(s.show_skip());
    s.on_state(&playing(89.0, 1400.0), t0);
    assert!(!s.show_skip());
}

#[test]
fn contagem_pausa_com_video() {
    let (mut s, t0) = session();
    s.on_state(&playing(1396.0, 1400.0), t0);
    assert_eq!(s.countdown(), Some((5.0, 5.0)));
    let mut p = playing(1396.0, 1400.0);
    p.status = Status::Paused;
    s.on_state(&p, t0 + Duration::from_secs(3));
    assert_eq!(s.countdown(), Some((5.0, 5.0)));
}

#[test]
fn contagem_zero_avanca() {
    let (mut s, t0) = session();
    s.on_state(&playing(1396.0, 1400.0), t0);
    s.on_state(&playing(1397.0, 1400.0), t0 + Duration::from_secs(2));
    let acts = s.on_state(&playing(1399.0, 1400.0), t0 + Duration::from_secs(6));
    assert!(acts.iter().any(|a| matches!(a, Action::Load { path, .. } if path.ends_with("02.mkv"))));
}

#[test]
fn eof_avanca_uma_vez() {
    let (mut s, t0) = session();
    s.on_state(&playing(1390.0, 1400.0), t0);
    let mut end = playing(1400.0, 1400.0);
    end.status = Status::Ended;
    let a = s.on_state(&end, t0);
    let b = s.on_state(&end, t0);
    let loads = a.iter().chain(&b).filter(|a| matches!(a, Action::Load { .. })).count();
    assert_eq!(loads, 1);
}

#[test]
fn cancelar_impede_eof() {
    let (mut s, t0) = session();
    s.on_state(&playing(1396.0, 1400.0), t0);
    s.cancel_next();
    let mut end = playing(1400.0, 1400.0);
    end.status = Status::Ended;
    assert!(!s.on_state(&end, t0).iter().any(|a| matches!(a, Action::Load { .. })));
    assert_eq!(s.countdown(), None);
}

#[test]
fn retentativas_2_4_8_16_30_depois_fatal() {
    let (mut s, t0) = session();
    s.on_state(&playing(42.0, 1400.0), t0);
    let mut now = t0;
    for (i, wait) in [2u64, 4, 8, 16, 30].into_iter().enumerate() {
        let st = failed(i as u32 + 1);
        assert!(s.on_state(&st, now).iter().all(|a| !matches!(a, Action::Load { .. })));
        assert!(s.net_error());
        now += Duration::from_millis(wait * 1000 - 1);
        assert!(s.on_state(&st, now).iter().all(|a| !matches!(a, Action::Load { .. })), "cedo demais ({wait}s)");
        now += Duration::from_millis(1);
        let loads: Vec<_> = s.on_state(&st, now).into_iter().filter_map(|a| match a {
            Action::Load { start, .. } => Some(start),
            _ => None,
        }).collect();
        assert_eq!(loads, vec![Some(42.0)], "tentativa {i}");
    }
    let st = failed(6);
    s.on_state(&st, now);
    assert!(s.fatal());
    assert!(!s.net_error());
}

#[test]
fn opcoes_sem_superficie() {
    let o = options::mpv(&Default::default());
    let keys: Vec<&str> = o.pairs().map(|(k, _)| k).collect();
    assert!(!keys.contains(&"wid") && !keys.contains(&"force-window"));
    assert!(o.pairs().any(|p| p == ("vo", "libmpv")));
    assert!(o.pairs().any(|p| p == ("hwdec", "auto-copy")));
    assert!(o.pairs().any(|p| p == ("demuxer-max-bytes", "100MiB")));
}

#[test]
fn fechar_salva_progresso() {
    let (mut s, t0) = session();
    s.on_state(&playing(42.0, 1400.0), t0);
    match s.close(t0) {
        Some(Action::SaveProgress { progress, .. }) => assert_eq!(progress.position, 42.0),
        a => panic!("{a:?}"),
    }
}

#[test]
fn fechar_carregando_sem_duracao_nao_salva() {
    let (mut s, t0) = session();
    let mut st = PlayerState::default();
    st.status = Status::Loading;
    s.on_state(&st, t0);
    assert_eq!(s.close(t0), None);
}

#[test]
fn contagem_e_eof_no_mesmo_render_avanca_uma_vez() {
    let (mut s, t0) = session3();
    s.on_state(&playing(1396.0, 1400.0), t0);
    let mut end = playing(1400.0, 1400.0);
    end.status = Status::Ended;
    let acts = s.on_state(&end, t0 + Duration::from_secs(6));
    let loads: Vec<_> = acts.iter().filter_map(|a| match a {
        Action::Load { path, .. } => Some(path.clone()),
        _ => None,
    }).collect();
    assert_eq!(loads, vec!["Animes/Show/Show - 02.mkv".to_string()]);
    assert!(!acts.iter().any(|a| matches!(a, Action::SaveProgress { path, .. } if path.ends_with("02.mkv"))), "02 não pode ser salvo");
}

#[test]
fn retomada_nao_perde_posicao_antes_do_time_pos() {
    let mut data = PlayerData::default();
    data.progress.insert("Animes/Show/Show - 01.mkv".into(), Progress { position: 300.0, duration: 1400.0, watched: false, updated_at: 0 });
    let t0 = Instant::now();
    let mut s = PlayerSession::new(target(), data, t0);
    s.start();
    let acts = s.on_state(&playing(0.0, 1400.0), t0 + Duration::from_secs(10));
    assert!(!acts.iter().any(|a| matches!(a, Action::SaveProgress { progress, .. } if progress.position == 0.0)));
    assert_eq!(s.pos(), 300.0);
}

fn session3() -> (PlayerSession, Instant) {
    let mut t = target();
    t.files.push(file("Show - 03.mkv"));
    let t0 = Instant::now();
    let mut s = PlayerSession::new(t, PlayerData::default(), t0);
    s.start();
    (s, t0)
}

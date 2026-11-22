//! Player com libmpv real (render sw): quadro, estado, erro e descarte.
use higanbana_gpui::video::{Player, Status};
use higanbana_gpui::Waker;
use std::time::{Duration, Instant};

const RED: &str = "av://lavfi:color=c=red:size=64x48:rate=10";

fn until(secs: u64, mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn plays_lavfi_and_delivers_frame() {
    let mut p = Player::new(Waker::noop()).expect("libmpv");
    p.load(RED);
    assert!(until(5, || p.frame().is_some()), "nenhum quadro em 5 s");
    let img = p.frame_source().unwrap();
    assert_eq!(img.size(), (64, 48));
    let px = p.last_pixel(32, 24).unwrap();
    assert!(px[0] > 200 && px[1] < 40 && px[2] < 40 && px[3] == 255, "{px:?}");
    assert!(until(3, || p.state().video_size == Some((64, 48))));
    assert!(matches!(p.state().status, Status::Playing));
}

#[test]
fn pause_and_seek_update_state() {
    let p = Player::new(Waker::noop()).expect("libmpv");
    p.load("av://lavfi:testsrc=size=64x48:rate=10:duration=30");
    assert!(until(5, || p.state().duration > 1.0), "duração");
    p.pause();
    assert!(until(3, || matches!(p.state().status, Status::Paused)));
    p.seek(10.0);
    assert!(until(3, || (p.state().position - 10.0).abs() < 0.6), "pos {}", p.state().position);
    p.toggle_pause();
    assert!(until(3, || matches!(p.state().status, Status::Playing)));
}

#[test]
fn invalid_file_reports_error() {
    let p = Player::new(Waker::noop()).expect("libmpv");
    p.load("C:/nao/existe/video.mkv");
    assert!(until(5, || matches!(p.state().status, Status::Error(_))), "{:?}", p.state().status);
}

#[test]
fn drop_while_playing() {
    let t = Instant::now();
    for _ in 0..5 {
        let mut p = Player::new(Waker::noop()).expect("libmpv");
        p.load(RED);
        until(3, || p.frame().is_some());
        drop(p);
    }
    assert!(t.elapsed() < Duration::from_secs(20));
}

#[test]
fn headless_tira_screenshot() {
    use higanbana_gpui::video::PlayerOptions;
    let opts = PlayerOptions::empty().set("vo", "null").set("pause", "yes").set("keep-open", "yes").set("audio", "no");
    let p = Player::headless(opts).expect("libmpv");
    p.load("av://lavfi:color=c=red:size=64x36:rate=10:duration=2");
    assert!(until(5, || matches!(p.state().status, Status::Paused | Status::Playing)), "não carregou");
    let out = std::env::temp_dir().join(format!("hg-thumb-{}.jpg", std::process::id()));
    p.command_blocking(&["seek", "1", "absolute+keyframes"]).expect("seek");
    p.command_blocking(&["screenshot-to-file", out.to_str().unwrap(), "video"]).expect("screenshot");
    let len = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    let _ = std::fs::remove_file(&out);
    assert!(len > 0, "arquivo vazio");
}

const BLUE: &str = "av://lavfi:color=c=blue:size=64x48:rate=10";

fn blue_frame(p: &mut Player) -> bool {
    p.frame();
    p.state().first_frame && p.last_pixel(32, 24).is_some_and(|px| px[2] > 200 && px[0] < 40)
}

#[test]
fn troca_de_arquivo_volta_a_ter_quadro() {
    let mut p = Player::new(Waker::noop()).expect("libmpv");
    p.load_at(RED, None);
    assert!(until(5, || { p.frame(); p.state().first_frame }), "primeiro arquivo sem quadro");
    p.load_at(BLUE, None);
    assert!(!p.state().first_frame, "load zera o sinal");
    assert!(until(5, || blue_frame(&mut p)), "segundo arquivo sem quadro: {:?}", p.state().first_frame);
}

#[test]
fn dois_loads_seguidos_mostram_o_ultimo() {
    let mut p = Player::new(Waker::noop()).expect("libmpv");
    p.load_at(RED, None);
    assert!(until(5, || { p.frame(); p.state().first_frame }));
    p.load_at(RED, None);
    p.load_at(BLUE, None);
    assert!(until(5, || blue_frame(&mut p)), "último arquivo sem quadro");
}

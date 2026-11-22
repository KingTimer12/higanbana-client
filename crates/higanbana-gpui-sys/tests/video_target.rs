//! Alvo de vídeo: OpenGL (outra thread) escreve numa textura D3D11 que a UI desenha.
mod common;
use common::*;
use higanbana_gpui_sys::*;
use std::ffi::{c_void, CStr};
use std::ptr;

struct Ptr(*mut hg_video_target);
unsafe impl Send for Ptr {}

type ClearColor = unsafe extern "system" fn(f32, f32, f32, f32);
type Clear = unsafe extern "system" fn(u32);
const GL_COLOR_BUFFER_BIT: u32 = 0x4000;

fn proc(t: *mut hg_video_target, name: &CStr) -> *mut c_void {
    let p = unsafe { hg_video_target_gl_proc(t, name.as_ptr()) };
    assert!(!p.is_null(), "sem {name:?}");
    p
}

/// Numa thread nova: inicia o GL, limpa o alvo w×h com `rgb` e publica. `Err` = sem interop.
fn paint(t: *mut hg_video_target, w: u32, h: u32, rgb: [f32; 3]) -> Result<(), String> {
    let tp = Ptr(t);
    std::thread::spawn(move || unsafe {
        let tp = tp;
        let t = tp.0;
        if hg_video_target_gl_init(t) != OK {
            return Err(CStr::from_ptr(hg_last_error()).to_string_lossy().into_owned());
        }
        let clear_color: ClearColor = std::mem::transmute(proc(t, c"glClearColor"));
        let clear: Clear = std::mem::transmute(proc(t, c"glClear"));
        let mut fbo = 0;
        assert_eq!(hg_video_target_begin(t, w, h, &mut fbo), OK, "{:?}", CStr::from_ptr(hg_last_error()));
        assert_ne!(fbo, 0);
        clear_color(rgb[0], rgb[1], rgb[2], 1.0);
        clear(GL_COLOR_BUFFER_BIT);
        assert_eq!(hg_video_target_end(t), OK);
        hg_video_target_gl_release(t);
        Ok(())
    })
    .join()
    .unwrap()
}

fn pixel(f: &Fixture, x: u32, y: u32) -> u32 {
    let clear = hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    let mut px = 0;
    unsafe {
        assert_eq!(hg_frame_begin(f.win, clear), OK);
        assert_eq!(hg_frame_submit(f.win, f.dl), OK, "{:?}", CStr::from_ptr(hg_last_error()));
        assert_eq!(hg__window_read_pixel(f.win, x, y, &mut px), OK);
        assert_eq!(hg_frame_end(f.win, 0), OK);
    }
    px & 0x00ff_ffff
}

fn draw(f: &Fixture, img: *mut hg_image) {
    let d = hg_image_draw {
        rect: hg_rect { x: 0.0, y: 0.0, w: 40.0, h: 40.0 },
        uv: hg_rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 },
        tint: hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
        radius: [0.0; 4],
    };
    unsafe { assert_eq!(hg_draw_image(f.dl, img, &d), OK) };
}

#[test]
fn gl_limpa_vermelho_e_d3d11_ve() {
    let f = Fixture::new();
    let mut t = ptr::null_mut();
    assert_eq!(unsafe { hg_video_target_create(f.ctx, &mut t) }, OK, "{:?}", unsafe { CStr::from_ptr(hg_last_error()) });
    let (mut img, mut fresh) = (ptr::null_mut(), 0);
    assert_eq!(unsafe { hg_video_target_image(t, &mut img, &mut fresh) }, OK);
    assert!(img.is_null(), "nada publicado ainda");
    if let Err(e) = paint(t, 64, 48, [1.0, 0.0, 0.0]) {
        eprintln!("SKIP: sem interop OpenGL→D3D11 nesta máquina: {e}");
        unsafe { hg_video_target_destroy(t) };
        return;
    }
    assert_eq!(unsafe { hg_video_target_image(t, &mut img, &mut fresh) }, OK);
    assert!(!img.is_null() && fresh == 1);
    let (mut w, mut h) = (0, 0);
    unsafe { hg_image_size(img, &mut w, &mut h) };
    assert_eq!((w, h), (64, 48));
    draw(&f, img);
    assert_eq!(pixel(&f, 20, 20), 0x00ff_0000, "vermelho do GL na tela D3D11");
    assert_eq!(unsafe { hg_video_target_image(t, &mut img, &mut fresh) }, OK);
    assert_eq!(fresh, 0, "sem quadro novo");
    unsafe { hg_video_target_destroy(t) };
}

#[test]
fn quadro_novo_troca_a_textura_e_o_tamanho() {
    let f = Fixture::new();
    let mut t = ptr::null_mut();
    assert_eq!(unsafe { hg_video_target_create(f.ctx, &mut t) }, OK);
    if paint(t, 64, 48, [1.0, 0.0, 0.0]).is_err() {
        unsafe { hg_video_target_destroy(t) };
        return;
    }
    paint(t, 32, 32, [0.0, 0.0, 1.0]).unwrap();
    let (mut img, mut fresh) = (ptr::null_mut(), 0);
    assert_eq!(unsafe { hg_video_target_image(t, &mut img, &mut fresh) }, OK);
    let (mut w, mut h) = (0, 0);
    unsafe { hg_image_size(img, &mut w, &mut h) };
    assert_eq!((w, h), (32, 32));
    draw(&f, img);
    assert_eq!(pixel(&f, 20, 20), 0x0000_00ff, "azul do segundo quadro");
    unsafe { hg_video_target_destroy(t) };
}

#[test]
fn destroy_libera_gpu_bytes() {
    let f = Fixture::new();
    let stats = || {
        f.frame();
        let mut s = unsafe { std::mem::zeroed::<hg_stats>() };
        unsafe { hg_context_stats(f.ctx, &mut s) };
        s.gpu_bytes
    };
    let before = stats();
    let mut t = ptr::null_mut();
    assert_eq!(unsafe { hg_video_target_create(f.ctx, &mut t) }, OK);
    if paint(t, 256, 256, [0.0, 1.0, 0.0]).is_err() {
        unsafe { hg_video_target_destroy(t) };
        return;
    }
    let (mut img, mut fresh) = (ptr::null_mut(), 0);
    assert_eq!(unsafe { hg_video_target_image(t, &mut img, &mut fresh) }, OK);
    unsafe { hg_video_target_destroy(t) };
    assert_eq!(stats(), before, "texturas do alvo liberadas");
}

#[test]
fn sem_d3d11_fica_indisponivel() {
    let mut ctx = ptr::null_mut();
    if unsafe { hg_context_create(hg_backend_kind_HG_BACKEND_OPENGL, &mut ctx) } != OK {
        return;
    }
    let mut t = ptr::null_mut();
    assert_eq!(unsafe { hg_video_target_create(ctx, &mut t) }, HG_ERR_UNAVAILABLE as i32);
    assert!(t.is_null());
    unsafe { hg_context_destroy(ctx) };
}

// ---- Correções da revisão final ----------------------------------------

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumThreadWindows(thread: u32, f: unsafe extern "system" fn(*mut c_void, isize) -> i32, l: isize) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
}

unsafe extern "system" fn count_window(_: *mut c_void, l: isize) -> i32 {
    unsafe { *(l as *mut u32) += 1 };
    1
}

enum Cmd {
    Paint(u32, u32, [f32; 3]),
    Windows,
    Stop,
}

/// Thread de render que mantém o GL entre quadros (como a do mpv).
struct GlThread {
    tx: std::sync::mpsc::Sender<Cmd>,
    rx: std::sync::mpsc::Receiver<u32>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl GlThread {
    fn start(t: *mut hg_video_target) -> Option<GlThread> {
        let (tx, crx) = std::sync::mpsc::channel::<Cmd>();
        let (ctx_, rx) = std::sync::mpsc::channel::<u32>();
        let tp = Ptr(t);
        let join = std::thread::spawn(move || unsafe {
            let tp = tp;
            let t = tp.0;
            if hg_video_target_gl_init(t) != OK {
                let _ = ctx_.send(0);
                return;
            }
            let _ = ctx_.send(1);
            let clear_color: ClearColor = std::mem::transmute(proc(t, c"glClearColor"));
            let clear: Clear = std::mem::transmute(proc(t, c"glClear"));
            for cmd in crx {
                match cmd {
                    Cmd::Paint(w, h, rgb) => {
                        let mut fbo = 0;
                        assert_eq!(hg_video_target_begin(t, w, h, &mut fbo), OK);
                        clear_color(rgb[0], rgb[1], rgb[2], 1.0);
                        clear(GL_COLOR_BUFFER_BIT);
                        assert_eq!(hg_video_target_end(t), OK);
                        let _ = ctx_.send(1);
                    }
                    Cmd::Windows => {
                        let mut n = 0u32;
                        EnumThreadWindows(GetCurrentThreadId(), count_window, &mut n as *mut u32 as isize);
                        let _ = ctx_.send(n);
                    }
                    Cmd::Stop => break,
                }
            }
            hg_video_target_gl_release(t);
        });
        let g = GlThread { tx, rx, join: Some(join) };
        (g.rx.recv().ok()? == 1).then_some(g)
    }

    fn ask(&self, c: Cmd) -> u32 {
        self.tx.send(c).unwrap();
        self.rx.recv().unwrap()
    }
}

impl Drop for GlThread {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Stop);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

#[test]
fn thread_de_render_sem_janela_que_recebe_broadcast() {
    let f = Fixture::new();
    let mut t = ptr::null_mut();
    assert_eq!(unsafe { hg_video_target_create(f.ctx, &mut t) }, OK);
    let Some(g) = GlThread::start(t) else { return };
    // Janela de topo numa thread que não processa mensagens trava quem faz broadcast.
    assert_eq!(g.ask(Cmd::Windows), 0, "janela escondida deve ser só de mensagens");
    drop(g);
    unsafe { hg_video_target_destroy(t) };
}

#[test]
fn slot_que_a_ui_mostra_nao_e_reescrito() {
    let f = Fixture::new();
    let mut t = ptr::null_mut();
    assert_eq!(unsafe { hg_video_target_create(f.ctx, &mut t) }, OK);
    let Some(g) = GlThread::start(t) else { return };
    let (mut img, mut fresh) = (ptr::null_mut(), 0);
    g.ask(Cmd::Paint(64, 48, [1.0, 0.0, 0.0]));
    assert_eq!(unsafe { hg_video_target_image(t, &mut img, &mut fresh) }, OK);
    // Dois quadros sem a UI pegar nenhum: o que ela mostra (vermelho) fica intacto.
    g.ask(Cmd::Paint(64, 48, [0.0, 0.0, 1.0]));
    g.ask(Cmd::Paint(64, 48, [0.0, 1.0, 0.0]));
    draw(&f, img);
    assert_eq!(pixel(&f, 20, 20), 0x00ff_0000, "UI redesenha o quadro que já tinha");
    assert_eq!(unsafe { hg_video_target_image(t, &mut img, &mut fresh) }, OK);
    assert_eq!(fresh, 1);
    draw(&f, img);
    assert_eq!(pixel(&f, 20, 20), 0x0000_ff00, "e depois pega o último");
    drop(g);
    unsafe { hg_video_target_destroy(t) };
}

#[test]
fn troca_de_backend_acusa_erro_em_vez_de_usar_o_device_velho() {
    let f = Fixture::new();
    let mut t = ptr::null_mut();
    assert_eq!(unsafe { hg_video_target_create(f.ctx, &mut t) }, OK);
    let Some(g) = GlThread::start(t) else { return };
    let (mut img, mut fresh) = (ptr::null_mut(), 0);
    g.ask(Cmd::Paint(64, 48, [1.0, 0.0, 0.0]));
    assert_eq!(unsafe { hg_video_target_image(t, &mut img, &mut fresh) }, OK);
    if unsafe { hg_context_switch_backend(f.ctx, hg_backend_kind_HG_BACKEND_OPENGL) } != OK {
        return;
    }
    // Pausado (sem quadro novo): mesmo assim a UI fica sabendo.
    assert_ne!(unsafe { hg_video_target_image(t, &mut img, &mut fresh) }, OK, "imagem sem textura no backend novo");
    assert_eq!(unsafe { hg_context_switch_backend(f.ctx, hg_backend_kind_HG_BACKEND_D3D11) }, OK);
    g.ask(Cmd::Paint(64, 48, [0.0, 0.0, 1.0]));
    assert_ne!(unsafe { hg_video_target_image(t, &mut img, &mut fresh) }, OK, "textura do device D3D11 antigo");
    drop(g);
    unsafe { hg_video_target_destroy(t) };
}

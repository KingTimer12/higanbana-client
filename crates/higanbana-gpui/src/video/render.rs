//! Thread de render: a render API "sw" do mpv escreve cada quadro (RGB0) num buffer na RAM;
//! o quadro pronto vai para um slot compartilhado e a UI é acordada.
use super::api::*;
use super::gpu::GpuLink;
use super::render_gl::GlRender;
use super::state::PlayerState;
use crate::gfx::Waker;
use std::ffi::c_void;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

/// Maior quadro renderizado (a GPU escala o resto); mantém a proporção.
const MAX_W: u32 = 2560;
const MAX_H: u32 = 1440;

/// Quadro pronto: RGBA8 opaco (alfa 255), linhas contíguas.
pub(crate) struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub(crate) type Slot = Arc<Mutex<Option<Frame>>>;

/// Buffers já enviados à GPU, devolvidos pela UI para o próximo quadro.
pub(crate) type Pool = Arc<Mutex<Vec<Vec<u8>>>>;

/// Quantos buffers o pool guarda (um em uso na UI, um sendo desenhado).
pub(crate) const POOL_MAX: usize = 2;

/// Devolve um buffer ao pool (descarta se já estiver cheio).
pub(crate) fn give_back(pool: &Pool, v: Vec<u8>) {
    let mut p = pool.lock().unwrap_or_else(|e| e.into_inner());
    if p.len() < POOL_MAX {
        p.push(v);
    }
}

/// Buffer de `len` bytes: do pool se houver, senão alocado.
pub(crate) fn take_buffer(pool: &Pool, len: usize) -> Vec<u8> {
    let mut v = pool.lock().unwrap_or_else(|e| e.into_inner()).pop().unwrap_or_default();
    v.resize(len, 0);
    v
}

#[derive(Default)]
struct Flags {
    pending: bool,
    stop: bool,
}

#[derive(Default)]
struct Signal {
    flags: Mutex<Flags>,
    cv: Condvar,
}

impl Signal {
    fn raise(&self, f: impl FnOnce(&mut Flags)) {
        f(&mut self.flags.lock().unwrap_or_else(|e| e.into_inner()));
        self.cv.notify_one();
    }
}

/// Chamado por threads do mpv quando há quadro novo.
unsafe extern "C" fn on_update(ctx: *mut c_void) {
    let signal = unsafe { &*(ctx as *const Signal) };
    signal.raise(|f| f.pending = true);
}

/// Handle do mpv compartilhado entre threads (a client API é thread-safe).
#[derive(Clone, Copy)]
pub(crate) struct SharedHandle(pub Handle);
unsafe impl Send for SharedHandle {}
unsafe impl Sync for SharedHandle {}

pub(crate) struct RenderThread {
    signal: Arc<Signal>,
    join: Option<JoinHandle<()>>,
}

impl RenderThread {
    /// Cria o contexto de render na thread nova. Falha se a render API recusar.
    pub fn start(handle: SharedHandle, state: Arc<Mutex<PlayerState>>, slot: Slot, pool: Pool, link: Option<Arc<GpuLink>>, waker: Waker) -> Result<Self, String> {
        let signal = Arc::new(Signal::default());
        let (tx, rx) = std::sync::mpsc::channel();
        let sig = signal.clone();
        let join = std::thread::Builder::new()
            .name("mpv-render".into())
            .spawn(move || run(handle, sig, state, slot, pool, link, waker, tx))
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|_| "thread de render do mpv morreu".to_string())??;
        Ok(RenderThread { signal, join: Some(join) })
    }
}

impl Drop for RenderThread {
    /// Para a thread e espera: o contexto de render é liberado antes do mpv ser destruído.
    fn drop(&mut self) {
        self.signal.raise(|f| f.stop = true);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn create_context(api: &Api, handle: Handle, signal: &Arc<Signal>) -> Result<*mut c_void, String> {
    let mut params = [
        RenderParam { kind: RENDER_PARAM_API_TYPE, data: c"sw".as_ptr() as *mut c_void },
        RenderParam { kind: RENDER_PARAM_INVALID, data: std::ptr::null_mut() },
    ];
    let mut ctx = std::ptr::null_mut();
    let code = unsafe { (api.render_create)(&mut ctx, handle, params.as_mut_ptr()) };
    if code < 0 {
        return Err(format!("render sw do mpv: {}", error_text(code)));
    }
    unsafe { (api.render_set_update_callback)(ctx, Some(on_update), Arc::as_ptr(signal) as *mut c_void) };
    Ok(ctx)
}

/// Tamanho do quadro: o do vídeo, reduzido para caber em MAX_W×MAX_H e na área
/// em que aparece na tela (`view`, pixels físicos). Nunca amplia.
pub(crate) fn target_size(video: (u32, u32), view: Option<(u32, u32)>) -> (u32, u32) {
    let (w, h) = video;
    let mut k = (MAX_W as f64 / w as f64).min(MAX_H as f64 / h as f64).min(1.0);
    if let Some((vw, vh)) = view.filter(|(vw, vh)| *vw > 0 && *vh > 0) {
        k = k.min(vw as f64 / w as f64).min(vh as f64 / h as f64);
    }
    (((w as f64 * k).round() as u32).max(1), ((h as f64 * k).round() as u32).max(1))
}

fn render_frame(api: &Api, ctx: *mut c_void, (w, h): (u32, u32), pool: &Pool) -> Option<Frame> {
    let stride = w as usize * 4;
    let mut pixels = take_buffer(pool, stride * h as usize);
    let mut size = [w as i32, h as i32];
    let mut stride_v = stride;
    let mut params = [
        RenderParam { kind: RENDER_PARAM_SW_SIZE, data: size.as_mut_ptr() as *mut c_void },
        RenderParam { kind: RENDER_PARAM_SW_FORMAT, data: c"rgba".as_ptr() as *mut c_void },
        RenderParam { kind: RENDER_PARAM_SW_STRIDE, data: &mut stride_v as *mut usize as *mut c_void },
        RenderParam { kind: RENDER_PARAM_SW_POINTER, data: pixels.as_mut_ptr() as *mut c_void },
        RenderParam { kind: RENDER_PARAM_INVALID, data: std::ptr::null_mut() },
    ];
    // SAFETY: buffer de stride*h bytes vivo até o fim da chamada.
    if unsafe { (api.render_render)(ctx, params.as_mut_ptr()) } < 0 {
        return None;
    }
    Some(Frame { width: w, height: h, pixels })
}

/// Contexto de render atual: CPU ("sw") ou GPU (OpenGL no alvo D3D11).
enum Mode {
    Sw(*mut c_void),
    Gl(GlRender),
}

impl Mode {
    fn ctx(&self) -> *mut c_void {
        match self {
            Mode::Sw(c) => *c,
            Mode::Gl(g) => g.ctx(),
        }
    }

    fn free(self, api: &Api) {
        match self {
            // SAFETY: único dono; nenhuma chamada de render em andamento.
            Mode::Sw(c) => unsafe { (api.render_free)(c) },
            Mode::Gl(g) => g.free(api),
        }
    }
}

/// Primeiro contexto: OpenGL no alvo se o player tem um (antes de qualquer `load`,
/// para a saída de vídeo do mpv já nascer nele), senão "sw".
fn first_mode(api: &Api, handle: Handle, signal: &Arc<Signal>, link: Option<&GpuLink>) -> Result<Mode, String> {
    if let Some(link) = link
        && let Some(target) = link.pending_target()
    {
        let update = (on_update as UpdateFn, Arc::as_ptr(signal) as *mut c_void);
        match GlRender::start(api, handle, target, update) {
            Ok(g) => {
                link.set_active(true);
                return Ok(Mode::Gl(g));
            }
            Err(e) => {
                eprintln!("higanbana-gpui: vídeo na GPU indisponível, segue na CPU: {e}");
                link.fail();
            }
        }
    }
    create_context(api, handle, signal).map(Mode::Sw)
}

/// GPU que falhou (quadro ou backend trocado): volta ao contexto "sw".
fn switch_mode(api: &Api, handle: Handle, signal: &Arc<Signal>, link: &GpuLink, mode: Mode) -> Result<Mode, String> {
    match mode {
        Mode::Gl(g) if link.failed() => {
            // A saída de vídeo do mpv não sobrevive à troca de contexto: fecha a trilha de
            // vídeo antes, solta o GL, cria o "sw" e reabre a trilha nele (o áudio segue).
            let h = SharedHandle(handle);
            super::player::command_raw(h, &["set", "vid", "no"]);
            g.free(api);
            link.set_active(false);
            let sw = create_context(api, handle, signal).map(Mode::Sw)?;
            super::player::command_raw(h, &["set", "vid", "auto"]);
            Ok(sw)
        }
        m => Ok(m),
    }
}

#[allow(clippy::too_many_arguments)]
fn run(handle: SharedHandle, signal: Arc<Signal>, state: Arc<Mutex<PlayerState>>, slot: Slot, pool: Pool, link: Option<Arc<GpuLink>>, waker: Waker, ready: std::sync::mpsc::Sender<Result<(), String>>) {
    let Ok(api) = api() else {
        let _ = ready.send(Err("libmpv não carregada".into()));
        return;
    };
    let mut mode = match first_mode(api, handle.0, &signal, link.as_deref()) {
        Ok(m) => m,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let _ = ready.send(Ok(()));
    let mut retries = 0u32;
    loop {
        {
            let mut f = signal.flags.lock().unwrap_or_else(|e| e.into_inner());
            while !f.pending && !f.stop {
                f = signal.cv.wait(f).unwrap_or_else(|e| e.into_inner());
            }
            if f.stop {
                break;
            }
            f.pending = false;
        }
        let mut switched = false;
        if let Some(link) = &link {
            let was_gl = matches!(mode, Mode::Gl(_));
            mode = match switch_mode(api, handle.0, &signal, link, mode) {
                Ok(m) => {
                    // Contexto novo não avisa quadro: sem um render logo, o mpv dá a saída
                    // de vídeo por perdida (~400 ms) e encerra o arquivo.
                    switched = was_gl && matches!(m, Mode::Sw(_));
                    m
                }
                Err(e) => {
                    eprintln!("higanbana-gpui: render do mpv parou: {e}");
                    return;
                }
            };
        }
        let (video, view) = {
            let s = state.lock().unwrap_or_else(|e| e.into_inner());
            (s.video_size, s.view_size)
        };
        let Some(video) = video.filter(|(w, h)| *w > 0 && *h > 0) else {
            // Quadro antes de `dwidth/dheight` chegarem: tenta de novo por até ~1 s
            // (sem vídeo, ex. só áudio, desiste e volta a dormir).
            if retries < 60 {
                retries += 1;
                std::thread::sleep(std::time::Duration::from_millis(15));
                signal.raise(|f| f.pending = true);
            } else {
                unsafe { (api.render_update)(mode.ctx()) };
            }
            continue;
        };
        retries = 0;
        if unsafe { (api.render_update)(mode.ctx()) } & RENDER_UPDATE_FRAME == 0 && !switched {
            continue;
        }
        let size = target_size(video, view);
        let delivered = match &mode {
            Mode::Sw(ctx) => match render_frame(api, *ctx, size, &pool) {
                Some(frame) => {
                    // Quadro que a UI não chegou a pegar: o buffer volta ao pool.
                    if let Some(old) = slot.lock().unwrap_or_else(|e| e.into_inner()).replace(frame) {
                        give_back(&pool, old.pixels);
                    }
                    true
                }
                None => false,
            },
            Mode::Gl(g) => match g.render(api, size) {
                Ok(()) => true,
                Err(e) => {
                    eprintln!("higanbana-gpui: quadro na GPU falhou, volta à CPU: {e}");
                    if let Some(l) = &link {
                        l.fail();
                    }
                    signal.raise(|f| f.pending = true);
                    false
                }
            },
        };
        if delivered {
            // Primeiro quadro desde o último load: vem daqui (o `vo-configured` não muda
            // na troca de arquivo e deixava o próximo episódio preto).
            state.lock().unwrap_or_else(|e| e.into_inner()).first_frame = true;
            waker.wake();
        }
    }
    mode.free(api);
}

#[cfg(test)]
mod tests {
    use super::target_size;

    #[test]
    fn target_size_caps_and_keeps_ratio() {
        assert_eq!(target_size((1280, 720), None), (1280, 720));
        assert_eq!(target_size((3840, 2160), None), (2560, 1440));
        assert_eq!(target_size((4000, 1000), None), (2560, 640));
    }

    #[test]
    fn buffer_do_pool_e_reaproveitado() {
        let pool = super::Pool::default();
        let v: Vec<u8> = Vec::with_capacity(256);
        let ptr = v.as_ptr();
        pool.lock().unwrap().push(v);
        let got = super::take_buffer(&pool, 64);
        assert_eq!((got.len(), got.as_ptr()), (64, ptr));
        // Pool vazio: aloca.
        assert_eq!(super::take_buffer(&pool, 16).len(), 16);
    }

    #[test]
    fn target_size_usa_a_vista() {
        assert_eq!(target_size((1920, 1080), Some((1280, 720))), (1280, 720));
        // Vista maior que o vídeo: tamanho do vídeo.
        assert_eq!(target_size((1280, 720), Some((2560, 1440))), (1280, 720));
        // Proporção do vídeo dentro da vista (4:3 numa vista 16:9).
        assert_eq!(target_size((1440, 1080), Some((1296, 839))), (1119, 839));
        // Vista vazia (janela minimizada) não zera o quadro.
        assert_eq!(target_size((1920, 1080), Some((0, 0))), (1920, 1080));
    }
}

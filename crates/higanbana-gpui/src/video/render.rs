//! Thread de render: a render API "sw" do mpv escreve cada quadro (RGB0) num buffer na RAM;
//! o quadro pronto vai para um slot compartilhado e a UI é acordada.
use super::api::*;
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
    pub fn start(handle: SharedHandle, state: Arc<Mutex<PlayerState>>, slot: Slot, waker: Waker) -> Result<Self, String> {
        let signal = Arc::new(Signal::default());
        let (tx, rx) = std::sync::mpsc::channel();
        let sig = signal.clone();
        let join = std::thread::Builder::new()
            .name("mpv-render".into())
            .spawn(move || run(handle, sig, state, slot, waker, tx))
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

/// Tamanho do quadro: o do vídeo, reduzido para caber em MAX_W×MAX_H.
fn target_size(video: (u32, u32)) -> (u32, u32) {
    let (w, h) = video;
    let k = (MAX_W as f64 / w as f64).min(MAX_H as f64 / h as f64).min(1.0);
    (((w as f64 * k).round() as u32).max(1), ((h as f64 * k).round() as u32).max(1))
}

fn render_frame(api: &Api, ctx: *mut c_void, (w, h): (u32, u32)) -> Option<Frame> {
    let stride = w as usize * 4;
    let mut pixels = vec![0u8; stride * h as usize];
    let mut size = [w as i32, h as i32];
    let mut stride_v = stride;
    let mut params = [
        RenderParam { kind: RENDER_PARAM_SW_SIZE, data: size.as_mut_ptr() as *mut c_void },
        RenderParam { kind: RENDER_PARAM_SW_FORMAT, data: c"rgb0".as_ptr() as *mut c_void },
        RenderParam { kind: RENDER_PARAM_SW_STRIDE, data: &mut stride_v as *mut usize as *mut c_void },
        RenderParam { kind: RENDER_PARAM_SW_POINTER, data: pixels.as_mut_ptr() as *mut c_void },
        RenderParam { kind: RENDER_PARAM_INVALID, data: std::ptr::null_mut() },
    ];
    // SAFETY: buffer de stride*h bytes vivo até o fim da chamada.
    if unsafe { (api.render_render)(ctx, params.as_mut_ptr()) } < 0 {
        return None;
    }
    for px in pixels.as_chunks_mut::<4>().0 {
        px[3] = 255;
    }
    Some(Frame { width: w, height: h, pixels })
}

fn run(handle: SharedHandle, signal: Arc<Signal>, state: Arc<Mutex<PlayerState>>, slot: Slot, waker: Waker, ready: std::sync::mpsc::Sender<Result<(), String>>) {
    let Ok(api) = api() else {
        let _ = ready.send(Err("libmpv não carregada".into()));
        return;
    };
    let ctx = match create_context(api, handle.0, &signal) {
        Ok(c) => c,
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
        let video = state.lock().unwrap_or_else(|e| e.into_inner()).video_size;
        let Some(video) = video.filter(|(w, h)| *w > 0 && *h > 0) else {
            // Quadro antes de `dwidth/dheight` chegarem: tenta de novo por até ~1 s
            // (sem vídeo, ex. só áudio, desiste e volta a dormir).
            if retries < 60 {
                retries += 1;
                std::thread::sleep(std::time::Duration::from_millis(15));
                signal.raise(|f| f.pending = true);
            } else {
                unsafe { (api.render_update)(ctx) };
            }
            continue;
        };
        retries = 0;
        if unsafe { (api.render_update)(ctx) } & RENDER_UPDATE_FRAME == 0 {
            continue;
        }
        if let Some(frame) = render_frame(api, ctx, target_size(video)) {
            *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(frame);
            waker.wake();
        }
    }
    // SAFETY: único dono; nenhuma chamada de render em andamento.
    unsafe { (api.render_free)(ctx) };
}

#[cfg(test)]
mod tests {
    use super::target_size;

    #[test]
    fn target_size_caps_and_keeps_ratio() {
        assert_eq!(target_size((1280, 720)), (1280, 720));
        assert_eq!(target_size((3840, 2160)), (2560, 1440));
        assert_eq!(target_size((4000, 1000)), (2560, 640));
    }
}

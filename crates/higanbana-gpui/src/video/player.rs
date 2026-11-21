//! `Player`: uma instância do mpv, thread de eventos (estado) e thread de render (quadros).
use super::api::*;
use super::render::{RenderThread, SharedHandle, Slot};
use super::state::{PlayerState, Status, OBSERVED};
use super::options::PlayerOptions;
use crate::element::{ImageData, ImageSource};
use crate::gfx::Waker;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Opções aplicadas antes de iniciar: saída pela render API, decodificação por hardware
/// com cópia (o render sw precisa do quadro na CPU), sem teclado/OSC do mpv.
struct Core {
    handle: SharedHandle,
    render: Option<RenderThread>,
    events: Option<JoinHandle<()>>,
}

/// Player de vídeo. Comandos podem ser chamados a qualquer momento; o estado e os
/// quadros chegam de outras threads e acordam a UI pelo `Waker`.
pub struct Player {
    core: Option<Core>,
    state: Arc<Mutex<PlayerState>>,
    slot: Slot,
    image: Option<ImageSource>,
    /// Comandos de um player sem mpv (`detached`), para testes de UI.
    log: RefCell<Vec<String>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Player {
    /// Carrega a libmpv (uma vez por processo) e cria uma instância.
    pub fn new(waker: Waker) -> Result<Player, String> {
        Player::with_options(waker, PlayerOptions::default())
    }

    /// Como `new`, com opções próprias (cache, threads, idioma...).
    pub fn with_options(waker: Waker, options: PlayerOptions) -> Result<Player, String> {
        Player::start(waker, options, true)
    }

    /// Sem quadros (nem thread de render): para tarefas de fundo, como miniaturas
    /// (`vo=null` + `screenshot-to-file`). Crie e use na mesma thread.
    pub fn headless(options: PlayerOptions) -> Result<Player, String> {
        Player::start(Waker::noop(), options, false)
    }

    fn start(waker: Waker, options: PlayerOptions, render: bool) -> Result<Player, String> {
        let api = api()?;
        // SAFETY: handle novo; strings vivem até o fim de cada chamada.
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err("mpv_create falhou".into());
        }
        for (k, v) in options.pairs() {
            let (k, v) = (cstr(k), cstr(v));
            unsafe { (api.set_option_string)(handle, k.as_ptr(), v.as_ptr()) };
        }
        let code = unsafe { (api.initialize)(handle) };
        if code < 0 {
            unsafe { (api.terminate_destroy)(handle) };
            return Err(format!("mpv_initialize: {}", error_text(code)));
        }
        for (name, format) in OBSERVED {
            let n = cstr(name);
            unsafe { (api.observe_property)(handle, 0, n.as_ptr(), *format) };
        }
        let handle = SharedHandle(handle);
        let state = Arc::new(Mutex::new(PlayerState::default()));
        let slot: Slot = Arc::default();
        let render = if render {
            match RenderThread::start(handle, state.clone(), slot.clone(), waker.clone()) {
                Ok(r) => Some(r),
                Err(e) => {
                    unsafe { (api.terminate_destroy)(handle.0) };
                    return Err(e);
                }
            }
        } else {
            None
        };
        let st = state.clone();
        let events = std::thread::Builder::new()
            .name("mpv-events".into())
            .spawn(move || event_loop(handle, st, waker));
        let events = match events {
            Ok(j) => j,
            Err(e) => {
                drop(render);
                unsafe { (api.terminate_destroy)(handle.0) };
                return Err(e.to_string());
            }
        };
        Ok(Player {
            core: Some(Core { handle, render, events: Some(events) }),
            state,
            slot,
            image: None,
            log: RefCell::new(Vec::new()),
        })
    }

    /// Player sem mpv: comandos só são registrados (`commands`). Para testes de UI.
    pub fn detached() -> Player {
        Player { core: None, state: Arc::default(), slot: Arc::default(), image: None, log: RefCell::new(Vec::new()) }
    }

    /// Estado atual (cópia).
    pub fn state(&self) -> PlayerState {
        lock(&self.state).clone()
    }

    /// Testes de UI: substitui o estado.
    pub fn set_state_for_test(&self, s: PlayerState) {
        *lock(&self.state) = s;
    }

    /// Comandos recebidos por um player `detached`.
    pub fn commands(&self) -> Vec<String> {
        self.log.borrow().clone()
    }

    /// Se chegou quadro novo, atualiza a imagem e a devolve.
    pub fn frame(&mut self) -> Option<ImageSource> {
        let f = lock(&self.slot).take()?;
        let img = self.image.get_or_insert_with(|| ImageData::dynamic(f.width, f.height)).clone();
        img.set_frame(f.width, f.height, f.pixels);
        Some(img)
    }

    /// Imagem do vídeo (último quadro), se já houve algum.
    pub fn frame_source(&self) -> Option<ImageSource> {
        self.image.clone()
    }

    /// Pixel RGBA do quadro ainda não enviado à GPU (testes).
    pub fn last_pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        self.image.as_ref()?.pending_pixel(x, y)
    }

    fn command(&self, args: &[&str]) {
        let Some(core) = &self.core else {
            self.log.borrow_mut().push(args.join(" "));
            return;
        };
        command_raw(core.handle, args);
    }

    /// Muda uma propriedade do mpv ao vivo (`set_property_string`).
    pub fn set_property(&self, name: &str, value: &str) {
        self.set(name, value);
    }

    /// `mpv_command` síncrono; devolve o erro do mpv. Bloqueia: fora da thread da UI.
    pub fn command_blocking(&self, args: &[&str]) -> Result<(), String> {
        let Some(core) = &self.core else {
            self.log.borrow_mut().push(args.join(" "));
            return Ok(());
        };
        let code = command_raw(core.handle, args);
        if code < 0 { Err(error_text(code)) } else { Ok(()) }
    }

    fn set(&self, name: &str, value: &str) {
        let Some(core) = &self.core else {
            self.log.borrow_mut().push(format!("set {name} {value}"));
            return;
        };
        let Ok(api) = api() else { return };
        let (n, v) = (cstr(name), cstr(value));
        unsafe { (api.set_property_string)(core.handle.0, n.as_ptr(), v.as_ptr()) };
    }

    /// Abre arquivo ou URL (inclui `av://lavfi:...`).
    pub fn load(&self, path: &str) {
        {
            let mut s = lock(&self.state);
            s.status = Status::Loading;
            s.position = 0.0;
            s.duration = 0.0;
        }
        self.command(&["loadfile", path, "replace"]);
    }

    /// Abre começando em `start` segundos (ignorado se até 1 s).
    pub fn load_at(&self, path: &str, start: Option<f64>) {
        {
            let mut s = lock(&self.state);
            s.status = Status::Loading;
            s.position = 0.0;
            s.duration = 0.0;
            s.first_frame = false;
        }
        match start.filter(|s| *s > 1.0) {
            Some(s) => self.command(&["loadfile", path, "replace", "-1", &format!("start={s:.1}")]),
            None => self.command(&["loadfile", path, "replace", "-1"]),
        }
    }

    pub fn play(&self) {
        self.set("pause", "no");
    }

    pub fn pause(&self) {
        self.set("pause", "yes");
    }

    pub fn toggle_pause(&self) {
        self.command(&["cycle", "pause"]);
    }

    /// Vai para `secs` (absoluto).
    pub fn seek(&self, secs: f64) {
        self.command(&["seek", &format!("{secs:.3}"), "absolute+exact"]);
    }

    /// Seek rápido durante o arraste (só quadros-chave): não enfileira decodificação a cada movimento.
    pub fn seek_preview(&self, secs: f64) {
        self.command(&["seek", &format!("{secs:.3}"), "absolute+keyframes"]);
    }

    pub fn seek_relative(&self, secs: f64) {
        self.command(&["seek", &format!("{secs:.3}"), "relative"]);
    }

    /// 0..=100.
    pub fn set_volume(&self, v: f64) {
        self.set("volume", &format!("{:.1}", v.clamp(0.0, 100.0)));
    }

    pub fn toggle_mute(&self) {
        self.command(&["cycle", "mute"]);
    }

    pub fn set_speed(&self, s: f64) {
        self.set("speed", &format!("{s:.2}"));
    }

    /// `None` desliga.
    pub fn set_audio_track(&self, id: Option<i64>) {
        self.set("aid", &id.map_or("no".into(), |i| i.to_string()));
    }

    /// `None` desliga a legenda.
    pub fn set_sub_track(&self, id: Option<i64>) {
        self.set("sid", &id.map_or("no".into(), |i| i.to_string()));
    }
}

impl Drop for Player {
    /// Ordem: render (libera o contexto) → `quit` → espera a thread de eventos → destrói.
    fn drop(&mut self) {
        let Some(mut core) = self.core.take() else { return };
        drop(core.render.take());
        command_raw(core.handle, &["quit"]);
        if let Some(j) = core.events.take() {
            let _ = j.join();
        }
        if let Ok(api) = api() {
            unsafe { (api.terminate_destroy)(core.handle.0) };
        }
    }
}

/// `mpv_command` com argumentos UTF-8.
fn command_raw(handle: SharedHandle, args: &[&str]) -> std::ffi::c_int {
    let Ok(api) = api() else { return -1 };
    let owned: Vec<_> = args.iter().map(|a| cstr(a)).collect();
    let mut ptrs: Vec<*const std::ffi::c_char> = owned.iter().map(|c| c.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    unsafe { (api.command)(handle.0, ptrs.as_mut_ptr()) }
}

/// Lê eventos até o `quit`; atualiza o estado e acorda a UI.
fn event_loop(handle: SharedHandle, state: Arc<Mutex<PlayerState>>, waker: Waker) {
    let Ok(api) = api() else { return };
    loop {
        // SAFETY: só esta thread chama wait_event; o evento vale até a próxima chamada.
        let ev = unsafe { &*(api.wait_event)(handle.0, -1.0) };
        match ev.event_id {
            EVENT_SHUTDOWN => break,
            EVENT_PROPERTY_CHANGE if !ev.data.is_null() => unsafe { lock(&state).apply(&*(ev.data as *const RawProperty)) },
            EVENT_START_FILE => lock(&state).status = Status::Loading,
            EVENT_FILE_LOADED => lock(&state).file_loaded(),
            EVENT_END_FILE if !ev.data.is_null() => {
                let end = unsafe { &*(ev.data as *const RawEndFile) };
                lock(&state).end_file(end.reason, end.error);
            }
            _ => continue,
        }
        waker.wake();
    }
}

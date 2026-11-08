//! libmpv carregada em tempo de execução (`libmpv-2.dll`), só com o que o player usa.
//!
//! Uma thread por instância espera eventos (`mpv_wait_event`) e os manda por canal;
//! comandos e propriedades são chamados direto (a API do mpv é thread-safe).

use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use libloading::Library;
use serde_json::{Map, Number, Value};

type Handle = *mut c_void;

const FORMAT_NONE: c_int = 0;
const FORMAT_STRING: c_int = 1;
const FORMAT_FLAG: c_int = 3;
const FORMAT_INT64: c_int = 4;
const FORMAT_DOUBLE: c_int = 5;
const FORMAT_NODE: c_int = 6;
const FORMAT_NODE_ARRAY: c_int = 7;
const FORMAT_NODE_MAP: c_int = 8;

const EVENT_SHUTDOWN: c_int = 1;
const EVENT_END_FILE: c_int = 7;
const EVENT_FILE_LOADED: c_int = 8;
const EVENT_PLAYBACK_RESTART: c_int = 21;
const EVENT_PROPERTY_CHANGE: c_int = 22;

#[repr(C)]
struct RawEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

#[repr(C)]
struct RawProperty {
    name: *const c_char,
    format: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct RawEndFile {
    reason: c_int,
    error: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
union NodeValue {
    string: *mut c_char,
    flag: c_int,
    int64: i64,
    double: f64,
    list: *mut NodeList,
}

#[repr(C)]
struct Node {
    u: NodeValue,
    format: c_int,
}

#[repr(C)]
struct NodeList {
    num: c_int,
    values: *mut Node,
    keys: *mut *mut c_char,
}

struct Api {
    _lib: Library,
    create: unsafe extern "C" fn() -> Handle,
    initialize: unsafe extern "C" fn(Handle) -> c_int,
    terminate_destroy: unsafe extern "C" fn(Handle),
    set_option_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    set_property_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    set_property: unsafe extern "C" fn(Handle, *const c_char, c_int, *mut c_void) -> c_int,
    command: unsafe extern "C" fn(Handle, *mut *const c_char) -> c_int,
    observe_property: unsafe extern "C" fn(Handle, u64, *const c_char, c_int) -> c_int,
    wait_event: unsafe extern "C" fn(Handle, f64) -> *mut RawEvent,
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
}

static API: OnceLock<Result<Api, String>> = OnceLock::new();

/// Pastas onde procurar a DLL: ao lado do executável e, em desenvolvimento, `lib/` do crate.
fn dll_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from)) {
        v.push(dir.join("libmpv-2.dll"));
        v.push(dir.join("lib").join("libmpv-2.dll"));
    }
    v.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib").join("libmpv-2.dll"));
    v
}

fn api() -> Result<&'static Api, String> {
    API.get_or_init(|| {
        let path = dll_candidates().into_iter().find(|p| p.exists()).ok_or("libmpv-2.dll não encontrada")?;
        // SAFETY: carrega a libmpv oficial; os símbolos abaixo têm as assinaturas da client API 2.x.
        unsafe {
            let lib = Library::new(&path).map_err(|e| format!("não foi possível carregar a libmpv: {e}"))?;
            macro_rules! sym {
                ($name:literal) => {
                    *lib.get(concat!($name, "\0").as_bytes()).map_err(|e| format!("{}: {e}", $name))?
                };
            }
            Ok(Api {
                create: sym!("mpv_create"),
                initialize: sym!("mpv_initialize"),
                terminate_destroy: sym!("mpv_terminate_destroy"),
                set_option_string: sym!("mpv_set_option_string"),
                set_property_string: sym!("mpv_set_property_string"),
                set_property: sym!("mpv_set_property"),
                command: sym!("mpv_command"),
                observe_property: sym!("mpv_observe_property"),
                wait_event: sym!("mpv_wait_event"),
                error_string: sym!("mpv_error_string"),
                _lib: lib,
            })
        }
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// Por que o arquivo terminou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    Eof,
    Stop,
    Error,
    Other,
}

#[derive(Debug, Clone)]
pub enum MpvEvent {
    Prop(String, Value),
    FileLoaded,
    EndFile(EndReason),
    /// Quadro pronto depois de abrir ou de um `seek` (`playback-restart`).
    Restart,
}

struct Inner {
    handle: Handle,
}

// SAFETY: a client API do mpv é thread-safe para tudo que usamos fora do `wait_event`,
// e só a thread de eventos chama `wait_event` e `terminate_destroy`.
unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

/// Uma instância do mpv. Ao ser descartada, manda `quit`; a thread de eventos destrói.
pub struct Mpv {
    inner: Arc<Inner>,
}

fn cstr(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

fn check(code: c_int) -> Result<(), String> {
    if code >= 0 {
        return Ok(());
    }
    let msg = api().ok().map(|a| unsafe { CStr::from_ptr((a.error_string)(code)) }.to_string_lossy().into_owned());
    Err(msg.unwrap_or_else(|| format!("erro {code} do mpv")))
}

/// Converte um `mpv_node` em JSON.
unsafe fn node_to_value(node: &Node) -> Value {
    unsafe {
        match node.format {
            FORMAT_STRING => Value::String(CStr::from_ptr(node.u.string).to_string_lossy().into_owned()),
            FORMAT_FLAG => Value::Bool(node.u.flag != 0),
            FORMAT_INT64 => Value::Number(node.u.int64.into()),
            FORMAT_DOUBLE => Number::from_f64(node.u.double).map_or(Value::Null, Value::Number),
            FORMAT_NODE_ARRAY | FORMAT_NODE_MAP => {
                let list = &*node.u.list;
                let n = list.num.max(0) as usize;
                let values = if n == 0 || list.values.is_null() { &[][..] } else { std::slice::from_raw_parts(list.values, n) };
                if node.format == FORMAT_NODE_ARRAY {
                    Value::Array(values.iter().map(|v| node_to_value(v)).collect())
                } else {
                    let keys = if n == 0 || list.keys.is_null() { &[][..] } else { std::slice::from_raw_parts(list.keys, n) };
                    let mut map = Map::new();
                    for (k, v) in keys.iter().zip(values) {
                        map.insert(CStr::from_ptr(*k).to_string_lossy().into_owned(), node_to_value(v));
                    }
                    Value::Object(map)
                }
            }
            _ => Value::Null,
        }
    }
}

unsafe fn property_value(p: &RawProperty) -> Value {
    unsafe {
        if p.data.is_null() {
            return Value::Null;
        }
        match p.format {
            FORMAT_FLAG => Value::Bool(*(p.data as *const c_int) != 0),
            FORMAT_INT64 => Value::Number((*(p.data as *const i64)).into()),
            FORMAT_DOUBLE => Number::from_f64(*(p.data as *const f64)).map_or(Value::Null, Value::Number),
            FORMAT_STRING => Value::String(CStr::from_ptr(*(p.data as *const *const c_char)).to_string_lossy().into_owned()),
            FORMAT_NODE => node_to_value(&*(p.data as *const Node)),
            _ => Value::Null,
        }
    }
}

/// O que observar e em que formato.
pub enum Observe {
    Double,
    Flag,
    Node,
}

impl Mpv {
    /// Cria, aplica as opções (antes de iniciar), inicia e observa as propriedades.
    pub fn new(options: &[(&str, String)], observe: &[(&str, Observe)]) -> Result<(Self, UnboundedReceiver<MpvEvent>), String> {
        let api = api()?;
        // SAFETY: handle novo; todas as strings vivem até o fim de cada chamada.
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err("mpv_create falhou".into());
        }
        for (k, v) in options {
            let (k, v) = (cstr(k), cstr(v));
            if let Err(e) = check(unsafe { (api.set_option_string)(handle, k.as_ptr(), v.as_ptr()) }) {
                eprintln!("mpv: opção {k:?}: {e}");
            }
        }
        if let Err(e) = check(unsafe { (api.initialize)(handle) }) {
            unsafe { (api.terminate_destroy)(handle) };
            return Err(e);
        }
        for (name, format) in observe {
            let fmt = match format {
                Observe::Double => FORMAT_DOUBLE,
                Observe::Flag => FORMAT_FLAG,
                Observe::Node => FORMAT_NODE,
            };
            let n = cstr(name);
            let _ = unsafe { (api.observe_property)(handle, 0, n.as_ptr(), fmt) };
        }

        let inner = Arc::new(Inner { handle });
        let (tx, rx) = unbounded();
        let events = inner.clone();
        std::thread::Builder::new()
            .name("mpv-events".into())
            .spawn(move || event_loop(api, events, tx))
            .map_err(|e| e.to_string())?;
        Ok((Self { inner }, rx))
    }

    pub fn command(&self, args: &[&str]) -> Result<(), String> {
        let api = api()?;
        let owned: Vec<CString> = args.iter().map(|a| cstr(a)).collect();
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        check(unsafe { (api.command)(self.inner.handle, ptrs.as_mut_ptr()) })
    }

    pub fn set(&self, name: &str, value: &str) -> Result<(), String> {
        let api = api()?;
        let (n, v) = (cstr(name), cstr(value));
        check(unsafe { (api.set_property_string)(self.inner.handle, n.as_ptr(), v.as_ptr()) })
    }

    pub fn set_flag(&self, name: &str, value: bool) -> Result<(), String> {
        let api = api()?;
        let n = cstr(name);
        let mut v: c_int = value.into();
        check(unsafe { (api.set_property)(self.inner.handle, n.as_ptr(), FORMAT_FLAG, &mut v as *mut c_int as *mut c_void) })
    }

    pub fn set_double(&self, name: &str, value: f64) -> Result<(), String> {
        let api = api()?;
        let n = cstr(name);
        let mut v = value;
        check(unsafe { (api.set_property)(self.inner.handle, n.as_ptr(), FORMAT_DOUBLE, &mut v as *mut f64 as *mut c_void) })
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        let _ = self.command(&["quit"]);
    }
}

fn event_loop(api: &'static Api, inner: Arc<Inner>, tx: UnboundedSender<MpvEvent>) {
    loop {
        // SAFETY: só esta thread espera eventos; o ponteiro vale até a próxima chamada.
        let ev = unsafe { &*(api.wait_event)(inner.handle, -1.0) };
        let out = match ev.event_id {
            EVENT_SHUTDOWN => break,
            EVENT_FILE_LOADED => Some(MpvEvent::FileLoaded),
            EVENT_PLAYBACK_RESTART => Some(MpvEvent::Restart),
            EVENT_END_FILE if !ev.data.is_null() => {
                let end = unsafe { &*(ev.data as *const RawEndFile) };
                Some(MpvEvent::EndFile(match end.reason {
                    0 => EndReason::Eof,
                    2 => EndReason::Stop,
                    4 => EndReason::Error,
                    _ => EndReason::Other,
                }))
            }
            EVENT_PROPERTY_CHANGE if !ev.data.is_null() => {
                let p = unsafe { &*(ev.data as *const RawProperty) };
                let name = unsafe { CStr::from_ptr(p.name) }.to_string_lossy().into_owned();
                let value = if p.format == FORMAT_NONE { Value::Null } else { unsafe { property_value(p) } };
                Some(MpvEvent::Prop(name, value))
            }
            _ => None,
        };
        if let Some(e) = out {
            let _ = tx.unbounded_send(e);
        }
    }
    // SAFETY: depois do SHUTDOWN ninguém mais usa o handle (o `Mpv` já foi descartado).
    unsafe { (api.terminate_destroy)(inner.handle) };
}

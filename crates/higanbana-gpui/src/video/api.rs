//! libmpv carregada em runtime (`libmpv-2.dll`): só os símbolos que o player usa,
//! incluindo a render API em modo software.
use libloading::Library;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::PathBuf;
use std::sync::OnceLock;

pub(crate) type Handle = *mut c_void;

pub(crate) const FORMAT_STRING: c_int = 1;
pub(crate) const FORMAT_FLAG: c_int = 3;
pub(crate) const FORMAT_INT64: c_int = 4;
pub(crate) const FORMAT_DOUBLE: c_int = 5;
pub(crate) const FORMAT_NODE: c_int = 6;
pub(crate) const FORMAT_NODE_ARRAY: c_int = 7;
pub(crate) const FORMAT_NODE_MAP: c_int = 8;

pub(crate) const EVENT_SHUTDOWN: c_int = 1;
pub(crate) const EVENT_START_FILE: c_int = 6;
pub(crate) const EVENT_END_FILE: c_int = 7;
pub(crate) const EVENT_FILE_LOADED: c_int = 8;
pub(crate) const EVENT_PLAYBACK_RESTART: c_int = 21;
pub(crate) const EVENT_PROPERTY_CHANGE: c_int = 22;

pub(crate) const END_REASON_ERROR: c_int = 4;

pub(crate) const RENDER_PARAM_INVALID: c_int = 0;
pub(crate) const RENDER_PARAM_API_TYPE: c_int = 1;
pub(crate) const RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
pub(crate) const RENDER_PARAM_OPENGL_FBO: c_int = 3;
pub(crate) const RENDER_PARAM_FLIP_Y: c_int = 4;
pub(crate) const RENDER_PARAM_SW_SIZE: c_int = 17;
pub(crate) const RENDER_PARAM_SW_FORMAT: c_int = 18;
pub(crate) const RENDER_PARAM_SW_STRIDE: c_int = 19;
pub(crate) const RENDER_PARAM_SW_POINTER: c_int = 20;
pub(crate) const RENDER_UPDATE_FRAME: u64 = 1;

/// `mpv_opengl_init_params` (API 2.x).
#[repr(C)]
pub(crate) struct OpenGlInitParams {
    pub get_proc_address: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
    pub get_proc_address_ctx: *mut c_void,
}

/// `mpv_opengl_fbo`.
#[repr(C)]
pub(crate) struct OpenGlFbo {
    pub fbo: c_int,
    pub w: c_int,
    pub h: c_int,
    pub internal_format: c_int,
}

#[repr(C)]
pub(crate) struct RawEvent {
    pub event_id: c_int,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

#[repr(C)]
pub(crate) struct RawProperty {
    pub name: *const c_char,
    pub format: c_int,
    pub data: *mut c_void,
}

#[repr(C)]
pub(crate) struct RawEndFile {
    pub reason: c_int,
    pub error: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) union NodeValue {
    pub string: *mut c_char,
    pub flag: c_int,
    pub int64: i64,
    pub double: f64,
    pub list: *mut NodeList,
}

#[repr(C)]
pub(crate) struct Node {
    pub u: NodeValue,
    pub format: c_int,
}

#[repr(C)]
pub(crate) struct NodeList {
    pub num: c_int,
    pub values: *mut Node,
    pub keys: *mut *mut c_char,
}

#[repr(C)]
pub(crate) struct RenderParam {
    pub kind: c_int,
    pub data: *mut c_void,
}

pub(crate) type UpdateFn = unsafe extern "C" fn(*mut c_void);

pub(crate) struct Api {
    _lib: Library,
    pub create: unsafe extern "C" fn() -> Handle,
    pub initialize: unsafe extern "C" fn(Handle) -> c_int,
    pub terminate_destroy: unsafe extern "C" fn(Handle),
    pub set_option_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    pub set_property_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    pub command: unsafe extern "C" fn(Handle, *mut *const c_char) -> c_int,
    pub observe_property: unsafe extern "C" fn(Handle, u64, *const c_char, c_int) -> c_int,
    pub wait_event: unsafe extern "C" fn(Handle, f64) -> *mut RawEvent,
    pub error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    pub render_create: unsafe extern "C" fn(*mut *mut c_void, Handle, *mut RenderParam) -> c_int,
    pub render_set_update_callback: unsafe extern "C" fn(*mut c_void, Option<UpdateFn>, *mut c_void),
    pub render_update: unsafe extern "C" fn(*mut c_void) -> u64,
    pub render_render: unsafe extern "C" fn(*mut c_void, *mut RenderParam) -> c_int,
    pub render_free: unsafe extern "C" fn(*mut c_void),
}

const LIB_NAME: &str = "libmpv-2.dll";

/// `HIGANBANA_MPV_DLL` (se definida, é a única), ao lado do exe, `<exe>/lib` e o
/// `lib/` do higanbana-desktop (desenvolvimento).
fn candidates() -> Vec<PathBuf> {
    if let Some(p) = std::env::var_os("HIGANBANA_MPV_DLL") {
        return vec![PathBuf::from(p)];
    }
    let mut v = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from)) {
        v.push(dir.join(LIB_NAME));
        v.push(dir.join("lib").join(LIB_NAME));
    }
    v.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../higanbana-desktop/lib").join(LIB_NAME));
    v
}

static API: OnceLock<Result<Api, String>> = OnceLock::new();

pub(crate) fn api() -> Result<&'static Api, String> {
    API.get_or_init(load).as_ref().map_err(Clone::clone)
}

fn load() -> Result<Api, String> {
    let path = candidates().into_iter().find(|p| p.exists()).ok_or_else(|| format!("{LIB_NAME} (libmpv) não encontrada"))?;
    // SAFETY: libmpv oficial; assinaturas da client API 2.x e da render API.
    unsafe {
        let lib = Library::new(&path).map_err(|e| format!("não foi possível carregar a libmpv: {e}"))?;
        macro_rules! sym {
            ($name:literal) => {
                *lib.get(concat!($name, "\0").as_bytes()).map_err(|e| format!("libmpv sem {}: {e}", $name))?
            };
        }
        Ok(Api {
            create: sym!("mpv_create"),
            initialize: sym!("mpv_initialize"),
            terminate_destroy: sym!("mpv_terminate_destroy"),
            set_option_string: sym!("mpv_set_option_string"),
            set_property_string: sym!("mpv_set_property_string"),
            command: sym!("mpv_command"),
            observe_property: sym!("mpv_observe_property"),
            wait_event: sym!("mpv_wait_event"),
            error_string: sym!("mpv_error_string"),
            render_create: sym!("mpv_render_context_create"),
            render_set_update_callback: sym!("mpv_render_context_set_update_callback"),
            render_update: sym!("mpv_render_context_update"),
            render_render: sym!("mpv_render_context_render"),
            render_free: sym!("mpv_render_context_free"),
            _lib: lib,
        })
    }
}

pub(crate) fn cstr(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

/// Mensagem do mpv para um código de erro negativo.
pub(crate) fn error_text(code: c_int) -> String {
    match api() {
        Ok(a) => unsafe { CStr::from_ptr((a.error_string)(code)) }.to_string_lossy().into_owned(),
        Err(e) => e,
    }
}

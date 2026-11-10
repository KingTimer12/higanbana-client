//! Janela filha onde o mpv desenha (`--wid`). Fica embaixo do que o GPUI desenha:
//! o GPUI compõe por cima das janelas filhas (DirectComposition) e deixa
//! transparente o que não pinta, então os controles aparecem sobre o vídeo.
//!
//! A janela é desabilitada: cliques e o mouse passam para a janela principal.

#[cfg(windows)]
mod imp {
    use std::sync::OnceLock;

    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::{BLACK_BRUSH, GetStockObject, HBRUSH},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, HWND_BOTTOM, RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE,
            SWP_NOACTIVATE, SetWindowPos, ShowCursor, ShowWindow, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
            WS_DISABLED,
        },
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    unsafe extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(h, m, w, l) }
    }

    fn class() -> &'static [u16] {
        static CLASS: OnceLock<Vec<u16>> = OnceLock::new();
        CLASS.get_or_init(|| {
            let name = wide("HiganbanaVideo");
            // SAFETY: classe registrada uma vez, com nome estático e procedimento padrão.
            unsafe {
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(proc),
                    hInstance: GetModuleHandleW(std::ptr::null()),
                    lpszClassName: name.as_ptr(),
                    hbrBackground: GetStockObject(BLACK_BRUSH) as HBRUSH,
                    ..std::mem::zeroed()
                };
                RegisterClassW(&wc);
            }
            name
        })
    }

    pub struct Surface {
        hwnd: HWND,
        last: std::cell::Cell<(i32, i32, i32, i32, bool)>,
        cursor_hidden: std::cell::Cell<bool>,
    }

    impl Surface {
        pub fn new(parent: usize) -> Option<Self> {
            let class = class();
            // SAFETY: filha da janela principal, criada na thread da interface.
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class.as_ptr(),
                    std::ptr::null(),
                    WS_CHILD | WS_CLIPSIBLINGS | WS_CLIPCHILDREN | WS_DISABLED,
                    0,
                    0,
                    1,
                    1,
                    parent as HWND,
                    std::ptr::null_mut(),
                    GetModuleHandleW(std::ptr::null()),
                    std::ptr::null(),
                )
            };
            (!hwnd.is_null()).then(|| Self { hwnd, last: std::cell::Cell::new((0, 0, 0, 0, false)), cursor_hidden: std::cell::Cell::new(false) })
        }

        /// Opções do mpv para desenhar nesta superfície (antes de iniciar).
        pub fn mpv_options(&self) -> Vec<(&'static str, String)> {
            vec![("wid", (self.hwnd as usize).to_string()), ("vo", "gpu-next".into()), ("force-window", "yes".into())]
        }

        /// No Windows o mpv desenha sozinho na janela filha (`wid`).
        pub fn attach(&self, _mpv: &super::super::mpv::Mpv) -> Result<(), String> {
            Ok(())
        }

        pub fn detach(&self, _mpv: &super::super::mpv::Mpv) {}

        /// Posição em pixels físicos da área cliente da janela principal.
        pub fn place(&self, x: i32, y: i32, w: i32, h: i32) {
            let next = (x, y, w.max(1), h.max(1), true);
            if self.last.get() == next {
                return;
            }
            let was_visible = self.last.get().4;
            self.last.set(next);
            // SAFETY: hwnd nosso, mesma thread.
            unsafe {
                SetWindowPos(self.hwnd, HWND_BOTTOM, x, y, next.2, next.3, SWP_NOACTIVATE);
                if !was_visible {
                    ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
                }
            }
        }

        /// Esconde o cursor do sistema sobre o vídeo parado de mexer. O GPUI não
        /// tem um estilo de cursor vazio; `ShowCursor` usa um contador por thread,
        /// então só chamamos quando o estado muda, para ele voltar ao equilíbrio.
        pub fn set_cursor_hidden(&self, hidden: bool) {
            if self.cursor_hidden.replace(hidden) != hidden {
                // SAFETY: chamada na thread da interface, sempre em pares.
                unsafe { ShowCursor(if hidden { 0 } else { 1 }) };
            }
        }

        pub fn hide(&self) {
            self.set_cursor_hidden(false);
            let mut l = self.last.get();
            if l.4 {
                l.4 = false;
                self.last.set(l);
                // SAFETY: hwnd nosso, mesma thread.
                unsafe { ShowWindow(self.hwnd, SW_HIDE) };
            }
        }
    }
}

/// macOS: `--wid` não é suportado, então usamos a render API da libmpv (como o
/// IINA). Uma `NSView` com um `CAOpenGLLayer` fica atrás da view do GPUI (irmã,
/// abaixo dela); o GPUI é transparente onde não pinta e o vídeo aparece por baixo.
/// Cliques nunca chegam aqui: a view do GPUI cobre tudo e é testada antes.
///
/// O layer desenha numa thread do Core Animation (`asynchronous`): a cada vsync
/// pergunta se há quadro novo (`canDraw…`) e, se houver, o mpv desenha no FBO.
/// O app tem uma superfície só, então o estado do render fica em estáticos.
#[cfg(target_os = "macos")]
mod imp {
    use std::{
        cell::Cell,
        ffi::{c_char, c_void},
        sync::{
            Mutex, OnceLock,
            atomic::{AtomicBool, Ordering},
        },
    };

    use objc2::{
        Encode, Encoding, RefEncode,
        rc::{Allocated, Retained},
        runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel},
        msg_send, sel,
    };

    use super::super::mpv::{Mpv, RenderContext};

    type Cgl = *mut c_void;

    /// Tipos opacos só para as assinaturas dos métodos do layer baterem com as
    /// do `CAOpenGLLayer` (o objc2 confere em debug).
    macro_rules! opaque {
        ($name:ident, $c:literal) => {
            #[repr(C)]
            struct $name([u8; 0]);
            // SAFETY: só usado atrás de ponteiro, nunca lido.
            unsafe impl RefEncode for $name {
                const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct($c, &[]));
            }
        };
    }
    opaque!(PixObj, "_CGLPixelFormatObject");
    opaque!(CtxObj, "_CGLContextObject");

    /// `CVTimeStamp` (struct anônima no runtime), só atrás de ponteiro.
    #[repr(C)]
    struct TimeStamp([u8; 0]);
    // SAFETY: só usado atrás de ponteiro, nunca lido.
    unsafe impl RefEncode for TimeStamp {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct(
            "?",
            &[
                Encoding::UInt,
                Encoding::Int,
                Encoding::LongLong,
                Encoding::ULongLong,
                Encoding::Double,
                Encoding::LongLong,
                Encoding::Struct(
                    "CVSMPTETime",
                    &[
                        Encoding::Short,
                        Encoding::Short,
                        Encoding::UInt,
                        Encoding::UInt,
                        Encoding::UInt,
                        Encoding::Short,
                        Encoding::Short,
                        Encoding::Short,
                        Encoding::Short,
                    ],
                ),
                Encoding::ULongLong,
                Encoding::ULongLong,
            ],
        ));
    }

    #[link(name = "OpenGL", kind = "framework")]
    unsafe extern "C" {
        fn CGLChoosePixelFormat(attribs: *const i32, pix: *mut Cgl, npix: *mut i32) -> i32;
        fn CGLCreateContext(pix: Cgl, share: Cgl, ctx: *mut Cgl) -> i32;
        fn CGLRetainContext(ctx: Cgl) -> Cgl;
        fn CGLRetainPixelFormat(pix: Cgl) -> Cgl;
        fn CGLGetCurrentContext() -> Cgl;
        fn CGLSetCurrentContext(ctx: Cgl) -> i32;
        fn CGLLockContext(ctx: Cgl) -> i32;
        fn CGLUnlockContext(ctx: Cgl) -> i32;
        fn glGetIntegerv(pname: u32, data: *mut i32);
        fn glClearColor(r: f32, g: f32, b: f32, a: f32);
        fn glClear(mask: u32);
        fn glFlush();
    }

    #[link(name = "QuartzCore", kind = "framework")]
    unsafe extern "C" {}

    unsafe extern "C" {
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }
    const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;

    const GL_DRAW_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
    const GL_VIEWPORT: u32 = 0x0BA2;
    const GL_COLOR_BUFFER_BIT: u32 = 0x4000;

    // Atributos de `CGLPixelFormatAttribute`.
    const PFA_DOUBLE_BUFFER: i32 = 5;
    const PFA_ACCELERATED: i32 = 73;
    const PFA_ALLOW_OFFLINE_RENDERERS: i32 = 96;
    const PFA_OPENGL_PROFILE: i32 = 99;
    const PFA_AUTOMATIC_GRAPHICS_SWITCHING: i32 = 101;
    const PROFILE_3_2_CORE: i32 = 0x3200;
    const PROFILE_LEGACY: i32 = 0x1000;

    /// `NSRect`/`CGRect` em 64 bits.
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }

    // SAFETY: mesmo layout e codificação de `CGRect` em 64 bits.
    unsafe impl Encode for Rect {
        const ENCODING: Encoding = Encoding::Struct(
            "CGRect",
            &[Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]), Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING])],
        );
    }

    /// Formato de pixel e contexto GL, criados uma vez e compartilhados com o layer.
    struct Gl {
        pix: usize,
        ctx: usize,
    }

    fn gl() -> Result<&'static Gl, String> {
        static GL: OnceLock<Result<Gl, String>> = OnceLock::new();
        GL.get_or_init(|| {
            for profile in [PROFILE_3_2_CORE, PROFILE_LEGACY] {
                let attrs = [
                    PFA_OPENGL_PROFILE,
                    profile,
                    PFA_ACCELERATED,
                    PFA_DOUBLE_BUFFER,
                    PFA_ALLOW_OFFLINE_RENDERERS,
                    PFA_AUTOMATIC_GRAPHICS_SWITCHING,
                    0,
                ];
                let (mut pix, mut n, mut ctx) = (std::ptr::null_mut(), 0, std::ptr::null_mut());
                // SAFETY: lista terminada em 0; ponteiros de saída válidos.
                unsafe {
                    if CGLChoosePixelFormat(attrs.as_ptr(), &mut pix, &mut n) != 0 || pix.is_null() {
                        continue;
                    }
                    if CGLCreateContext(pix, std::ptr::null_mut(), &mut ctx) != 0 || ctx.is_null() {
                        continue;
                    }
                }
                return Ok(Gl { pix: pix as usize, ctx: ctx as usize });
            }
            Err("não foi possível criar o contexto OpenGL do vídeo".into())
        })
        .as_ref()
        .map_err(Clone::clone)
    }

    /// Contexto de render do mpv atual e de quem ele é (`Mpv::id`).
    static RENDER: Mutex<Option<(usize, RenderContext)>> = Mutex::new(None);
    /// O mpv avisou que tem novidade (`update callback`).
    static PENDING: AtomicBool = AtomicBool::new(false);
    /// Redesenhar mesmo sem quadro novo (mudou de tamanho, trocou de mpv).
    static DIRTY: AtomicBool = AtomicBool::new(true);

    unsafe extern "C" fn on_update(_: *mut c_void) {
        PENDING.store(true, Ordering::Release);
    }

    unsafe extern "C" fn get_proc_address(_: *mut c_void, name: *const c_char) -> *mut c_void {
        // SAFETY: nome C válido vindo do mpv; o framework OpenGL está ligado.
        unsafe { dlsym(RTLD_DEFAULT, name) }
    }

    unsafe extern "C-unwind" fn copy_pixel_format(_: *mut AnyObject, _: Sel, _mask: u32) -> *mut PixObj {
        // SAFETY: o layer só existe depois de `gl()` ter dado certo.
        gl().map_or(std::ptr::null_mut(), |g| unsafe { CGLRetainPixelFormat(g.pix as Cgl) }.cast())
    }

    unsafe extern "C-unwind" fn copy_context(_: *mut AnyObject, _: Sel, _pix: *mut PixObj) -> *mut CtxObj {
        gl().map_or(std::ptr::null_mut(), |g| unsafe { CGLRetainContext(g.ctx as Cgl) }.cast())
    }

    unsafe extern "C-unwind" fn can_draw(_: *mut AnyObject, _: Sel, _ctx: *mut CtxObj, _pix: *mut PixObj, _t: f64, _ts: *const TimeStamp) -> Bool {
        if DIRTY.swap(false, Ordering::AcqRel) {
            return Bool::YES;
        }
        if !PENDING.swap(false, Ordering::AcqRel) {
            return Bool::NO;
        }
        let guard = RENDER.lock().unwrap_or_else(|e| e.into_inner());
        Bool::new(guard.as_ref().is_some_and(|(_, r)| r.update()))
    }

    unsafe extern "C-unwind" fn draw(_: *mut AnyObject, _: Sel, _ctx: *mut CtxObj, _pix: *mut PixObj, _t: f64, _ts: *const TimeStamp) {
        let guard = RENDER.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: o Core Animation deixa nosso contexto corrente (e travado) aqui.
        unsafe {
            match guard.as_ref() {
                Some((_, r)) => {
                    let (mut fbo, mut vp) = (0, [0i32; 4]);
                    glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &mut fbo);
                    glGetIntegerv(GL_VIEWPORT, vp.as_mut_ptr());
                    r.render(fbo, vp[2], vp[3]);
                }
                None => {
                    glClearColor(0., 0., 0., 1.);
                    glClear(GL_COLOR_BUFFER_BIT);
                }
            }
            glFlush();
        }
    }

    fn layer_class() -> &'static AnyClass {
        static CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
        CLASS.get_or_init(|| {
            let sup = AnyClass::get(c"CAOpenGLLayer").expect("CAOpenGLLayer");
            let mut b = ClassBuilder::new(c"HiganbanaVideoLayer", sup).expect("classe do layer de vídeo");
            // SAFETY: assinaturas iguais às do `CAOpenGLLayer`.
            unsafe {
                b.add_method(
                    sel!(copyCGLPixelFormatForDisplayMask:),
                    copy_pixel_format as unsafe extern "C-unwind" fn(*mut AnyObject, Sel, u32) -> *mut PixObj,
                );
                b.add_method(
                    sel!(copyCGLContextForPixelFormat:),
                    copy_context as unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut PixObj) -> *mut CtxObj,
                );
                b.add_method(
                    sel!(canDrawInCGLContext:pixelFormat:forLayerTime:displayTime:),
                    can_draw as unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut CtxObj, *mut PixObj, f64, *const TimeStamp) -> Bool,
                );
                b.add_method(
                    sel!(drawInCGLContext:pixelFormat:forLayerTime:displayTime:),
                    draw as unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut CtxObj, *mut PixObj, f64, *const TimeStamp),
                );
            }
            b.register()
        })
    }

    /// Roda `f` com nosso contexto GL travado e corrente nesta thread. Mesma ordem
    /// de travas do Core Animation (contexto, depois `RENDER`), sem deadlock.
    fn with_gl<R>(f: impl FnOnce() -> R) -> Result<R, String> {
        let ctx = gl()?.ctx as Cgl;
        // SAFETY: contexto nosso; restauramos o anterior.
        unsafe {
            CGLLockContext(ctx);
            let prev = CGLGetCurrentContext();
            CGLSetCurrentContext(ctx);
            let r = f();
            CGLSetCurrentContext(prev);
            CGLUnlockContext(ctx);
            Ok(r)
        }
    }

    pub struct Surface {
        /// View do GPUI (dona da janela, vive mais que nós).
        parent: *mut AnyObject,
        view: Retained<AnyObject>,
        layer: Retained<AnyObject>,
        last: Cell<(i32, i32, i32, i32, bool)>,
        cursor_hidden: Cell<bool>,
    }

    impl Surface {
        /// `parent` é a `NSView` do GPUI (`ns_view` do handle AppKit).
        pub fn new(parent: usize) -> Option<Self> {
            if let Err(e) = gl() {
                eprintln!("vídeo: {e}");
                return None;
            }
            let parent = parent as *mut AnyObject;
            // SAFETY: thread principal; `parent` é a view viva da janela.
            unsafe {
                let sup: *mut AnyObject = msg_send![parent, superview];
                if sup.is_null() {
                    eprintln!("vídeo: a view do GPUI ainda não está numa janela");
                    return None;
                }
                let layer: Retained<AnyObject> = msg_send![layer_class(), new];
                let _: () = msg_send![&*layer, setAsynchronous: Bool::YES];
                let _: () = msg_send![&*layer, setOpaque: Bool::YES];
                let _: () = msg_send![&*layer, setNeedsDisplayOnBoundsChange: Bool::YES];

                let alloc: Allocated<AnyObject> = msg_send![AnyClass::get(c"NSView")?, alloc];
                let view: Retained<AnyObject> = msg_send![alloc, initWithFrame: Rect::default()];
                // Layer antes do `wantsLayer`: view hospedeira do nosso layer.
                let _: () = msg_send![&*view, setLayer: &*layer];
                let _: () = msg_send![&*view, setWantsLayer: Bool::YES];
                let _: () = msg_send![&*view, setHidden: Bool::YES];
                // NSWindowBelow = -1: atrás da view do GPUI.
                let _: () = msg_send![sup, addSubview: &*view, positioned: -1isize, relativeTo: parent];
                Some(Self { parent, view, layer, last: Cell::new((0, 0, 0, 0, false)), cursor_hidden: Cell::new(false) })
            }
        }

        /// Opções do mpv para desenhar nesta superfície (antes de iniciar).
        pub fn mpv_options(&self) -> Vec<(&'static str, String)> {
            vec![("vo", "libmpv".into())]
        }

        /// Liga o render do `mpv` a esta superfície. Chamar antes de abrir o arquivo:
        /// sem contexto de render a saída de vídeo `libmpv` não inicia.
        pub fn attach(&self, mpv: &Mpv) -> Result<(), String> {
            let old = with_gl(|| {
                // SAFETY: `with_gl` deixa o contexto corrente.
                let r = unsafe { RenderContext::new(mpv, get_proc_address, on_update) }?;
                let mut guard = RENDER.lock().unwrap_or_else(|e| e.into_inner());
                // O anterior é descartado aqui, ainda com o contexto corrente.
                Ok::<_, String>(guard.replace((mpv.id(), r)))
            })??;
            drop(old);
            DIRTY.store(true, Ordering::Release);
            Ok(())
        }

        /// Solta o render do `mpv` (se ainda for o atual). Antes de descartar o `Mpv`.
        pub fn detach(&self, mpv: &Mpv) {
            let _ = with_gl(|| {
                let mut guard = RENDER.lock().unwrap_or_else(|e| e.into_inner());
                if guard.as_ref().is_some_and(|(id, _)| *id == mpv.id()) {
                    drop(guard.take());
                }
            });
            DIRTY.store(true, Ordering::Release);
        }

        /// Posição em pixels físicos a partir do canto superior esquerdo da view do GPUI.
        pub fn place(&self, x: i32, y: i32, w: i32, h: i32) {
            let next = (x, y, w.max(1), h.max(1), true);
            if self.last.get() == next {
                return;
            }
            self.last.set(next);
            // SAFETY: thread principal; views nossas e do GPUI vivas.
            unsafe {
                let window: *mut AnyObject = msg_send![self.parent, window];
                let scale: f64 = if window.is_null() { 1. } else { msg_send![window, backingScaleFactor] };
                let (px, py, pw, ph) = (x as f64 / scale, y as f64 / scale, next.2 as f64 / scale, next.3 as f64 / scale);
                let flipped: Bool = msg_send![self.parent, isFlipped];
                let bounds: Rect = msg_send![self.parent, bounds];
                let local = Rect { x: px, y: if flipped.as_bool() { py } else { bounds.h - py - ph }, w: pw, h: ph };
                let sup: *mut AnyObject = msg_send![&*self.view, superview];
                let frame: Rect = msg_send![self.parent, convertRect: local, toView: sup];

                // Sem animação implícita ao mover/redimensionar.
                let tx = AnyClass::get(c"CATransaction").expect("CATransaction");
                let _: () = msg_send![tx, begin];
                let _: () = msg_send![tx, setDisableActions: Bool::YES];
                let _: () = msg_send![&*self.view, setFrame: frame];
                let _: () = msg_send![&*self.layer, setContentsScale: scale];
                let _: () = msg_send![&*self.view, setHidden: Bool::NO];
                let _: () = msg_send![tx, commit];
            }
            DIRTY.store(true, Ordering::Release);
        }

        /// Esconde o cursor do sistema sobre o vídeo parado. `NSCursor hide/unhide`
        /// usa contador, então só chamamos quando o estado muda.
        pub fn set_cursor_hidden(&self, hidden: bool) {
            if self.cursor_hidden.replace(hidden) != hidden
                && let Some(cls) = AnyClass::get(c"NSCursor")
            {
                // SAFETY: métodos de classe do AppKit, thread principal, sempre em pares.
                unsafe {
                    if hidden {
                        let _: () = msg_send![cls, hide];
                    } else {
                        let _: () = msg_send![cls, unhide];
                    }
                }
            }
        }

        pub fn hide(&self) {
            self.set_cursor_hidden(false);
            let mut l = self.last.get();
            if l.4 {
                l.4 = false;
                self.last.set(l);
                // SAFETY: view nossa, thread principal.
                unsafe {
                    let _: () = msg_send![&*self.view, setHidden: Bool::YES];
                }
            }
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    pub struct Surface;
    impl Surface {
        pub fn new(_parent: usize) -> Option<Self> {
            None
        }
        pub fn mpv_options(&self) -> Vec<(&'static str, String)> {
            Vec::new()
        }
        pub fn attach(&self, _mpv: &super::super::mpv::Mpv) -> Result<(), String> {
            Ok(())
        }
        pub fn detach(&self, _mpv: &super::super::mpv::Mpv) {}
        pub fn place(&self, _x: i32, _y: i32, _w: i32, _h: i32) {}
        pub fn hide(&self) {}
        pub fn set_cursor_hidden(&self, _hidden: bool) {}
    }
}

pub use imp::Surface;

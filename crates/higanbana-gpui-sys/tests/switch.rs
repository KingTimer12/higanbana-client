mod common;
use common::*;
use higanbana_gpui_sys::*;

#[test]
fn switch_to_same_backend_is_noop() {
    let f = Fixture::new();
    unsafe {
        assert_eq!(hg_context_switch_backend(f.ctx, hg_backend_kind_HG_BACKEND_D3D11), OK);
        assert_eq!(hg_context_backend(f.ctx), hg_backend_kind_HG_BACKEND_D3D11);
    }
    assert_eq!(f.frame(), [OK; 3]);
}

#[test]
fn switch_to_unavailable_backend_keeps_current() {
    let f = Fixture::new();
    let mut infos = [hg_backend_info { kind: 0, name: [0; 32], adapter: [0; 128] }; 8];
    let n = unsafe { hg_enumerate_backends(infos.as_mut_ptr(), 8) } as usize;
    let available: Vec<_> = infos[..n].iter().map(|i| i.kind).collect();
    let missing = [hg_backend_kind_HG_BACKEND_D3D12, hg_backend_kind_HG_BACKEND_VULKAN,
                   hg_backend_kind_HG_BACKEND_OPENGL, hg_backend_kind_HG_BACKEND_WGPU]
        .into_iter().find(|k| !available.contains(k));
    let Some(missing) = missing else { return };
    unsafe {
        assert_eq!(hg_context_switch_backend(f.ctx, missing), HG_ERR_UNAVAILABLE as i32);
        assert_eq!(hg_context_backend(f.ctx), hg_backend_kind_HG_BACKEND_D3D11);
    }
    assert_eq!(f.frame(), [OK; 3]);
}

/// Troca para cada backend disponível e volta, desenhando texto em cada um.
#[test]
fn switch_through_every_available_backend_keeps_window_drawing() {
    let f = Fixture::new();
    let mut infos = [hg_backend_info { kind: 0, name: [0; 32], adapter: [0; 128] }; 8];
    let n = unsafe { hg_enumerate_backends(infos.as_mut_ptr(), 8) } as usize;
    let family = std::ffi::CString::new("Segoe UI").unwrap();
    let mut font = 0;
    unsafe { assert_eq!(hg_font_load(f.ctx, family.as_ptr(), 400.0, 0, &mut font), OK) };
    for info in &infos[..n] {
        unsafe {
            assert_eq!(hg_context_switch_backend(f.ctx, info.kind), OK,
                       "{:?}", std::ffi::CStr::from_ptr(hg_last_error()));
            assert_eq!(hg_context_backend(f.ctx), info.kind);
            hg_draw_list_reset(f.dl);
            let q = hg_quad { rect: hg_rect { x: 0.0, y: 0.0, w: 50.0, h: 50.0 },
                              fill: hg_color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }, ..std::mem::zeroed() };
            assert_eq!(hg_draw_quad(f.dl, &q), OK);
            let text = "Olá";
            assert_eq!(hg_draw_text(f.dl, font, 16.0, 60.0, 10.0, 0.0,
                                    hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 }, text.as_ptr() as _, text.len()), OK);
            let clear = hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
            assert_eq!(hg_frame_begin(f.win, clear), OK);
            assert_eq!(hg_frame_submit(f.win, f.dl), OK, "{:?}", std::ffi::CStr::from_ptr(hg_last_error()));
            let mut px = 0u32;
            assert_eq!(hg__window_read_pixel(f.win, 10, 10, &mut px), OK);
            assert_eq!(px & 0x00ff_ffff, 0x00ff_0000, "{:?}: pixel {px:08x}", info.kind);
            assert_eq!(hg_frame_end(f.win, 0), OK);
        }
    }
    unsafe { assert_eq!(hg_context_switch_backend(f.ctx, hg_backend_kind_HG_BACKEND_D3D11), OK) };
    assert_eq!(f.frame(), [OK; 3]);
}

#[link(name = "user32")]
unsafe extern "system" {
    fn IsWindow(h: *mut std::ffi::c_void) -> i32;
    fn IsWindowVisible(h: *mut std::ffi::c_void) -> i32;
}

/// Regressão: D3D11 -> OpenGL congelava a imagem (HWND que já teve swapchain
/// flip-model não mostra o SwapBuffers). Entrar no GL troca a HWND por uma nova.
#[test]
fn switch_to_opengl_gets_fresh_visible_hwnd() {
    let f = Fixture::new();
    let mut infos = [hg_backend_info { kind: 0, name: [0; 32], adapter: [0; 128] }; 8];
    let n = unsafe { hg_enumerate_backends(infos.as_mut_ptr(), 8) } as usize;
    if !infos[..n].iter().any(|i| i.kind == hg_backend_kind_HG_BACKEND_OPENGL) {
        return;
    }
    assert_eq!(f.frame(), [OK; 3]);
    unsafe {
        let old = hg__window_hwnd(f.win);
        assert_eq!(hg_context_switch_backend(f.ctx, hg_backend_kind_HG_BACKEND_OPENGL), OK);
        let fresh = hg__window_hwnd(f.win);
        assert_ne!(old, fresh, "HWND devia ser recriada para o GL");
        assert_eq!(IsWindow(old), 0, "HWND antiga devia ter sido destruída");
        assert_ne!(IsWindowVisible(fresh), 0);
    }
    assert_eq!(f.frame(), [OK; 3]);
    let mut ev: hg_event = unsafe { std::mem::zeroed() };
    while unsafe { hg_window_poll_event(f.win, &mut ev) } != 0 {
        assert_ne!(ev.kind, hg_event_kind_HG_EVENT_CLOSE, "trocar a HWND não pode fechar a janela");
    }
    unsafe { assert_eq!(hg_context_switch_backend(f.ctx, hg_backend_kind_HG_BACKEND_D3D11), OK) };
    assert_eq!(f.frame(), [OK; 3]);
}

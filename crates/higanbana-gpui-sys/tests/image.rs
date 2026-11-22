mod common;
use common::*;
use higanbana_gpui_sys::*;
use std::ptr;

const WHITE: hg_color = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

fn solid_image(ctx: *mut hg_context, rgba: [u8; 4], w: u32, h: u32) -> *mut hg_image {
    let px: Vec<u8> = (0..w * h).flat_map(|_| rgba).collect();
    let mut img = ptr::null_mut();
    unsafe { assert_eq!(hg_image_create(ctx, w, h, px.as_ptr(), w * 4, &mut img), OK) };
    img
}

fn draw(rect: hg_rect, tint: hg_color, radius: f32) -> hg_image_draw {
    hg_image_draw { rect, uv: hg_rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }, tint, radius: [radius; 4] }
}

fn pixel(f: &Fixture, x: u32, y: u32) -> u32 {
    let clear = hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    let mut px = 0;
    unsafe {
        assert_eq!(hg_frame_begin(f.win, clear), OK);
        assert_eq!(hg_frame_submit(f.win, f.dl), OK, "{:?}", std::ffi::CStr::from_ptr(hg_last_error()));
        assert_eq!(hg__window_read_pixel(f.win, x, y, &mut px), OK);
        assert_eq!(hg_frame_end(f.win, 0), OK);
    }
    px & 0x00ff_ffff
}

#[test]
fn invalid_image_args_are_rejected() {
    let f = Fixture::new();
    let px = [0u8; 16];
    let mut img = ptr::null_mut();
    unsafe {
        assert_eq!(hg_image_create(f.ctx, 0, 2, px.as_ptr(), 8, &mut img), HG_ERR_INVALID as i32);
        assert_eq!(hg_image_create(f.ctx, 2, 2, px.as_ptr(), 4, &mut img), HG_ERR_INVALID as i32);
        assert!(img.is_null());
    }
}

#[test]
fn image_pixels_reach_screen_with_tint() {
    let f = Fixture::new();
    let img = solid_image(f.ctx, [0, 255, 0, 255], 4, 4);
    unsafe {
        let r = hg_rect { x: 0.0, y: 0.0, w: 40.0, h: 40.0 };
        assert_eq!(hg_draw_image(f.dl, img, &draw(r, WHITE, 0.0)), OK);
        let half = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 0.5 };
        let r2 = hg_rect { x: 50.0, y: 0.0, w: 40.0, h: 40.0 };
        assert_eq!(hg_draw_image(f.dl, img, &draw(r2, half, 0.0)), OK);
    }
    assert_eq!(pixel(&f, 20, 20), 0x0000_ff00);
    let g = (pixel(&f, 70, 20) >> 8) & 0xff;
    assert!((120..=136).contains(&g), "meio alfa: {g}");
    let mut s = unsafe { std::mem::zeroed::<hg_stats>() };
    unsafe { hg_context_stats(f.ctx, &mut s) };
    assert_eq!(s.draw_calls, 1, "mesma imagem vizinha = um draw");
    unsafe { hg_image_destroy(img) };
}

#[test]
fn rounded_image_corner_is_background() {
    let f = Fixture::new();
    let img = solid_image(f.ctx, [255, 0, 0, 255], 2, 2);
    unsafe {
        let r = hg_rect { x: 0.0, y: 0.0, w: 60.0, h: 60.0 };
        assert_eq!(hg_draw_image(f.dl, img, &draw(r, WHITE, 30.0)), OK);
    }
    assert_eq!(pixel(&f, 1, 1), 0);
    assert_eq!(pixel(&f, 30, 30), 0x00ff_0000);
    unsafe { hg_image_destroy(img) };
}

#[test]
fn layer_puts_quad_over_image() {
    let f = Fixture::new();
    let img = solid_image(f.ctx, [255, 0, 0, 255], 2, 2);
    unsafe {
        let r = hg_rect { x: 0.0, y: 0.0, w: 60.0, h: 60.0 };
        let q = hg_quad { rect: r, fill: hg_color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 }, ..std::mem::zeroed() };
        assert_eq!(hg_draw_quad(f.dl, &q), OK);
        assert_eq!(hg_draw_image(f.dl, img, &draw(r, WHITE, 0.0)), OK);
        assert_eq!(pixel(&f, 30, 30), 0x00ff_0000, "imagem sobre quad na mesma camada");
        assert_eq!(hg_draw_list_layer(f.dl), OK);
        assert_eq!(hg_draw_quad(f.dl, &q), OK);
        assert_eq!(pixel(&f, 30, 30), 0x0000_00ff, "camada nova fica por cima");
        hg_image_destroy(img);
    }
}

#[test]
fn backend_switch_keeps_drawing_and_context_destroy_frees_leftovers() {
    let f = Fixture::new();
    let img = solid_image(f.ctx, [0, 0, 255, 255], 2, 2);
    let _leak = solid_image(f.ctx, [1, 2, 3, 255], 2, 2); // liberada por hg_context_destroy
    let mut infos = [hg_backend_info { kind: 0, name: [0; 32], adapter: [0; 128] }; 8];
    let n = unsafe { hg_enumerate_backends(infos.as_mut_ptr(), 8) } as usize;
    unsafe {
        let r = hg_rect { x: 0.0, y: 0.0, w: 40.0, h: 40.0 };
        assert_eq!(hg_draw_image(f.dl, img, &draw(r, WHITE, 0.0)), OK);
    }
    assert_eq!(pixel(&f, 20, 20), 0x0000_00ff);
    for info in &infos[..n] {
        unsafe { assert_eq!(hg_context_switch_backend(f.ctx, info.kind), OK) };
        pixel(&f, 20, 20); // backends sem imagem ainda apenas pulam
    }
    unsafe { assert_eq!(hg_context_switch_backend(f.ctx, hg_backend_kind_HG_BACKEND_D3D11), OK) };
    // A cópia em RAM saiu no primeiro envio: depois da troca a imagem volta vazia
    // (quem tem os pixels recria), sem falhar o frame.
    assert_eq!(pixel(&f, 20, 20), 0x0000_0000, "imagem vazia após troca");
    unsafe { hg_image_destroy(img) };
}

fn image_ram(f: &Fixture) -> u64 {
    let mut s = unsafe { std::mem::zeroed::<hg_stats>() };
    unsafe { hg_context_stats(f.ctx, &mut s) };
    s.image_ram_bytes
}

fn draw_full(f: &Fixture, img: *mut hg_image) {
    let r = hg_rect { x: 0.0, y: 0.0, w: 40.0, h: 40.0 };
    unsafe { assert_eq!(hg_draw_image(f.dl, img, &draw(r, WHITE, 0.0)), OK) };
}

#[test]
fn imagem_estatica_solta_a_copia_depois_de_enviar() {
    let f = Fixture::new();
    let img = solid_image(f.ctx, [0, 255, 0, 255], 64, 64);
    assert_eq!(image_ram(&f), 64 * 64 * 4, "antes do envio a cópia existe");
    draw_full(&f, img);
    assert_eq!(pixel(&f, 20, 20), 0x0000_ff00);
    assert_eq!(image_ram(&f), 0, "textura criada: a cópia em RAM sai");
    assert_eq!(pixel(&f, 20, 20), 0x0000_ff00, "e continua desenhando");
    unsafe { hg_image_destroy(img) };
    assert_eq!(image_ram(&f), 0);
}

#[test]
fn imagem_dinamica_mantem_a_copia() {
    let f = Fixture::new();
    let img = solid_image(f.ctx, [0, 255, 0, 255], 8, 8);
    draw_full(&f, img);
    pixel(&f, 20, 20);
    let blue: Vec<u8> = (0..64).flat_map(|_| [0u8, 0, 255, 255]).collect();
    unsafe { assert_eq!(hg_image_update(img, blue.as_ptr(), 32), OK) };
    assert_eq!(pixel(&f, 20, 20), 0x0000_00ff);
    assert_eq!(image_ram(&f), 8 * 8 * 4, "quadro de vídeo: a cópia fica para o próximo update");
    unsafe { hg_image_destroy(img) };
}

#[test]
fn troca_de_backend_com_imagem_sem_copia_segue_desenhando() {
    let f = Fixture::new();
    let img = solid_image(f.ctx, [0, 255, 0, 255], 16, 16);
    draw_full(&f, img);
    pixel(&f, 20, 20);
    assert_eq!(image_ram(&f), 0);
    if unsafe { hg_context_switch_backend(f.ctx, hg_backend_kind_HG_BACKEND_OPENGL) } != OK {
        unsafe { hg_image_destroy(img) };
        return;
    }
    // Sem a cópia, a imagem volta vazia (quem tem os pixels reenvia), mas o frame não falha.
    let clear = hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    unsafe {
        assert_eq!(hg_frame_begin(f.win, clear), OK);
        assert_eq!(hg_frame_submit(f.win, f.dl), OK);
        assert_eq!(hg_frame_end(f.win, 0), OK);
        assert_eq!(hg_context_switch_backend(f.ctx, hg_backend_kind_HG_BACKEND_D3D11), OK);
        hg_image_destroy(img);
    }
}

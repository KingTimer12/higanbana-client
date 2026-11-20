/* hgui_backend.h — superfície mínima para backends escritos fora do core (ex.: WGPU
 * em Rust). Acessores em vez de structs internas: o backend externo não depende do
 * layout de hg_context/hg_window/hg_draw_list nem de windows.h/DirectWrite. */
#ifndef HGUI_BACKEND_H
#define HGUI_BACKEND_H
#include "hgui.h"

#ifdef __cplusplus
extern "C" {
#endif

/* Grava a mensagem de erro thread-local e devolve o código. */
hg_result hg__fail(hg_result code, const char* message);

/* ---- Contexto ---------------------------------------------------------- */
void*            hg__ctx_backend_data(hg_context* ctx);
void             hg__ctx_set_backend_data(hg_context* ctx, void* data);
hg_backend_info* hg__ctx_info(hg_context* ctx);
hg_stats*        hg__ctx_stats(hg_context* ctx);

/* ---- Janela ------------------------------------------------------------ */
hg_context* hg__window_ctx(hg_window* win);
void*       hg__window_hwnd(hg_window* win);
void        hg__window_size(hg_window* win, uint32_t* width, uint32_t* height);
void*       hg__window_surface(hg_window* win);
void        hg__window_set_surface(hg_window* win, void* surface);

/* ---- Draw list --------------------------------------------------------- */
typedef struct hg_batch_view {
    hg_rect  clip;
    int      has_clip;
    uint32_t quad_count;
    uint32_t image_count;
} hg_batch_view;

/* Itera os lotes: prev = NULL começa; devolve NULL no fim. */
const void* hg__draw_list_next_batch(const hg_draw_list* dl, const void* prev, hg_batch_view* out);
/* Copia todos os quads, em ordem de lote, para `dst` (hg_draw_list_quad_count itens). */
void        hg__draw_list_copy_quads(const hg_draw_list* dl, hg_quad* dst);
/* Total de imagens da draw list (todos os lotes). */
uint32_t    hg__draw_list_image_count(const hg_draw_list* dl);
/* Copia, em ordem de lote, os desenhos para `items` e as imagens para `images`
 * (hg__draw_list_image_count itens cada). */
void        hg__draw_list_copy_images(const hg_draw_list* dl, hg_image_draw* items, hg_image** images);
/* Garante a textura do backend em todas as imagens da draw list. */
hg_result   hg__draw_list_prepare_images(const hg_draw_list* dl);

/* ---- Imagens ----------------------------------------------------------- */
/* Pixels RGBA8 pré-multiplicados, compactos (stride = largura*4); ver hg_image_size. */
const uint8_t* hg__image_pixels(const hg_image* img);
void*          hg__image_gpu_data(const hg_image* img);
void           hg__image_set_gpu_data(hg_image* img, void* gpu);

/* ---- Glyphs ------------------------------------------------------------ */
typedef struct hg_glyph_frame hg_glyph_frame;
typedef hg_result (*hg_glyph_upload_fn)(void* user, hg_rect at, const uint8_t* r8, uint32_t pitch);

/* Lado do atlas R8 que o backend deve alocar (pixels). */
uint32_t        hg__glyph_atlas_size(void);
hg_glyph_frame* hg__glyph_frame_new(void);
void            hg__glyph_frame_delete(hg_glyph_frame* g);
/* Instâncias coletadas (layout: rect, uv, color — 12 floats cada). */
const void*     hg__glyph_frame_instances(const hg_glyph_frame* g, uint32_t* count);
/* Glyphs do lote `batch` no último collect (0 se não houve texto). */
uint32_t        hg__glyph_frame_batch_count(const hg_glyph_frame* g, uint32_t batch);
hg_result       hg__glyph_frame_collect(hg_glyph_frame* g, hg_context* ctx, const hg_draw_list* dl,
                                        hg_glyph_upload_fn upload, void* user);
int             hg__draw_list_has_text(const hg_draw_list* dl);

#ifdef __cplusplus
}
#endif
#endif

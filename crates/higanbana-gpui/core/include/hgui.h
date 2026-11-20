/* hgui.h — API pública do core C do higanbana-gpui. */
#ifndef HGUI_H
#define HGUI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Erros --------------------------------------------------------- */
typedef int32_t hg_result;
#define HG_OK              0
#define HG_ERR_UNAVAILABLE 1
#define HG_ERR_DEVICE      2
#define HG_ERR_OOM         3
#define HG_ERR_INVALID     4

const char* hg_last_error(void);

/* ---- Geometria ----------------------------------------------------- */
typedef struct hg_rect  { float x, y, w, h; } hg_rect;
typedef struct hg_color { float r, g, b, a; } hg_color;

typedef struct hg_quad {
    hg_rect  rect;
    hg_color fill;
    hg_color border;
    float    radius[4];    /* tl, tr, br, bl */
    float    border_width;
    float    shadow_blur;
    hg_color shadow;
} hg_quad;

/* ---- Draw list ----------------------------------------------------- */
typedef struct hg_draw_list hg_draw_list;

hg_draw_list* hg_draw_list_create(void);
void          hg_draw_list_destroy(hg_draw_list* dl);
void          hg_draw_list_reset(hg_draw_list* dl);
hg_result     hg_draw_quad(hg_draw_list* dl, const hg_quad* quad);
hg_result     hg_push_clip(hg_draw_list* dl, hg_rect clip);
void          hg_pop_clip(hg_draw_list* dl);
uint32_t      hg_draw_list_quad_count(const hg_draw_list* dl);

/* ---- Backends e contexto --------------------------------------------- */
typedef enum hg_backend_kind {
    HG_BACKEND_AUTO = 0,
    HG_BACKEND_D3D11,
    HG_BACKEND_D3D12,
    HG_BACKEND_VULKAN,
    HG_BACKEND_OPENGL,
    HG_BACKEND_WGPU
} hg_backend_kind;

typedef struct hg_backend_info {
    hg_backend_kind kind;
    char            name[32];
    char            adapter[128];
} hg_backend_info;

typedef struct hg_context hg_context;

/* Preenche até `max` backends disponíveis nesta máquina; devolve quantos. */
int             hg_enumerate_backends(hg_backend_info* out, int max);
/* Usa `preferred` se disponível; senão D3D11 -> OpenGL -> WGPU. */
hg_result       hg_context_create(hg_backend_kind preferred, hg_context** out);
void            hg_context_destroy(hg_context* ctx);
hg_backend_kind hg_context_backend(const hg_context* ctx);

#ifdef __cplusplus
}
#endif
#endif /* HGUI_H */

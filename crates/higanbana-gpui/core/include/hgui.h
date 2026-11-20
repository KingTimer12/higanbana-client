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

#ifdef __cplusplus
}
#endif
#endif /* HGUI_H */

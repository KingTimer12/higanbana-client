/* backend_api.c — acessores de hgui_backend.h para backends externos (ex.: WGPU). */
#include "hgui_backend.h"
#include "draw_list.h"
#include "glyph_frame.h"
#include "window.h"
#include <stdlib.h>
#include <string.h>

void* hg__ctx_backend_data(hg_context* ctx) { return ctx->backend_data; }
void  hg__ctx_set_backend_data(hg_context* ctx, void* data) { ctx->backend_data = data; }
hg_backend_info* hg__ctx_info(hg_context* ctx) { return &ctx->info; }
hg_stats* hg__ctx_stats(hg_context* ctx) { return &ctx->stats; }

hg_context* hg__window_ctx(hg_window* win) { return win->ctx; }
void* hg__window_hwnd(hg_window* win) { return win->hwnd; }
void* hg__window_surface(hg_window* win) { return win->surface; }
void  hg__window_set_surface(hg_window* win, void* surface) { win->surface = surface; }

void hg__window_size(hg_window* win, uint32_t* width, uint32_t* height) {
    *width = win->width;
    *height = win->height;
}

const void* hg__draw_list_next_batch(const hg_draw_list* dl, const void* prev, hg_batch_view* out) {
    const hg_batch* b = prev ? ((const hg_batch*)prev)->next : dl->first;
    if (!b) return NULL;
    out->clip = b->clip;
    out->has_clip = b->has_clip;
    out->quad_count = b->quad_count;
    return b;
}

void hg__draw_list_copy_quads(const hg_draw_list* dl, hg_quad* dst) {
    for (const hg_batch* b = dl->first; b; b = b->next)
        for (const hg_quad_chunk* c = b->quads_first; c; c = c->next) {
            memcpy(dst, c->items, sizeof(hg_quad) * c->count);
            dst += c->count;
        }
}

uint32_t hg__glyph_atlas_size(void) { return HG_GLYPH_ATLAS_SIZE; }

hg_glyph_frame* hg__glyph_frame_new(void) { return calloc(1, sizeof(hg_glyph_frame)); }

void hg__glyph_frame_delete(hg_glyph_frame* g) {
    if (!g) return;
    hg__glyph_frame_free(g);
    free(g);
}

const void* hg__glyph_frame_instances(const hg_glyph_frame* g, uint32_t* count) {
    *count = g->count;
    return g->instances;
}

uint32_t hg__glyph_frame_batch_count(const hg_glyph_frame* g, uint32_t batch) {
    return g->count && batch < g->batch_capacity ? g->batch_counts[batch] : 0;
}

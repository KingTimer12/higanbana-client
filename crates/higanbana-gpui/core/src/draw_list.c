/* draw_list.c — lotes por clip/camada, quads em chunks na arena. */
#include "draw_list.h"
#include "error.h"
#include <stdlib.h>
#include <string.h>

hg_batch* hg__draw_list_open_batch(hg_draw_list* dl) {
    hg_batch* b = hg_arena_alloc(&dl->arena, sizeof *b, _Alignof(hg_batch));
    if (!b) return NULL;
    memset(b, 0, sizeof *b);
    if (dl->clip_depth > 0) {
        b->clip = dl->clips[dl->clip_depth - 1];
        b->has_clip = 1;
    }
    if (dl->last) dl->last->next = b; else dl->first = b;
    dl->last = b;
    return b;
}

hg_batch* hg__draw_list_batch(hg_draw_list* dl) {
    return dl->last ? dl->last : hg__draw_list_open_batch(dl);
}

hg_draw_list* hg_draw_list_create(void) {
    hg_draw_list* dl = calloc(1, sizeof *dl);
    if (dl) hg_arena_init(&dl->arena);
    return dl;
}

void hg_draw_list_destroy(hg_draw_list* dl) {
    if (!dl) return;
    hg_arena_free(&dl->arena);
    free(dl);
}

void hg_draw_list_reset(hg_draw_list* dl) {
    hg_arena_reset(&dl->arena);
    dl->first = dl->last = NULL;
    dl->quad_count = 0;
    dl->image_count = 0;
    dl->clip_depth = 0;
}

static hg_quad_chunk* writable_chunk(hg_draw_list* dl, hg_batch* b) {
    hg_quad_chunk* c = b->quads_last;
    if (c && c->count < HG_QUAD_CHUNK) return c;
    c = hg_arena_alloc(&dl->arena, sizeof *c, _Alignof(hg_quad_chunk));
    if (!c) return NULL;
    c->next = NULL;
    c->count = 0;
    if (b->quads_last) b->quads_last->next = c; else b->quads_first = c;
    b->quads_last = c;
    return c;
}

hg_result hg_draw_quad(hg_draw_list* dl, const hg_quad* quad) {
    if (!dl || !quad) return hg__fail(HG_ERR_INVALID, "hg_draw_quad: argumento nulo");
    hg_batch* b = hg__draw_list_batch(dl);
    hg_quad_chunk* c = b ? writable_chunk(dl, b) : NULL;
    if (!c) return hg__fail(HG_ERR_OOM, "hg_draw_quad: sem memória");
    c->items[c->count++] = *quad;
    b->quad_count++;
    dl->quad_count++;
    return HG_OK;
}

static hg_rect intersect(hg_rect a, hg_rect b) {
    float x0 = a.x > b.x ? a.x : b.x, y0 = a.y > b.y ? a.y : b.y;
    float x1 = (a.x + a.w) < (b.x + b.w) ? a.x + a.w : b.x + b.w;
    float y1 = (a.y + a.h) < (b.y + b.h) ? a.y + a.h : b.y + b.h;
    hg_rect r = { x0, y0, x1 > x0 ? x1 - x0 : 0, y1 > y0 ? y1 - y0 : 0 };
    return r;
}

hg_result hg_push_clip(hg_draw_list* dl, hg_rect clip) {
    if (!dl) return hg__fail(HG_ERR_INVALID, "hg_push_clip: lista nula");
    if (dl->clip_depth == HG_CLIP_STACK_MAX)
        return hg__fail(HG_ERR_INVALID, "hg_push_clip: pilha de clip cheia");
    if (dl->clip_depth > 0) clip = intersect(dl->clips[dl->clip_depth - 1], clip);
    dl->clips[dl->clip_depth++] = clip;
    return hg__draw_list_open_batch(dl) ? HG_OK : hg__fail(HG_ERR_OOM, "hg_push_clip: sem memória");
}

void hg_pop_clip(hg_draw_list* dl) {
    if (!dl || dl->clip_depth == 0) return;
    dl->clip_depth--;
    hg__draw_list_open_batch(dl);
}

uint32_t hg_draw_list_quad_count(const hg_draw_list* dl) { return dl ? dl->quad_count : 0; }

hg_result hg_draw_list_layer(hg_draw_list* dl) {
    if (!dl) return hg__fail(HG_ERR_INVALID, "hg_draw_list_layer: lista nula");
    return hg__draw_list_open_batch(dl) ? HG_OK : hg__fail(HG_ERR_OOM, "hg_draw_list_layer: sem memória");
}

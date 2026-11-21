#include "atlas_pack.h"
#include <stdlib.h>

static int push(hg_atlas_pack* p, hg_urect r) {
    if (r.w == 0 || r.h == 0) return 1;
    if (p->count == p->capacity) {
        uint32_t cap = p->capacity ? p->capacity * 2 : 32;
        hg_urect* grown = realloc(p->free_rects, sizeof *grown * cap);
        if (!grown) return 0;
        p->free_rects = grown;
        p->capacity = cap;
    }
    p->free_rects[p->count++] = r;
    return 1;
}

static void remove_at(hg_atlas_pack* p, uint32_t i) { p->free_rects[i] = p->free_rects[--p->count]; }

int hg_pack_init(hg_atlas_pack* p, uint32_t width, uint32_t height) {
    p->free_rects = NULL;
    p->count = p->capacity = 0;
    p->width = width;
    p->height = height;
    hg_urect all = { 0, 0, width, height };
    return push(p, all);
}

void hg_pack_free(hg_atlas_pack* p) {
    free(p->free_rects);
    p->free_rects = NULL;
    p->count = p->capacity = 0;
}

void hg_pack_clear(hg_atlas_pack* p) {
    p->count = 0;
    hg_urect all = { 0, 0, p->width, p->height };
    push(p, all);
}

int hg_pack_alloc(hg_atlas_pack* p, uint32_t w, uint32_t h, hg_urect* out) {
    /* Best short side fit. */
    uint32_t best = UINT32_MAX, best_i = UINT32_MAX;
    for (uint32_t i = 0; i < p->count; i++) {
        hg_urect f = p->free_rects[i];
        if (f.w < w || f.h < h) continue;
        uint32_t dw = f.w - w, dh = f.h - h, s = dw < dh ? dw : dh;
        if (s < best) { best = s; best_i = i; }
    }
    if (best_i == UINT32_MAX) return 0;
    hg_urect f = p->free_rects[best_i];
    remove_at(p, best_i);
    *out = (hg_urect){ f.x, f.y, w, h };
    /* Corte pelo eixo de maior sobra para manter retângulos grandes. */
    if (f.w - w > f.h - h) {
        push(p, (hg_urect){ f.x + w, f.y, f.w - w, f.h });
        push(p, (hg_urect){ f.x, f.y + h, w, f.h - h });
    } else {
        push(p, (hg_urect){ f.x, f.y + h, f.w, f.h - h });
        push(p, (hg_urect){ f.x + w, f.y, f.w - w, h });
    }
    return 1;
}

static int try_merge(hg_urect a, hg_urect b, hg_urect* out) {
    if (a.y == b.y && a.h == b.h && (a.x + a.w == b.x || b.x + b.w == a.x)) {
        *out = (hg_urect){ a.x < b.x ? a.x : b.x, a.y, a.w + b.w, a.h };
        return 1;
    }
    if (a.x == b.x && a.w == b.w && (a.y + a.h == b.y || b.y + b.h == a.y)) {
        *out = (hg_urect){ a.x, a.y < b.y ? a.y : b.y, a.w, a.h + b.h };
        return 1;
    }
    return 0;
}

int hg_pack_release(hg_atlas_pack* p, hg_urect r) {
    /* Funde repetidamente até não haver vizinho alinhado. */
    for (int merged = 1; merged;) {
        merged = 0;
        for (uint32_t i = 0; i < p->count; i++) {
            hg_urect m;
            if (try_merge(r, p->free_rects[i], &m)) {
                remove_at(p, i);
                r = m;
                merged = 1;
                break;
            }
        }
    }
    return push(p, r);
}

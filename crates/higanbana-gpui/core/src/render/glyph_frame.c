/* glyph_frame.c — sink comum: acumula instâncias por lote e repassa uploads ao backend. */
#include "glyph_frame.h"
#include "context.h"
#include "draw_list.h"
#include "error.h"
#include <stdlib.h>

typedef struct sink_state {
    hg_glyph_frame*    g;
    hg_glyph_upload_fn upload;
    void*              user;
} sink_state;

static hg_result sink_upload(void* user, hg_rect at, const uint8_t* r8, uint32_t pitch) {
    sink_state* s = user;
    return s->upload(s->user, at, r8, pitch);
}

static hg_result sink_emit(void* user, const hg_glyph_instance* glyph) {
    hg_glyph_frame* g = ((sink_state*)user)->g;
    if (g->count == g->capacity) {
        uint32_t cap = g->capacity ? g->capacity * 2 : 1024;
        hg_glyph_instance* grown = realloc(g->instances, sizeof *grown * cap);
        if (!grown) return hg__fail(HG_ERR_OOM, "glyphs: sem memória");
        g->instances = grown;
        g->capacity = cap;
    }
    g->instances[g->count++] = *glyph;
    return HG_OK;
}

static hg_result reserve_batches(hg_glyph_frame* g, uint32_t batches) {
    if (batches <= g->batch_capacity) return HG_OK;
    uint32_t* grown = realloc(g->batch_counts, sizeof *grown * batches);
    if (!grown) return hg__fail(HG_ERR_OOM, "glyphs: sem memória para lotes");
    g->batch_counts = grown;
    g->batch_capacity = batches;
    return HG_OK;
}

int hg__draw_list_has_text(const hg_draw_list* dl) {
    for (const hg_batch* b = dl->first; b; b = b->next)
        if (b->text_count) return 1;
    return 0;
}

hg_result hg__glyph_frame_collect(hg_glyph_frame* g, hg_context* ctx, const hg_draw_list* dl,
                                  hg_glyph_upload_fn upload, void* user) {
    g->count = 0;
    uint32_t batches = 0, texts = 0;
    for (const hg_batch* b = dl->first; b; b = b->next, batches++) texts += b->text_count;
    if (texts == 0) return HG_OK;
    if (!ctx->text) return hg__fail(HG_ERR_INVALID, "texto desenhado sem fonte carregada");

    hg_result r = reserve_batches(g, batches);
    sink_state state = { g, upload, user };
    hg_glyph_sink sink = { &state, sink_upload, sink_emit };
    uint32_t i = 0;
    for (const hg_batch* b = dl->first; b && r == HG_OK; b = b->next, i++) {
        uint32_t before = g->count;
        for (const hg_text_run* run = b->texts_first; run && r == HG_OK; run = run->next)
            r = hg__text_render_run(ctx->text, run, ctx->frame_index, &sink);
        g->batch_counts[i] = g->count - before;
    }
    return r;
}

void hg__glyph_frame_free(hg_glyph_frame* g) {
    free(g->instances);
    free(g->batch_counts);
    *g = (hg_glyph_frame){ 0 };
}

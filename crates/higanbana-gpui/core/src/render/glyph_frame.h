/* glyph_frame.h — coleta, independente de backend, das instâncias de glyph de um frame.
 * O backend só fornece `upload` (escreve R8 na sua textura de atlas) e depois
 * sobe `instances` para a GPU, desenhando `batch_counts[i]` glyphs no lote i. */
#ifndef HG_GLYPH_FRAME_H
#define HG_GLYPH_FRAME_H
#include "text.h"
#include "hgui_backend.h" /* typedefs hg_glyph_frame / hg_glyph_upload_fn */

struct hg_glyph_frame {
    hg_glyph_instance* instances;    /* rascunho reaproveitado entre frames */
    uint32_t           count, capacity;
    uint32_t*          batch_counts; /* glyphs por lote da draw list */
    uint32_t           batch_capacity;
};

/* Rasteriza o texto de `dl`. count = 0 se não há texto (e nada é exigido do backend). */
hg_result hg__glyph_frame_collect(hg_glyph_frame* g, hg_context* ctx, const hg_draw_list* dl,
                                  hg_glyph_upload_fn upload, void* user);
/* 1 se a draw list tem algum texto (para criar recursos sob demanda antes de coletar). */
int       hg__draw_list_has_text(const hg_draw_list* dl);
void      hg__glyph_frame_free(hg_glyph_frame* g);
#endif

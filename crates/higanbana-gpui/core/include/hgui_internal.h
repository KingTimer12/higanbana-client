/* hgui_internal.h — símbolos internos (prefixo hg__) expostos só para testes.
 * Não fazem parte da API estável. */
#ifndef HGUI_INTERNAL_H
#define HGUI_INTERNAL_H
#include "hgui.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Atlas (skyline + LRU) ------------------------------------------ */
typedef struct hg_atlas hg_atlas;
typedef void (*hg_atlas_evict_fn)(void* user, uint64_t key);

hg_atlas* hg__atlas_create(uint32_t width, uint32_t height);
void      hg__atlas_destroy(hg_atlas* atlas);
void      hg__atlas_on_evict(hg_atlas* atlas, hg_atlas_evict_fn fn, void* user);
/* out_uv em pixels. out_inserted=1 quando o espaço é novo (precisa upload). */
hg_result hg__atlas_get_or_insert(hg_atlas* atlas, uint64_t key, uint32_t w, uint32_t h,
                                  uint64_t frame, hg_rect* out_uv, int* out_inserted);
int       hg__atlas_contains(const hg_atlas* atlas, uint64_t key);
uint32_t  hg__atlas_entry_count(const hg_atlas* atlas);

#ifdef __cplusplus
}
#endif
#endif

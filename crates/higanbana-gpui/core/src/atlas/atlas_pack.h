/* atlas_pack.h — empacotador por retângulos livres (guillotine) que aceita liberar espaço. */
#ifndef HG_ATLAS_PACK_H
#define HG_ATLAS_PACK_H
#include <stdint.h>

typedef struct hg_urect { uint32_t x, y, w, h; } hg_urect;

typedef struct hg_atlas_pack {
    hg_urect* free_rects;
    uint32_t  count;
    uint32_t  capacity;
    uint32_t  width, height;
} hg_atlas_pack;

int  hg_pack_init(hg_atlas_pack* p, uint32_t width, uint32_t height);
void hg_pack_free(hg_atlas_pack* p);
void hg_pack_clear(hg_atlas_pack* p);
/* 1 = encontrou espaço em *out. */
int  hg_pack_alloc(hg_atlas_pack* p, uint32_t w, uint32_t h, hg_urect* out);
/* Devolve a região e funde com vizinhos alinhados. 0 = sem memória. */
int  hg_pack_release(hg_atlas_pack* p, hg_urect r);
#endif

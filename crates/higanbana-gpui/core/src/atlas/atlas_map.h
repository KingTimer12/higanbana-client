/* atlas_map.h — hash aberto (linear probing, remoção por backward shift) key -> slot. */
#ifndef HG_ATLAS_MAP_H
#define HG_ATLAS_MAP_H
#include <stdint.h>

#define HG_MAP_EMPTY UINT32_MAX

typedef struct hg_atlas_map {
    uint64_t* keys;
    uint32_t* values;
    uint32_t  capacity; /* potência de 2 */
    uint32_t  count;
} hg_atlas_map;

int      hg_map_init(hg_atlas_map* m, uint32_t capacity);
void     hg_map_free(hg_atlas_map* m);
uint32_t hg_map_get(const hg_atlas_map* m, uint64_t key);       /* HG_MAP_EMPTY se ausente */
int      hg_map_put(hg_atlas_map* m, uint64_t key, uint32_t v); /* 0 = sem memória */
void     hg_map_remove(hg_atlas_map* m, uint64_t key);
void     hg_map_clear(hg_atlas_map* m); /* mantém a capacidade */
#endif

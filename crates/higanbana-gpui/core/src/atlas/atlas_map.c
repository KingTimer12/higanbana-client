#include "atlas_map.h"
#include <stdlib.h>

static uint32_t slot_of(const hg_atlas_map* m, uint64_t key) {
    key ^= key >> 33;
    key *= 0xff51afd7ed558ccdULL;
    key ^= key >> 33;
    return (uint32_t)key & (m->capacity - 1);
}

int hg_map_init(hg_atlas_map* m, uint32_t capacity) {
    uint32_t cap = 16;
    while (cap < capacity) cap <<= 1;
    m->keys = malloc(sizeof *m->keys * cap);
    m->values = malloc(sizeof *m->values * cap);
    m->capacity = cap;
    m->count = 0;
    if (!m->keys || !m->values) { hg_map_free(m); return 0; }
    for (uint32_t i = 0; i < cap; i++) m->values[i] = HG_MAP_EMPTY;
    return 1;
}

void hg_map_free(hg_atlas_map* m) {
    free(m->keys);
    free(m->values);
    m->keys = NULL;
    m->values = NULL;
    m->capacity = m->count = 0;
}

uint32_t hg_map_get(const hg_atlas_map* m, uint64_t key) {
    for (uint32_t i = slot_of(m, key);; i = (i + 1) & (m->capacity - 1)) {
        if (m->values[i] == HG_MAP_EMPTY) return HG_MAP_EMPTY;
        if (m->keys[i] == key) return m->values[i];
    }
}

static int grow(hg_atlas_map* m) {
    hg_atlas_map bigger;
    if (!hg_map_init(&bigger, m->capacity * 2)) return 0;
    for (uint32_t i = 0; i < m->capacity; i++)
        if (m->values[i] != HG_MAP_EMPTY) hg_map_put(&bigger, m->keys[i], m->values[i]);
    hg_map_free(m);
    *m = bigger;
    return 1;
}

int hg_map_put(hg_atlas_map* m, uint64_t key, uint32_t v) {
    if ((m->count + 1) * 4 > m->capacity * 3 && !grow(m)) return 0;
    uint32_t i = slot_of(m, key);
    while (m->values[i] != HG_MAP_EMPTY && m->keys[i] != key) i = (i + 1) & (m->capacity - 1);
    if (m->values[i] == HG_MAP_EMPTY) m->count++;
    m->keys[i] = key;
    m->values[i] = v;
    return 1;
}

void hg_map_remove(hg_atlas_map* m, uint64_t key) {
    uint32_t mask = m->capacity - 1, i = slot_of(m, key);
    while (m->values[i] != HG_MAP_EMPTY && m->keys[i] != key) i = (i + 1) & mask;
    if (m->values[i] == HG_MAP_EMPTY) return;
    m->values[i] = HG_MAP_EMPTY;
    m->count--;
    /* Backward shift: reposiciona o cluster seguinte para não deixar buracos. */
    for (uint32_t j = (i + 1) & mask; m->values[j] != HG_MAP_EMPTY; j = (j + 1) & mask) {
        uint32_t home = slot_of(m, m->keys[j]);
        int between = (i <= j) ? (home > i && home <= j) : (home > i || home <= j);
        if (between) continue;
        m->keys[i] = m->keys[j];
        m->values[i] = m->values[j];
        m->values[j] = HG_MAP_EMPTY;
        i = j;
    }
}

void hg_map_clear(hg_atlas_map* m) {
    for (uint32_t i = 0; i < m->capacity; i++) m->values[i] = HG_MAP_EMPTY;
    m->count = 0;
}

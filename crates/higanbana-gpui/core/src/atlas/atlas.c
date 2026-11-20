/* atlas.c — cache de regiões com eviction LRU sobre atlas_pack + atlas_map. */
#include "hgui_internal.h"
#include "atlas_map.h"
#include "atlas_pack.h"
#include "error.h"
#include <stdlib.h>

typedef struct hg_atlas_entry {
    uint64_t key;
    uint64_t last_used;
    hg_urect rect;
    uint64_t user;
    int      alive;
} hg_atlas_entry;

struct hg_atlas {
    hg_atlas_pack     pack;
    hg_atlas_map      map;
    hg_atlas_entry*   entries;
    uint32_t          entry_count; /* slots usados (inclui mortos) */
    uint32_t          entry_capacity;
    uint32_t*         free_slots;  /* pilha de slots mortos para reuso */
    uint32_t          free_count;
    uint32_t          alive_count;
    hg_atlas_evict_fn on_evict;
    void*             evict_user;
};

hg_atlas* hg__atlas_create(uint32_t width, uint32_t height) {
    hg_atlas* a = calloc(1, sizeof *a);
    if (!a) return NULL;
    if (!hg_pack_init(&a->pack, width, height) || !hg_map_init(&a->map, 256)) {
        hg__atlas_destroy(a);
        return NULL;
    }
    return a;
}

void hg__atlas_destroy(hg_atlas* a) {
    if (!a) return;
    hg_pack_free(&a->pack);
    hg_map_free(&a->map);
    free(a->entries);
    free(a->free_slots);
    free(a);
}

void hg__atlas_on_evict(hg_atlas* a, hg_atlas_evict_fn fn, void* user) {
    a->on_evict = fn;
    a->evict_user = user;
}

static uint32_t new_slot(hg_atlas* a) {
    if (a->free_count) return a->free_slots[--a->free_count];
    if (a->entry_count == a->entry_capacity) {
        uint32_t cap = a->entry_capacity ? a->entry_capacity * 2 : 64;
        hg_atlas_entry* e = realloc(a->entries, sizeof *e * cap);
        uint32_t* f = e ? realloc(a->free_slots, sizeof *f * cap) : NULL;
        if (e) a->entries = e;
        if (!f) return HG_MAP_EMPTY;
        a->free_slots = f;
        a->entry_capacity = cap;
    }
    return a->entry_count++;
}

static void evict_slot(hg_atlas* a, uint32_t slot) {
    hg_atlas_entry* e = &a->entries[slot];
    hg_pack_release(&a->pack, e->rect);
    hg_map_remove(&a->map, e->key);
    e->alive = 0;
    a->free_slots[a->free_count++] = slot;
    a->alive_count--;
    if (a->alive_count == 0) hg_pack_clear(&a->pack); /* desfaz fragmentação */
    if (a->on_evict) a->on_evict(a->evict_user, e->key);
}

static uint32_t oldest_slot(const hg_atlas* a, uint64_t frame) {
    uint32_t best = HG_MAP_EMPTY;
    for (uint32_t i = 0; i < a->entry_count; i++) {
        const hg_atlas_entry* e = &a->entries[i];
        if (!e->alive || e->last_used >= frame) continue; /* nunca evicta o frame atual */
        if (best == HG_MAP_EMPTY || e->last_used < a->entries[best].last_used) best = i;
    }
    return best;
}

static hg_rect to_rect(hg_urect r) {
    hg_rect out = { (float)r.x, (float)r.y, (float)r.w, (float)r.h };
    return out;
}

hg_result hg__atlas_get_or_insert(hg_atlas* a, uint64_t key, uint32_t w, uint32_t h,
                                  uint64_t frame, hg_rect* out_uv, int* out_inserted) {
    if (!a || !out_uv || !out_inserted) return hg__fail(HG_ERR_INVALID, "atlas: argumento nulo");
    uint32_t slot = hg_map_get(&a->map, key);
    if (slot != HG_MAP_EMPTY) {
        a->entries[slot].last_used = frame;
        *out_uv = to_rect(a->entries[slot].rect);
        *out_inserted = 0;
        return HG_OK;
    }
    if (w > a->pack.width || h > a->pack.height)
        return hg__fail(HG_ERR_OOM, "atlas: entrada maior que o atlas");

    hg_urect rect;
    while (!hg_pack_alloc(&a->pack, w, h, &rect)) {
        uint32_t victim = oldest_slot(a, frame);
        if (victim == HG_MAP_EMPTY) return hg__fail(HG_ERR_OOM, "atlas: cheio no frame atual");
        evict_slot(a, victim);
    }
    slot = new_slot(a);
    if (slot == HG_MAP_EMPTY || !hg_map_put(&a->map, key, slot)) {
        hg_pack_release(&a->pack, rect);
        return hg__fail(HG_ERR_OOM, "atlas: sem memória");
    }
    a->entries[slot] = (hg_atlas_entry){ key, frame, rect, 0, 1 };
    a->alive_count++;
    *out_uv = to_rect(rect);
    *out_inserted = 1;
    return HG_OK;
}

int hg__atlas_contains(const hg_atlas* a, uint64_t key) {
    return a && hg_map_get(&a->map, key) != HG_MAP_EMPTY;
}

uint32_t hg__atlas_entry_count(const hg_atlas* a) { return a ? a->alive_count : 0; }

int hg__atlas_get_user(const hg_atlas* a, uint64_t key, uint64_t* out) {
    uint32_t slot = a ? hg_map_get(&a->map, key) : HG_MAP_EMPTY;
    if (slot == HG_MAP_EMPTY) return 0;
    *out = a->entries[slot].user;
    return 1;
}

void hg__atlas_set_user(hg_atlas* a, uint64_t key, uint64_t user) {
    uint32_t slot = a ? hg_map_get(&a->map, key) : HG_MAP_EMPTY;
    if (slot != HG_MAP_EMPTY) a->entries[slot].user = user;
}

#include "arena.h"
#include <stdint.h>
#include <stdlib.h>

struct hg_arena_block {
    hg_arena_block* next;
    size_t          capacity;
    size_t          offset;
    /* dados seguem o cabeçalho */
};

static unsigned char* block_data(hg_arena_block* b) { return (unsigned char*)(b + 1); }

static hg_arena_block* block_new(size_t min_size) {
    size_t cap = min_size > HG_ARENA_BLOCK_SIZE ? min_size : HG_ARENA_BLOCK_SIZE;
    hg_arena_block* b = malloc(sizeof *b + cap);
    if (!b) return NULL;
    b->next = NULL;
    b->capacity = cap;
    b->offset = 0;
    return b;
}

void hg_arena_init(hg_arena* arena) {
    arena->first = NULL;
    arena->current = NULL;
    arena->used_bytes = 0;
}

void hg_arena_free(hg_arena* arena) {
    hg_arena_block* b = arena->first;
    while (b) {
        hg_arena_block* next = b->next;
        free(b);
        b = next;
    }
    hg_arena_init(arena);
}

static void* try_block(hg_arena_block* b, size_t size, size_t align) {
    uintptr_t base = (uintptr_t)block_data(b);
    uintptr_t at = (base + b->offset + (align - 1)) & ~(uintptr_t)(align - 1);
    if (at + size > base + b->capacity) return NULL;
    b->offset = (size_t)(at - base) + size;
    return (void*)at;
}

void* hg_arena_alloc(hg_arena* arena, size_t size, size_t align) {
    if (align == 0) align = 1;
    /* Reaproveita blocos existentes (pós-reset) antes de pedir memória nova. */
    while (arena->current) {
        void* p = try_block(arena->current, size, align);
        if (p) { arena->used_bytes += size; return p; }
        if (!arena->current->next) break;
        arena->current = arena->current->next;
        arena->current->offset = 0;
    }
    hg_arena_block* b = block_new(size + align);
    if (!b) return NULL;
    if (arena->current) arena->current->next = b; else arena->first = b;
    arena->current = b;
    arena->used_bytes += size;
    return try_block(b, size, align);
}

void hg_arena_reset(hg_arena* arena) {
    arena->current = arena->first;
    if (arena->current) arena->current->offset = 0;
    arena->used_bytes = 0;
}

size_t hg_arena_capacity(const hg_arena* arena) {
    size_t total = 0;
    for (hg_arena_block* b = arena->first; b; b = b->next) total += b->capacity;
    return total;
}

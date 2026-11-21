/* arena.h — alocador por blocos encadeados; reset mantém os blocos. */
#ifndef HG_ARENA_H
#define HG_ARENA_H
#include <stddef.h>

#define HG_ARENA_BLOCK_SIZE (64u * 1024u)

typedef struct hg_arena_block hg_arena_block;

typedef struct hg_arena {
    hg_arena_block* first;
    hg_arena_block* current;
    size_t          used_bytes; /* soma do que foi entregue desde o reset */
} hg_arena;

void   hg_arena_init(hg_arena* arena);
void   hg_arena_free(hg_arena* arena);
void*  hg_arena_alloc(hg_arena* arena, size_t size, size_t align);
void   hg_arena_reset(hg_arena* arena);
size_t hg_arena_capacity(const hg_arena* arena);
#endif

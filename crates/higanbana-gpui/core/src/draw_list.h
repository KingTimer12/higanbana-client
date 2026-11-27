/* draw_list.h — estrutura interna da draw list, lida pelos backends.
 * Primitivas são agrupadas em lotes (batch); cada mudança de clip abre lote novo. */
#ifndef HG_DRAW_LIST_H
#define HG_DRAW_LIST_H
#include "arena.h"
#include "hgui.h"

#define HG_QUAD_CHUNK 256
#define HG_CLIP_STACK_MAX 64

typedef struct hg_quad_chunk {
    struct hg_quad_chunk* next;
    uint32_t              count;
    hg_quad               items[HG_QUAD_CHUNK];
} hg_quad_chunk;

#define HG_IMAGE_CHUNK 64

/* Imagens em ordem de desenho; o backend agrupa vizinhas com a mesma imagem. */
typedef struct hg_image_chunk {
    struct hg_image_chunk* next;
    uint32_t               count;
    struct hg_image*       images[HG_IMAGE_CHUNK];
    hg_image_draw          items[HG_IMAGE_CHUNK];
} hg_image_chunk;

struct hg_text_run; /* definido pelo módulo de texto */

typedef struct hg_batch {
    struct hg_batch*    next;
    hg_rect             clip;
    int                 has_clip;
    hg_quad_chunk*      quads_first;
    hg_quad_chunk*      quads_last;
    uint32_t            quad_count;
    hg_image_chunk*     images_first;
    hg_image_chunk*     images_last;
    uint32_t            image_count;
    struct hg_text_run* texts_first;
    struct hg_text_run* texts_last;
    uint32_t            text_count;
    struct hg_shader*   shader;      /* != NULL: lote só de shader (sem quads/imagens/texto) */
    hg_shader_draw      shader_draw;
} hg_batch;

struct hg_draw_list {
    hg_arena  arena;
    hg_batch* first;
    hg_batch* last;
    uint32_t  quad_count;
    uint32_t  image_count;
    hg_rect   clips[HG_CLIP_STACK_MAX];
    uint32_t  clip_depth;
};

/* Lote corrente, abrindo um se preciso. NULL = sem memória. */
hg_batch* hg__draw_list_batch(hg_draw_list* dl);
/* Abre lote novo herdando o clip atual. NULL = sem memória. */
hg_batch* hg__draw_list_open_batch(hg_draw_list* dl);
#endif

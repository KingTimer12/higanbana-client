/* draw_image.c — grava imagens na draw list, em chunks na arena. */
#include "draw_list.h"
#include "error.h"

static hg_image_chunk* writable_chunk(hg_draw_list* dl, hg_batch* b) {
    hg_image_chunk* c = b->images_last;
    if (c && c->count < HG_IMAGE_CHUNK) return c;
    c = hg_arena_alloc(&dl->arena, sizeof *c, _Alignof(hg_image_chunk));
    if (!c) return NULL;
    c->next = NULL;
    c->count = 0;
    if (b->images_last) b->images_last->next = c; else b->images_first = c;
    b->images_last = c;
    return c;
}

hg_result hg_draw_image(hg_draw_list* dl, hg_image* img, const hg_image_draw* draw) {
    if (!dl || !img || !draw) return hg__fail(HG_ERR_INVALID, "hg_draw_image: argumento nulo");
    hg_batch* b = hg__draw_list_batch(dl);
    hg_image_chunk* c = b ? writable_chunk(dl, b) : NULL;
    if (!c) return hg__fail(HG_ERR_OOM, "hg_draw_image: sem memória");
    c->images[c->count] = img;
    c->items[c->count++] = *draw;
    b->image_count++;
    dl->image_count++;
    return HG_OK;
}

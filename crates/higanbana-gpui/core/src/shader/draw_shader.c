/* draw_shader.c — grava um shader na draw list: camada própria, lote só dele. */
#include "draw_list.h"
#include "error.h"

hg_result hg_draw_shader(hg_draw_list* dl, hg_shader* sh, const hg_shader_draw* d) {
    if (!dl || !sh || !d) return hg__fail(HG_ERR_INVALID, "hg_draw_shader: argumento nulo");
    hg_batch* b = hg__draw_list_open_batch(dl);
    if (!b) return hg__fail(HG_ERR_OOM, "hg_draw_shader: sem memória");
    b->shader = sh;
    b->shader_draw = *d;
    /* O que vier depois fica por cima, em lote próprio. */
    return hg__draw_list_open_batch(dl) ? HG_OK : hg__fail(HG_ERR_OOM, "hg_draw_shader: sem memória");
}

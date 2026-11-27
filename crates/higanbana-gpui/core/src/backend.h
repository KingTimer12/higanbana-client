/* backend.h — contrato que todo backend gráfico implementa. */
#ifndef HG_BACKEND_H
#define HG_BACKEND_H
#include "hgui.h"

struct hg_window;

typedef struct hg_backend_vtbl {
    hg_backend_kind kind;
    const char*     name;
    /* 1 = disponível; preenche adapter. Pode carregar o driver: só roda quando
     * o backend é pedido (criação de contexto ou troca). */
    int       (*probe)(hg_backend_info* info);
    /* 1 = provavelmente disponível; preenche adapter. Barato: não carrega driver
     * de GPU (usa hg__detect_*). Usado por hg_enumerate_backends. NULL = usa probe. */
    int       (*detect)(hg_backend_info* info);
    hg_result (*create)(hg_context* ctx);
    void      (*destroy)(hg_context* ctx);
    /* Superfície por janela. */
    hg_result (*attach)(hg_context* ctx, struct hg_window* win);
    void      (*detach)(hg_context* ctx, struct hg_window* win);
    hg_result (*resize)(struct hg_window* win, uint32_t w, uint32_t h);
    hg_result (*begin_frame)(struct hg_window* win, hg_color clear);
    hg_result (*submit)(struct hg_window* win, const hg_draw_list* dl);
    hg_result (*end_frame)(struct hg_window* win, int vsync);
    /* Texturas de imagem (opcionais: NULL = backend sem imagens, que são puladas).
     * create preenche img->gpu a partir de img->pixels; destroy zera img->gpu. */
    hg_result (*texture_create)(hg_context* ctx, struct hg_image* img);
    void      (*texture_destroy)(hg_context* ctx, struct hg_image* img);
    /* Testes: lê um pixel BGRA8 do backbuffer. */
    hg_result (*read_pixel)(struct hg_window* win, uint32_t x, uint32_t y, uint32_t* out);
    /* Opcional: reenvia img->pixels para a textura existente (NULL = destroy + create lazy). */
    hg_result (*texture_update)(hg_context* ctx, struct hg_image* img);
    /* Shaders do usuário (opcionais: NULL = lote de shader pulado). prepare cria sh->gpu;
     * se o compilador nativo falhar grava sh->error e devolve HG_ERR_DEVICE. */
    hg_result (*shader_prepare)(hg_context* ctx, struct hg_shader* sh);
    void      (*shader_release)(hg_context* ctx, struct hg_shader* sh);
} hg_backend_vtbl;

/* Registro: backends compilados nesta build, em ordem de fallback. */
const hg_backend_vtbl* const* hg__backends(int* count);
/* Disponível nesta máquina (probe cacheado). NULL se indisponível. */
const hg_backend_vtbl* hg__backend_find(hg_backend_kind kind, hg_backend_info* info);
#endif

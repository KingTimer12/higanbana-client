/* context.h — estado global de um contexto: backend escolhido e seus dados. */
#ifndef HG_CONTEXT_H
#define HG_CONTEXT_H
#include "backend.h"

struct hg_context {
    const hg_backend_vtbl* backend;
    hg_backend_info        info;
    void*                  backend_data; /* dono: backend->create/destroy */
    uint64_t               frame_index;
    struct hg_text_system* text;         /* criado sob demanda (hg_font_load) */
    hg_stats               stats;        /* backend preenche gpu_bytes/draw_calls */
    long long              last_frame_ticks;
    struct hg_image*       images;       /* lista p/ liberar/recriar texturas */
    struct hg_shader*      shaders;      /* lista p/ liberar o GPU na troca/destruição */
    uint64_t               image_ram;    /* bytes em hg_image.pixels (stats) */
    struct hg_window*      windows;      /* lista intrusiva (hg_window.next) p/ troca de backend */
};
/* stats.c */
void hg__stats_frame_end(hg_context* ctx);
#endif

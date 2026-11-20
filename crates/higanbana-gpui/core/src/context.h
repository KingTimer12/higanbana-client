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
};
#endif

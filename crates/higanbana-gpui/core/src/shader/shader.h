/* shader.h — shader do usuário: cópia do código por alvo + objeto GPU criado sob demanda. */
#ifndef HG_SHADER_H
#define HG_SHADER_H
#include "hgui.h"

struct hg_shader {
    hg_context*      ctx;
    char*            hlsl;
    char*            glsl_vs;
    char*            glsl_fs;
    char*            wgsl;
    uint32_t*        spirv;
    size_t           spirv_words;
    void*            gpu;      /* dono: backend->shader_prepare/shader_release */
    char*            error;    /* erro do compilador nativo; NULL = ok */
    struct hg_shader* prev;    /* lista do contexto */
    struct hg_shader* next;
};

/* Garante o objeto GPU no backend atual. HG_OK também se o backend não suporta shaders. */
hg_result hg__shader_gpu(hg_shader* sh);
/* Libera o objeto GPU de todos os shaders no backend atual (troca de backend / destroy). */
void      hg__shaders_release_gpu(hg_context* ctx);
#endif

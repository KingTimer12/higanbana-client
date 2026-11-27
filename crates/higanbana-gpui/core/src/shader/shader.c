/* shader.c — ciclo de vida dos shaders do usuário. */
#include "shader.h"
#include "context.h"
#include "error.h"
#include <stdlib.h>
#include <string.h>

static char* dup_str(const char* s) {
    if (!s) return NULL;
    size_t n = strlen(s) + 1;
    char* p = malloc(n);
    if (p) memcpy(p, s, n);
    return p;
}

static void free_code(hg_shader* sh) {
    free(sh->hlsl);
    free(sh->glsl_vs);
    free(sh->glsl_fs);
    free(sh->wgsl);
    free(sh->spirv);
    free(sh->error);
}

hg_result hg_shader_create(hg_context* ctx, const hg_shader_code* code, hg_shader** out) {
    if (!ctx || !code || !out) return hg__fail(HG_ERR_INVALID, "hg_shader_create: argumento nulo");
    *out = NULL;
    hg_shader* sh = calloc(1, sizeof *sh);
    if (!sh) return hg__fail(HG_ERR_OOM, "hg_shader_create: sem memória");
    sh->ctx = ctx;
    sh->hlsl = dup_str(code->hlsl);
    sh->glsl_vs = dup_str(code->glsl_vs);
    sh->glsl_fs = dup_str(code->glsl_fs);
    sh->wgsl = dup_str(code->wgsl);
    int ok = (!code->hlsl || sh->hlsl) && (!code->glsl_vs || sh->glsl_vs) &&
             (!code->glsl_fs || sh->glsl_fs) && (!code->wgsl || sh->wgsl);
    if (code->spirv && code->spirv_words) {
        sh->spirv = malloc(code->spirv_words * sizeof *sh->spirv);
        if (sh->spirv) {
            memcpy(sh->spirv, code->spirv, code->spirv_words * sizeof *sh->spirv);
            sh->spirv_words = code->spirv_words;
        } else ok = 0;
    }
    if (!ok) { free_code(sh); free(sh); return hg__fail(HG_ERR_OOM, "hg_shader_create: sem memória"); }
    sh->next = ctx->shaders;
    if (ctx->shaders) ctx->shaders->prev = sh;
    ctx->shaders = sh;
    *out = sh;
    return HG_OK;
}

void hg_shader_destroy(hg_shader* sh) {
    if (!sh) return;
    hg_context* ctx = sh->ctx;
    if (sh->gpu && ctx->backend && ctx->backend->shader_release) ctx->backend->shader_release(ctx, sh);
    if (sh->prev) sh->prev->next = sh->next; else ctx->shaders = sh->next;
    if (sh->next) sh->next->prev = sh->prev;
    free_code(sh);
    free(sh);
}

const char* hg_shader_error(const hg_shader* sh) { return sh ? sh->error : NULL; }

hg_result hg__shader_gpu(hg_shader* sh) {
    if (sh->gpu) return HG_OK;
    /* Já falhou neste backend: não recompila a cada quadro. */
    if (sh->error) return hg__fail(HG_ERR_DEVICE, sh->error);
    const hg_backend_vtbl* vt = sh->ctx->backend;
    if (!vt || !vt->shader_prepare) return HG_OK;
    return vt->shader_prepare(sh->ctx, sh);
}

void hg__shaders_release_gpu(hg_context* ctx) {
    const hg_backend_vtbl* vt = ctx->backend;
    for (hg_shader* s = ctx->shaders; s; s = s->next) {
        if (s->gpu && vt && vt->shader_release) vt->shader_release(ctx, s);
        s->gpu = NULL;
        /* O erro era do compilador deste backend: o próximo tenta de novo. */
        free(s->error);
        s->error = NULL;
    }
}

hg_result hg__shader_set_error(hg_shader* sh, const char* msg) {
    free(sh->error);
    sh->error = dup_str(msg && *msg ? msg : "shader: erro do compilador nativo");
    return hg__fail(HG_ERR_DEVICE, sh->error ? sh->error : "shader: sem memória para o erro");
}

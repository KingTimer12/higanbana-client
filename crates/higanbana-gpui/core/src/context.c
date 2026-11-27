/* context.c — criação do contexto com seleção de backend e fallback. */
#include "context.h"
#include "error.h"
#include "text.h"
#include "image.h"
#include "shader.h"
#include <stdlib.h>

static const hg_backend_kind g_fallback[] = { HG_BACKEND_D3D11, HG_BACKEND_OPENGL, HG_BACKEND_WGPU };

static hg_result try_backend(hg_context* ctx, hg_backend_kind kind) {
    const hg_backend_vtbl* vt = hg__backend_find(kind, &ctx->info);
    if (!vt) return HG_ERR_UNAVAILABLE;
    ctx->backend = vt;
    hg_result r = vt->create(ctx);
    if (r != HG_OK) ctx->backend = NULL;
    return r;
}

hg_result hg_context_create(hg_backend_kind preferred, hg_context** out) {
    if (!out) return hg__fail(HG_ERR_INVALID, "hg_context_create: out nulo");
    *out = NULL;
    hg_context* ctx = calloc(1, sizeof *ctx);
    if (!ctx) return hg__fail(HG_ERR_OOM, "hg_context_create: sem memória");

    hg_result r = HG_ERR_UNAVAILABLE;
    if (preferred != HG_BACKEND_AUTO) r = try_backend(ctx, preferred);
    for (size_t i = 0; r != HG_OK && i < sizeof g_fallback / sizeof g_fallback[0]; i++)
        if (g_fallback[i] != preferred) r = try_backend(ctx, g_fallback[i]);

    if (r != HG_OK) {
        free(ctx);
        return hg__fail(HG_ERR_UNAVAILABLE, "hg_context_create: nenhum backend disponível");
    }
    *out = ctx;
    return HG_OK;
}

void hg_context_destroy(hg_context* ctx) {
    if (!ctx) return;
    hg__text_system_destroy(ctx->text);
    hg__images_release_gpu(ctx);
    while (ctx->images) hg_image_destroy(ctx->images); /* imagens esquecidas pelo app */
    hg__shaders_release_gpu(ctx);
    while (ctx->shaders) hg_shader_destroy(ctx->shaders); /* shaders esquecidos pelo app */
    if (ctx->backend) ctx->backend->destroy(ctx);
    free(ctx);
}

hg_backend_kind hg_context_backend(const hg_context* ctx) {
    return ctx && ctx->backend ? ctx->backend->kind : HG_BACKEND_AUTO;
}

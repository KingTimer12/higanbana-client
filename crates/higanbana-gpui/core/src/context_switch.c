/* context_switch.c — troca de backend em runtime sem fechar as janelas. */
#include "context.h"
#include "error.h"
#include "text.h"
#include "image.h"
#include "shader.h"
#include "window.h"
#include <string.h>

static void detach_all(hg_context* ctx) {
    for (hg_window* w = ctx->windows; w; w = w->next) ctx->backend->detach(ctx, w);
}

static hg_result attach_all(hg_context* ctx) {
    for (hg_window* w = ctx->windows; w; w = w->next) {
        hg_result r = ctx->backend->attach(ctx, w);
        if (r != HG_OK) {
            for (hg_window* u = ctx->windows; u != w; u = u->next) ctx->backend->detach(ctx, u);
            return r;
        }
    }
    return HG_OK;
}

/* Cria `vt` e prende todas as janelas nele. Em falha deixa ctx sem backend. */
static hg_result bring_up(hg_context* ctx, const hg_backend_vtbl* vt, const hg_backend_info* info) {
    ctx->backend = vt;
    ctx->info = *info;
    hg_result r = vt->create(ctx);
    if (r == HG_OK) {
        r = attach_all(ctx);
        if (r != HG_OK) vt->destroy(ctx);
    }
    if (r != HG_OK) ctx->backend = NULL;
    return r;
}

hg_result hg_context_switch_backend(hg_context* ctx, hg_backend_kind kind) {
    if (!ctx || !ctx->backend) return hg__fail(HG_ERR_INVALID, "switch_backend: contexto nulo");
    if (kind == ctx->backend->kind) return HG_OK;
    hg_backend_info info;
    const hg_backend_vtbl* next = hg__backend_find(kind, &info);
    if (!next) return hg__fail(HG_ERR_UNAVAILABLE, "switch_backend: backend indisponível");

    const hg_backend_vtbl* prev = ctx->backend;
    hg_backend_info prev_info = ctx->info;
    detach_all(ctx);
    hg__images_release_gpu(ctx); /* recriadas sob demanda no backend novo */
    hg__shaders_release_gpu(ctx);
    prev->destroy(ctx);
    if (ctx->text) hg__atlas_clear(ctx->text->atlas); /* textura do atlas morreu com o backend */
    /* HWND que já teve swapchain flip-model (DXGI/Vulkan) não mostra o SwapBuffers
     * do GL: a imagem congelava. Entrando no GL, a janela ganha uma HWND nova. */
    if (kind == HG_BACKEND_OPENGL)
        for (hg_window* w = ctx->windows; w; w = w->next) hg__window_recreate_hwnd(w);

    hg_result r = bring_up(ctx, next, &info);
    if (r == HG_OK) return HG_OK;
    char why[256];
    strncpy_s(why, sizeof why, hg_last_error(), _TRUNCATE);
    if (bring_up(ctx, prev, &prev_info) != HG_OK)
        return hg__fail(HG_ERR_DEVICE, "switch_backend: falhou e não conseguiu voltar ao anterior");
    return hg__fail(r, why);
}

hg_backend_info hg_context_backend_info(const hg_context* ctx) {
    hg_backend_info none = { 0 };
    return ctx && ctx->backend ? ctx->info : none;
}

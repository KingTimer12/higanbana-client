/* image.c — ciclo de vida das imagens e texturas lazy por backend. */
#include "image.h"
#include "context.h"
#include "error.h"
#include <stdlib.h>
#include <string.h>

hg_result hg_image_create(hg_context* ctx, uint32_t width, uint32_t height, const uint8_t* rgba,
                          uint32_t stride, hg_image** out) {
    if (!ctx || !rgba || !out || width == 0 || height == 0 || stride < width * 4)
        return hg__fail(HG_ERR_INVALID, "hg_image_create: argumento inválido");
    *out = NULL;
    hg_image* img = calloc(1, sizeof *img);
    uint8_t* px = img ? malloc((size_t)width * height * 4) : NULL;
    if (!px) { free(img); return hg__fail(HG_ERR_OOM, "hg_image_create: sem memória"); }
    for (uint32_t y = 0; y < height; y++) memcpy(px + (size_t)y * width * 4, rgba + (size_t)y * stride, width * 4);
    img->ctx = ctx;
    img->width = width;
    img->height = height;
    img->pixels = px;
    img->next = ctx->images;
    if (ctx->images) ctx->images->prev = img;
    ctx->images = img;
    *out = img;
    return HG_OK;
}

void hg_image_destroy(hg_image* img) {
    if (!img) return;
    hg_context* ctx = img->ctx;
    if (img->gpu && ctx->backend && ctx->backend->texture_destroy) ctx->backend->texture_destroy(ctx, img);
    if (img->prev) img->prev->next = img->next; else ctx->images = img->next;
    if (img->next) img->next->prev = img->prev;
    free(img->pixels);
    free(img);
}

hg_result hg_image_update(hg_image* img, const uint8_t* rgba, uint32_t stride) {
    if (!img || !rgba || stride < img->width * 4) return hg__fail(HG_ERR_INVALID, "hg_image_update: argumento inválido");
    for (uint32_t y = 0; y < img->height; y++)
        memcpy(img->pixels + (size_t)y * img->width * 4, rgba + (size_t)y * stride, img->width * 4);
    if (!img->gpu) return HG_OK;
    const hg_backend_vtbl* vt = img->ctx->backend;
    if (vt->texture_update) return vt->texture_update(img->ctx, img);
    vt->texture_destroy(img->ctx, img); /* recria com os pixels novos no próximo desenho */
    return HG_OK;
}

void hg_image_size(const hg_image* img, uint32_t* width, uint32_t* height) {
    if (width) *width = img ? img->width : 0;
    if (height) *height = img ? img->height : 0;
}

hg_result hg__image_gpu(hg_image* img) {
    if (img->gpu) return HG_OK;
    const hg_backend_vtbl* vt = img->ctx->backend;
    if (!vt->texture_create) return HG_ERR_UNAVAILABLE;
    return vt->texture_create(img->ctx, img);
}

void hg__images_release_gpu(hg_context* ctx) {
    if (!ctx->backend || !ctx->backend->texture_destroy) return;
    for (hg_image* i = ctx->images; i; i = i->next)
        if (i->gpu) ctx->backend->texture_destroy(ctx, i);
}

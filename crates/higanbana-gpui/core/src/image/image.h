/* image.h — imagem do core: cópia dos pixels + textura do backend criada sob demanda. */
#ifndef HG_IMAGE_H
#define HG_IMAGE_H
#include "hgui.h"

struct hg_image {
    hg_context*      ctx;
    uint32_t         width, height;
    uint8_t*         pixels;     /* RGBA8 pré-multiplicado, compacto (stride = width*4) */
    void*            gpu;        /* dono: backend->texture_create/destroy */
    struct hg_image* prev;       /* lista do contexto */
    struct hg_image* next;
};

/* Garante a textura do backend atual. HG_ERR_UNAVAILABLE se o backend não tem imagens. */
hg_result hg__image_gpu(hg_image* img);
/* Libera as texturas de todas as imagens no backend atual (troca de backend / destroy). */
void      hg__images_release_gpu(hg_context* ctx);
#endif

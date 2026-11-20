/* d3d12_images.c — instâncias de imagem no anel do frame e desenho agrupado por textura
 * (vizinhas com a mesma textura viram um único draw instanciado). */
#include "d3d12_internal.h"
#include "draw_list.h"
#include "error.h"
#include "image.h"
#include <stdlib.h>
#include <string.h>

hg_result d3d12_images_prepare(d3d12_device* d, const hg_draw_list* dl) {
    d->images.instances = 0;
    if (dl->image_count == 0) return HG_OK;
    hg_result r = HG_OK;
    for (const hg_batch* b = dl->first; b && r == HG_OK; b = b->next)
        for (const hg_image_chunk* c = b->images_first; c && r == HG_OK; c = c->next)
            for (uint32_t i = 0; i < c->count && r == HG_OK; i++) r = hg__image_gpu(c->images[i]);
    if (r != HG_OK) return r;
    UINT bytes = dl->image_count * (UINT)sizeof(hg_image_draw);
    d3d12_alloc a;
    r = d3d12_upload_alloc(d->dev, d3d12_ring(d), bytes, 16, &a);
    if (r != HG_OK) return r;
    hg_image_draw* dst = (hg_image_draw*)a.cpu;
    for (const hg_batch* b = dl->first; b; b = b->next)
        for (const hg_image_chunk* c = b->images_first; c; c = c->next) {
            memcpy(dst, c->items, sizeof(hg_image_draw) * c->count);
            dst += c->count;
        }
    d->images.instances = a.gpu;
    d->images.instance_bytes = bytes;
    return HG_OK;
}

static void draw_run(d3d12_device* d, const hg_image* img, UINT count, UINT start) {
    UINT srv = ((const d3d12_texture*)img->gpu)->srv;
    ID3D12GraphicsCommandList_SetGraphicsRootDescriptorTable(d->list, 1, d3d12_srv_gpu(&d->srvs, srv));
    ID3D12GraphicsCommandList_DrawInstanced(d->list, 4, count, 0, start);
}

/* Desenha as imagens do lote a partir da instância `start`; devolve draw calls.
 * Troca para a root signature de imagem (sampler linear) e volta à principal no fim. */
UINT d3d12_images_draw(d3d12_device* d, const hg_batch* b, UINT start) {
    ID3D12GraphicsCommandList* l = d->list;
    D3D12_VERTEX_BUFFER_VIEW vb = { d->images.instances, d->images.instance_bytes, sizeof(hg_image_draw) };
    d3d12_use_root(d, d->pipeline.image_root);
    ID3D12GraphicsCommandList_SetPipelineState(l, d->pipeline.image);
    ID3D12GraphicsCommandList_IASetVertexBuffers(l, 0, 1, &vb);
    const hg_image* run_img = NULL;
    UINT run_start = start, run_len = 0, calls = 0;
    for (const hg_image_chunk* ch = b->images_first; ch; ch = ch->next)
        for (uint32_t i = 0; i < ch->count; i++) {
            if (ch->images[i] == run_img) { run_len++; continue; }
            if (run_len) { draw_run(d, run_img, run_len, run_start); calls++; }
            run_img = ch->images[i];
            run_start += run_len;
            run_len = 1;
        }
    if (run_len) { draw_run(d, run_img, run_len, run_start); calls++; }
    d3d12_use_root(d, d->pipeline.root);
    return calls;
}

/* Chamado com a GPU ociosa (device_release): solta tudo o que estava adiado. */
void d3d12_images_release(d3d12_device* d) {
    d3d12_textures_collect(d, 1);
    free(d->images.retired);
    memset(&d->images, 0, sizeof d->images);
}

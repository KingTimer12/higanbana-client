/* vk_images.c — garante as texturas das imagens do frame, sobe as instâncias no
 * buffer do frame e desenha agrupando vizinhas com a mesma textura. */
#include "vk_internal.h"
#include "draw_list.h"
#include "image.h"
#include <string.h>

/* Antes do render pass: textura criada aqui sobe por submissão própria. */
hg_result vk_images_prepare(vk_device* d, vk_frame* f, const hg_draw_list* dl) {
    if (dl->image_count == 0) return HG_OK;
    hg_result r = HG_OK;
    for (const hg_batch* b = dl->first; b && r == HG_OK; b = b->next)
        for (const hg_image_chunk* c = b->images_first; c && r == HG_OK; c = c->next)
            for (uint32_t i = 0; i < c->count && r == HG_OK; i++) r = hg__image_gpu(c->images[i]);
    if (r == HG_OK)
        r = vk_buffer_reserve(d, &f->images, (VkDeviceSize)dl->image_count * sizeof(hg_image_draw), VK_BUFFER_USAGE_VERTEX_BUFFER);
    if (r != HG_OK) return r;
    hg_image_draw* dst = f->images.mapped;
    for (const hg_batch* b = dl->first; b; b = b->next)
        for (const hg_image_chunk* c = b->images_first; c; c = c->next) {
            memcpy(dst, c->items, sizeof(hg_image_draw) * c->count);
            dst += c->count;
        }
    return HG_OK;
}

static void draw_run(vk_device* d, VkCommandBuffer cmd, const hg_image* img, uint32_t start, uint32_t count) {
    VkDescriptorSet set = vk_texture_set(img);
    d->api.vkCmdBindDescriptorSets(cmd, 0, d->pipeline.layout, 0, 1, &set, 0, NULL);
    d->api.vkCmdDraw(cmd, 4, count, 0, start);
}

/* Desenha as imagens do lote; devolve o número de draws. */
uint32_t vk_images_draw(vk_device* d, VkCommandBuffer cmd, vk_frame* f, const hg_batch* b, uint32_t start) {
    VkDeviceSize offset = 0;
    d->api.vkCmdBindPipeline(cmd, 0, d->pipeline.image);
    d->api.vkCmdBindVertexBuffers(cmd, 0, 1, &f->images.buffer, &offset);
    const hg_image* run_img = NULL;
    uint32_t run_start = start, run_len = 0, calls = 0;
    for (const hg_image_chunk* ch = b->images_first; ch; ch = ch->next)
        for (uint32_t i = 0; i < ch->count; i++) {
            if (ch->images[i] == run_img) { run_len++; continue; }
            if (run_len) { draw_run(d, cmd, run_img, run_start, run_len); calls++; }
            run_img = ch->images[i];
            run_start += run_len;
            run_len = 1;
        }
    if (run_len) { draw_run(d, cmd, run_img, run_start, run_len); calls++; }
    return calls;
}

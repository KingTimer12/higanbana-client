/* vk_glyphs.c — sink do glyph_frame: junta os R8 novos num rascunho de CPU, copia
 * para o staging do frame e grava vkCmdCopyBufferToImage; sobe as instâncias. */
#include "vk_atlas.h"
#include "draw_list.h"
#include "error.h"
#include <stdlib.h>
#include <string.h>

static int grow(void** p, size_t* cap, size_t need, size_t elem) {
    if (need <= *cap) return 1;
    size_t n = *cap ? *cap : 64;
    while (n < need) n *= 2;
    void* q = realloc(*p, n * elem);
    if (!q) return 0;
    *p = q;
    *cap = n;
    return 1;
}

static hg_result upload_r8(void* user, hg_rect at, const uint8_t* r8, uint32_t pitch) {
    vk_glyphs* g = user;
    uint32_t w = (uint32_t)at.w, h = (uint32_t)at.h;
    if (w == 0 || h == 0) return HG_OK;
    size_t offset = (g->pending_bytes + 3) & ~(size_t)3; /* bufferOffset múltiplo de 4 */
    size_t region_cap = g->region_capacity;
    if (!grow((void**)&g->pending, &g->pending_capacity, offset + (size_t)w * h, 1) ||
        !grow((void**)&g->regions, &region_cap, g->region_count + 1, sizeof *g->regions))
        return hg__fail(HG_ERR_OOM, "vulkan: sem memória para upload de glyph");
    g->region_capacity = (uint32_t)region_cap;
    for (uint32_t y = 0; y < h; y++) memcpy(g->pending + offset + (size_t)y * w, r8 + (size_t)y * pitch, w);
    VkBufferImageCopy c = { offset, 0, 0, { VK_ASPECT_COLOR, 0, 0, 1 }, { (int32_t)at.x, (int32_t)at.y, 0 }, { w, h, 1 } };
    g->regions[g->region_count++] = c;
    g->pending_bytes = offset + (size_t)w * h;
    return HG_OK;
}

static hg_result record_uploads(vk_device* d, vk_frame* f) {
    vk_glyphs* g = &d->glyphs;
    if (g->region_count) {
        hg_result r = vk_buffer_reserve(d, &f->staging, g->pending_bytes, VK_BUFFER_USAGE_TRANSFER_SRC);
        if (r != HG_OK) return r;
        memcpy(f->staging.mapped, g->pending, g->pending_bytes);
        vk_atlas_transition(d, f->cmd, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL);
        d->api.vkCmdCopyBufferToImage(f->cmd, f->staging.buffer, g->atlas, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                                      g->region_count, g->regions);
    }
    if (g->atlas_layout != VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL)
        vk_atlas_transition(d, f->cmd, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL);
    return HG_OK;
}

hg_result vk_glyphs_prepare(vk_device* d, hg_context* ctx, vk_frame* f, const hg_draw_list* dl) {
    vk_glyphs* g = &d->glyphs;
    g->frame.count = 0;
    g->pending_bytes = 0;
    g->region_count = 0;
    if (!hg__draw_list_has_text(dl)) return HG_OK;
    hg_result r = vk_atlas_ensure(d);
    if (r == HG_OK) r = hg__glyph_frame_collect(&g->frame, ctx, dl, upload_r8, g);
    if (r == HG_OK) r = record_uploads(d, f);
    if (r != HG_OK || g->frame.count == 0) return r;

    VkDeviceSize bytes = (VkDeviceSize)g->frame.count * sizeof(hg_glyph_instance);
    r = vk_buffer_reserve(d, &f->glyphs, bytes, VK_BUFFER_USAGE_VERTEX_BUFFER);
    if (r == HG_OK) memcpy(f->glyphs.mapped, g->frame.instances, (size_t)bytes);
    return r;
}

void vk_glyphs_bind(vk_device* d, VkCommandBuffer cmd, vk_frame* f) {
    VkDeviceSize offset = 0;
    d->api.vkCmdBindPipeline(cmd, 0, d->pipeline.glyph);
    d->api.vkCmdBindVertexBuffers(cmd, 0, 1, &f->glyphs.buffer, &offset);
    d->api.vkCmdBindDescriptorSets(cmd, 0, d->pipeline.layout, 0, 1, &d->glyphs.set, 0, NULL);
}

void vk_glyphs_release(vk_device* d) {
    vk_glyphs* g = &d->glyphs;
    vk_atlas_release(d);
    hg__glyph_frame_free(&g->frame);
    free(g->pending);
    free(g->regions);
    g->pending = NULL;
    g->regions = NULL;
    g->pending_bytes = g->pending_capacity = 0;
    g->region_count = g->region_capacity = 0;
}

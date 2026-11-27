/* vk_render.c — sobe quads no buffer do frame e grava um render pass com um
 * scissor por lote: quads primeiro, imagens, texto por cima (instanciado, strip de 4). */
#include "vk_internal.h"
#include "draw_list.h"
#include "error.h"
#include "text.h"
#include <string.h>

static hg_result upload_quads(vk_device* d, vk_frame* f, const hg_draw_list* dl) {
    hg_result r = vk_buffer_reserve(d, &f->quads, (VkDeviceSize)dl->quad_count * sizeof(hg_quad), VK_BUFFER_USAGE_VERTEX_BUFFER);
    if (r != HG_OK) return r;
    hg_quad* dst = f->quads.mapped;
    for (const hg_batch* b = dl->first; b; b = b->next)
        for (const hg_quad_chunk* c = b->quads_first; c; c = c->next) {
            memcpy(dst, c->items, sizeof(hg_quad) * c->count);
            dst += c->count;
        }
    return HG_OK;
}

/* Scissor do lote recortado ao framebuffer (Vulkan não aceita offset negativo). 0 se vazio. */
static int scissor_for(const hg_batch* b, const vk_surface* s, VkRect2D* out) {
    float x0 = 0, y0 = 0, x1 = (float)s->width, y1 = (float)s->height;
    if (b->has_clip) {
        x0 = b->clip.x > 0 ? b->clip.x : 0;
        y0 = b->clip.y > 0 ? b->clip.y : 0;
        if (b->clip.x + b->clip.w < x1) x1 = b->clip.x + b->clip.w;
        if (b->clip.y + b->clip.h < y1) y1 = b->clip.y + b->clip.h;
    }
    if (x1 <= x0 || y1 <= y0) return 0;
    out->offset.x = (int32_t)x0;
    out->offset.y = (int32_t)y0;
    out->extent.width = (uint32_t)(x1 - (float)out->offset.x);
    out->extent.height = (uint32_t)(y1 - (float)out->offset.y);
    return out->extent.width && out->extent.height;
}

static void bind_common(vk_device* d, VkCommandBuffer cmd, const vk_surface* s) {
    VkViewport vp = { 0, 0, (float)s->width, (float)s->height, 0, 1 };
    const float globals[4] = { (float)s->width, (float)s->height, (float)HG_GLYPH_ATLAS_SIZE, (float)HG_GLYPH_ATLAS_SIZE };
    d->api.vkCmdSetViewport(cmd, 0, 1, &vp);
    d->api.vkCmdPushConstants(cmd, d->pipeline.layout, VK_SHADER_STAGE_VERTEX | VK_SHADER_STAGE_FRAGMENT, 0, sizeof globals, globals);
}

static void bind_quads(vk_device* d, VkCommandBuffer cmd, vk_frame* f) {
    VkDeviceSize offset = 0;
    d->api.vkCmdBindPipeline(cmd, 0, d->pipeline.quad);
    d->api.vkCmdBindVertexBuffers(cmd, 0, 1, &f->quads.buffer, &offset);
}

static void record_batches(vk_device* d, hg_context* ctx, vk_surface* s, vk_frame* f, const hg_draw_list* dl) {
    VkCommandBuffer cmd = f->cmd;
    const hg_glyph_frame* gf = &d->glyphs.frame;
    uint32_t quad_start = 0, image_start = 0, glyph_start = 0, i = 0;
    for (const hg_batch* b = dl->first; b; b = b->next, i++) {
        if (b->shader) { /* lote só de shader: camada própria */
            VkRect2D sc;
            if (!scissor_for(b, s, &sc)) continue;
            d->api.vkCmdSetScissor(cmd, 0, 1, &sc);
            if (vk_user_shader_draw(d, s, f, b, &sc)) {
                ctx->stats.draw_calls++;
                bind_common(d, cmd, s);
            }
            continue;
        }
        uint32_t glyphs = gf->count ? gf->batch_counts[i] : 0;
        VkRect2D sc;
        if ((b->quad_count == 0 && b->image_count == 0 && glyphs == 0) || !scissor_for(b, s, &sc)) {
            quad_start += b->quad_count;
            image_start += b->image_count;
            glyph_start += glyphs;
            continue;
        }
        d->api.vkCmdSetScissor(cmd, 0, 1, &sc);
        if (b->quad_count) {
            bind_quads(d, cmd, f);
            d->api.vkCmdDraw(cmd, 4, b->quad_count, 0, quad_start);
            ctx->stats.draw_calls++;
            quad_start += b->quad_count;
        }
        if (b->image_count) {
            ctx->stats.draw_calls += vk_images_draw(d, cmd, f, b, image_start);
            image_start += b->image_count;
        }
        if (glyphs) {
            vk_glyphs_bind(d, cmd, f);
            d->api.vkCmdDraw(cmd, 4, glyphs, 0, glyph_start);
            ctx->stats.draw_calls++;
            glyph_start += glyphs;
        }
    }
}

hg_result vk_render_draw_list(vk_device* d, hg_context* ctx, vk_surface* s, const hg_draw_list* dl) {
    if (!s->active) return HG_OK;
    hg_result r = vk_frame_record(d, s);
    vk_frame* f = &s->frames[s->slot];
    if (r == HG_OK) r = vk_glyphs_prepare(d, ctx, f, dl); /* cópias do atlas: fora do render pass */
    if (r == HG_OK) r = vk_images_prepare(d, f, dl);
    if (r == HG_OK && dl->quad_count > 0) r = upload_quads(d, f, dl);
    if (r != HG_OK) return r;
    vk_shader_frame_prepare(d, s, f, dl); /* uniforms/sets/backdrop: fora do render pass */

    ctx->stats.draw_calls = 0;
    vk_frame_pass_begin(d, s);
    bind_common(d, f->cmd, s);
    record_batches(d, ctx, s, f, dl);
    d->api.vkCmdEndRenderPass(f->cmd);
    ctx->stats.gpu_bytes = vk_gpu_bytes(d, s);
    return HG_OK;
}

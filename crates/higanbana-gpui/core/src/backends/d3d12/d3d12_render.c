/* d3d12_render.c — sobe os quads no anel do frame e grava um lote por clip
 * (quads, imagens e depois texto), na mesma ordem do D3D11. */
#include "d3d12_internal.h"
#include "draw_list.h"
#include "error.h"
#include "text.h"
#include <string.h>

static hg_result upload_quads(d3d12_device* d, const hg_draw_list* dl, D3D12_VERTEX_BUFFER_VIEW* vb) {
    UINT bytes = dl->quad_count * (UINT)sizeof(hg_quad);
    d3d12_alloc a;
    hg_result r = d3d12_upload_alloc(d->dev, d3d12_ring(d), bytes, 16, &a);
    if (r != HG_OK) return r;
    hg_quad* dst = (hg_quad*)a.cpu;
    for (const hg_batch* b = dl->first; b; b = b->next)
        for (const hg_quad_chunk* c = b->quads_first; c; c = c->next) {
            memcpy(dst, c->items, sizeof(hg_quad) * c->count);
            dst += c->count;
        }
    *vb = (D3D12_VERTEX_BUFFER_VIEW){ a.gpu, bytes, sizeof(hg_quad) };
    return HG_OK;
}

void d3d12_use_root(d3d12_device* d, ID3D12RootSignature* root) {
    ID3D12GraphicsCommandList_SetGraphicsRootSignature(d->list, root);
    ID3D12GraphicsCommandList_SetGraphicsRoot32BitConstants(d->list, 0, 4, d->globals, 0);
}

/* Estado comum a quads, imagens e glyphs: definido uma vez por submit. */
static void bind_common(d3d12_device* d, const d3d12_surface* s) {
    ID3D12GraphicsCommandList* l = d->list;
    d->globals[0] = (float)s->width;
    d->globals[1] = (float)s->height;
    d->globals[2] = d->globals[3] = (float)HG_GLYPH_ATLAS_SIZE;
    D3D12_VIEWPORT vp = { 0, 0, (FLOAT)s->width, (FLOAT)s->height, 0, 1 };
    ID3D12GraphicsCommandList_SetDescriptorHeaps(l, 1, &d->srvs.heap);
    d3d12_use_root(d, d->pipeline.root);
    ID3D12GraphicsCommandList_IASetPrimitiveTopology(l, D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
    ID3D12GraphicsCommandList_RSSetViewports(l, 1, &vp);
}

static D3D12_RECT scissor_for(const hg_batch* b, const d3d12_surface* s) {
    D3D12_RECT r = { 0, 0, (LONG)s->width, (LONG)s->height };
    if (b->has_clip) {
        r.left = (LONG)b->clip.x;
        r.top = (LONG)b->clip.y;
        r.right = (LONG)(b->clip.x + b->clip.w);
        r.bottom = (LONG)(b->clip.y + b->clip.h);
    }
    return r;
}

hg_result d3d12_render_draw_list(d3d12_device* d, hg_context* ctx, d3d12_surface* s, const hg_draw_list* dl) {
    if (!d->recording) return hg__fail(HG_ERR_INVALID, "d3d12: submit fora de frame");
    D3D12_VERTEX_BUFFER_VIEW quads = { 0 };
    hg_result r = d3d12_glyphs_prepare(d, ctx, dl);
    if (r == HG_OK) r = d3d12_images_prepare(d, dl);
    if (r == HG_OK && dl->quad_count > 0) r = upload_quads(d, dl, &quads);
    if (r != HG_OK) return r;
    bind_common(d, s);
    ctx->stats.draw_calls = 0;

    /* Por lote: quads, imagens, texto por cima. */
    ID3D12GraphicsCommandList* l = d->list;
    UINT quad_start = 0, image_start = 0, glyph_start = 0, i = 0;
    for (const hg_batch* b = dl->first; b; b = b->next, i++) {
        UINT glyphs = d->glyphs.frame.count ? d->glyphs.frame.batch_counts[i] : 0;
        if (b->quad_count == 0 && b->image_count == 0 && glyphs == 0) continue;
        D3D12_RECT sc = scissor_for(b, s);
        ID3D12GraphicsCommandList_RSSetScissorRects(l, 1, &sc);
        if (b->quad_count) {
            ID3D12GraphicsCommandList_SetPipelineState(l, d->pipeline.quad);
            ID3D12GraphicsCommandList_IASetVertexBuffers(l, 0, 1, &quads);
            ID3D12GraphicsCommandList_DrawInstanced(l, 4, b->quad_count, 0, quad_start);
            ctx->stats.draw_calls++;
            quad_start += b->quad_count;
        }
        if (b->image_count) {
            ctx->stats.draw_calls += d3d12_images_draw(d, b, image_start);
            image_start += b->image_count;
        }
        if (glyphs) {
            d3d12_glyphs_bind(d);
            ID3D12GraphicsCommandList_DrawInstanced(l, 4, glyphs, 0, glyph_start);
            ctx->stats.draw_calls++;
            glyph_start += glyphs;
        }
    }
    ctx->stats.gpu_bytes = d3d12_gpu_bytes(d);
    return HG_OK;
}

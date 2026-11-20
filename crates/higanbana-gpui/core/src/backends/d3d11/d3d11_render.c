/* d3d11_render.c — sobe todos os quads num único Map e desenha um lote por clip. */
#include "d3d11_internal.h"
#include "draw_list.h"
#include "error.h"
#include "text.h"
#include <string.h>

static hg_result upload_quads(d3d11_device* d, const hg_draw_list* dl) {
    UINT bytes = dl->quad_count * (UINT)sizeof(hg_quad);
    hg_result r = d3d11_buffer_reserve(d->dev, &d->quads, bytes);
    if (r != HG_OK) return r;
    D3D11_MAPPED_SUBRESOURCE map;
    if (FAILED(ID3D11DeviceContext_Map(d->imm, (ID3D11Resource*)d->quads.buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, &map)))
        return hg__fail(HG_ERR_DEVICE, "d3d11: Map do instance buffer falhou");
    hg_quad* dst = map.pData;
    for (const hg_batch* b = dl->first; b; b = b->next)
        for (const hg_quad_chunk* c = b->quads_first; c; c = c->next) {
            memcpy(dst, c->items, sizeof(hg_quad) * c->count);
            dst += c->count;
        }
    ID3D11DeviceContext_Unmap(d->imm, (ID3D11Resource*)d->quads.buffer, 0);
    return HG_OK;
}

static void upload_globals(d3d11_device* d, const d3d11_surface* s) {
    D3D11_MAPPED_SUBRESOURCE map;
    if (FAILED(ID3D11DeviceContext_Map(d->imm, (ID3D11Resource*)d->pipeline.globals, 0, D3D11_MAP_WRITE_DISCARD, 0, &map)))
        return;
    float* g = map.pData;
    g[0] = (float)s->width;
    g[1] = (float)s->height;
    g[2] = g[3] = (float)HG_GLYPH_ATLAS_SIZE;
    ID3D11DeviceContext_Unmap(d->imm, (ID3D11Resource*)d->pipeline.globals, 0);
}

static void bind_quad_pipeline(d3d11_device* d, const d3d11_surface* s) {
    ID3D11DeviceContext* c = d->imm;
    const d3d11_pipeline* p = &d->pipeline;
    UINT stride = sizeof(hg_quad), offset = 0;
    D3D11_VIEWPORT vp = { 0, 0, (FLOAT)s->width, (FLOAT)s->height, 0, 1 };
    ID3D11DeviceContext_IASetPrimitiveTopology(c, D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
    ID3D11DeviceContext_IASetInputLayout(c, p->quad_layout);
    ID3D11DeviceContext_IASetVertexBuffers(c, 0, 1, &d->quads.buffer, &stride, &offset);
    ID3D11DeviceContext_VSSetShader(c, p->quad_vs, NULL, 0);
    ID3D11DeviceContext_VSSetConstantBuffers(c, 0, 1, &p->globals);
    ID3D11DeviceContext_PSSetShader(c, p->quad_ps, NULL, 0);
    ID3D11DeviceContext_RSSetState(c, p->raster);
    ID3D11DeviceContext_RSSetViewports(c, 1, &vp);
    ID3D11DeviceContext_OMSetBlendState(c, p->blend, NULL, 0xffffffff);
}

static D3D11_RECT scissor_for(const hg_batch* b, const d3d11_surface* s) {
    D3D11_RECT r = { 0, 0, (LONG)s->width, (LONG)s->height };
    if (b->has_clip) {
        r.left = (LONG)b->clip.x;
        r.top = (LONG)b->clip.y;
        r.right = (LONG)(b->clip.x + b->clip.w);
        r.bottom = (LONG)(b->clip.y + b->clip.h);
    }
    return r;
}

hg_result d3d11_render_draw_list(d3d11_device* d, hg_context* ctx, d3d11_surface* s, const hg_draw_list* dl) {
    hg_result r = d3d11_glyphs_prepare(d, ctx, dl);
    if (r == HG_OK && dl->quad_count > 0) r = upload_quads(d, dl);
    if (r != HG_OK) return r;
    upload_globals(d, s);
    ctx->stats.draw_calls = 0;

    /* Por lote: quads primeiro, texto por cima. */
    UINT quad_start = 0, glyph_start = 0, i = 0;
    for (const hg_batch* b = dl->first; b; b = b->next, i++) {
        UINT glyphs = d->glyphs.count ? d->glyphs.batch_counts[i] : 0;
        if (b->quad_count == 0 && glyphs == 0) continue;
        D3D11_RECT sc = scissor_for(b, s);
        ID3D11DeviceContext_RSSetScissorRects(d->imm, 1, &sc);
        if (b->quad_count) {
            bind_quad_pipeline(d, s);
            ID3D11DeviceContext_DrawInstanced(d->imm, 4, b->quad_count, 0, quad_start);
            ctx->stats.draw_calls++;
            quad_start += b->quad_count;
        }
        if (glyphs) {
            d3d11_glyphs_bind(d);
            ID3D11DeviceContext_DrawInstanced(d->imm, 4, glyphs, 0, glyph_start);
            ctx->stats.draw_calls++;
            glyph_start += glyphs;
        }
    }
    ctx->stats.gpu_bytes = d3d11_gpu_bytes(d);
    return HG_OK;
}

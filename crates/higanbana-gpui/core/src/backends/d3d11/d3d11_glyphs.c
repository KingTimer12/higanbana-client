/* d3d11_glyphs.c — textura do atlas de glyphs, pipeline de glyph e sink que
 * coleta as instâncias geradas pelo sistema de texto. */
#include "d3d11_internal.h"
#include "draw_list.h"
#include "error.h"
#include "text.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include "glyph_vs.h" /* gerado: hg_glyph_vs[] */
#include "glyph_ps.h" /* gerado: hg_glyph_ps[] */

#define INSTANCE(name, field) \
    { name, 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 0, (UINT)offsetof(hg_glyph_instance, field), D3D11_INPUT_PER_INSTANCE_DATA, 1 }

static const D3D11_INPUT_ELEMENT_DESC k_glyph_layout[] = {
    INSTANCE("RECT", rect),
    INSTANCE("UV", uv),
    INSTANCE("COLOR", color),
};

static hg_result create_atlas_texture(ID3D11Device* dev, d3d11_glyphs* g) {
    D3D11_TEXTURE2D_DESC td = { 0 };
    td.Width = td.Height = HG_GLYPH_ATLAS_SIZE;
    td.MipLevels = td.ArraySize = 1;
    td.Format = DXGI_FORMAT_R8_UNORM;
    td.SampleDesc.Count = 1;
    td.Usage = D3D11_USAGE_DEFAULT;
    td.BindFlags = D3D11_BIND_SHADER_RESOURCE;
    HRESULT hr = ID3D11Device_CreateTexture2D(dev, &td, NULL, &g->atlas);
    if (SUCCEEDED(hr)) hr = ID3D11Device_CreateShaderResourceView(dev, (ID3D11Resource*)g->atlas, NULL, &g->atlas_srv);
    return SUCCEEDED(hr) ? HG_OK : hg__fail(HG_ERR_OOM, "d3d11: textura do atlas falhou");
}

static hg_result ensure_pipeline(ID3D11Device* dev, d3d11_glyphs* g) {
    if (g->vs) return HG_OK;
    D3D11_SAMPLER_DESC sd = { 0 };
    sd.Filter = D3D11_FILTER_MIN_MAG_MIP_POINT;
    sd.AddressU = sd.AddressV = sd.AddressW = D3D11_TEXTURE_ADDRESS_CLAMP;
    sd.MaxLOD = D3D11_FLOAT32_MAX;
    HRESULT hr = ID3D11Device_CreateVertexShader(dev, hg_glyph_vs, sizeof hg_glyph_vs, NULL, &g->vs);
    if (SUCCEEDED(hr)) hr = ID3D11Device_CreatePixelShader(dev, hg_glyph_ps, sizeof hg_glyph_ps, NULL, &g->ps);
    if (SUCCEEDED(hr))
        hr = ID3D11Device_CreateInputLayout(dev, k_glyph_layout, ARRAYSIZE(k_glyph_layout), hg_glyph_vs,
                                            sizeof hg_glyph_vs, &g->layout);
    if (SUCCEEDED(hr)) hr = ID3D11Device_CreateSamplerState(dev, &sd, &g->sampler);
    if (FAILED(hr)) return hg__fail(HG_ERR_DEVICE, "d3d11: pipeline de glyph falhou");
    g->instances_gpu.bind = D3D11_BIND_VERTEX_BUFFER;
    return create_atlas_texture(dev, g);
}

void d3d11_glyphs_release(d3d11_glyphs* g) {
    d3d11_buffer_release(&g->instances_gpu);
    D3D11_RELEASE(g->sampler);
    D3D11_RELEASE(g->atlas_srv);
    D3D11_RELEASE(g->atlas);
    D3D11_RELEASE(g->layout);
    D3D11_RELEASE(g->ps);
    D3D11_RELEASE(g->vs);
    free(g->instances);
    free(g->batch_counts);
    g->instances = NULL;
    g->batch_counts = NULL;
    g->count = g->capacity = g->batch_capacity = 0;
}

/* ---- sink --------------------------------------------------------------- */

typedef struct sink_state {
    d3d11_device* d;
} sink_state;

static hg_result sink_upload(void* user, hg_rect at, const uint8_t* r8, uint32_t pitch) {
    d3d11_device* d = ((sink_state*)user)->d;
    D3D11_BOX box = { (UINT)at.x, (UINT)at.y, 0, (UINT)(at.x + at.w), (UINT)(at.y + at.h), 1 };
    ID3D11DeviceContext_UpdateSubresource(d->imm, (ID3D11Resource*)d->glyphs.atlas, 0, &box, r8, pitch, 0);
    return HG_OK;
}

static hg_result sink_emit(void* user, const hg_glyph_instance* glyph) {
    d3d11_glyphs* g = &((sink_state*)user)->d->glyphs;
    if (g->count == g->capacity) {
        uint32_t cap = g->capacity ? g->capacity * 2 : 1024;
        hg_glyph_instance* grown = realloc(g->instances, sizeof *grown * cap);
        if (!grown) return hg__fail(HG_ERR_OOM, "d3d11: sem memória para glyphs");
        g->instances = grown;
        g->capacity = cap;
    }
    g->instances[g->count++] = *glyph;
    return HG_OK;
}

static hg_result reserve_batches(d3d11_glyphs* g, uint32_t batches) {
    if (batches <= g->batch_capacity) return HG_OK;
    uint32_t* grown = realloc(g->batch_counts, sizeof *grown * batches);
    if (!grown) return hg__fail(HG_ERR_OOM, "d3d11: sem memória para lotes");
    g->batch_counts = grown;
    g->batch_capacity = batches;
    return HG_OK;
}

hg_result d3d11_glyphs_prepare(d3d11_device* d, hg_context* ctx, const hg_draw_list* dl) {
    d3d11_glyphs* g = &d->glyphs;
    g->count = 0;
    uint32_t batches = 0, texts = 0;
    for (const hg_batch* b = dl->first; b; b = b->next, batches++) texts += b->text_count;
    if (texts == 0) return HG_OK;
    if (!ctx->text) return hg__fail(HG_ERR_INVALID, "texto desenhado sem fonte carregada");

    hg_result r = ensure_pipeline(d->dev, g);
    if (r == HG_OK) r = reserve_batches(g, batches);
    sink_state state = { d };
    hg_glyph_sink sink = { &state, sink_upload, sink_emit };
    uint32_t i = 0;
    for (const hg_batch* b = dl->first; b && r == HG_OK; b = b->next, i++) {
        uint32_t before = g->count;
        for (const hg_text_run* run = b->texts_first; run && r == HG_OK; run = run->next)
            r = hg__text_render_run(ctx->text, run, ctx->frame_index, &sink);
        g->batch_counts[i] = g->count - before;
    }
    if (r != HG_OK || g->count == 0) return r;

    r = d3d11_buffer_reserve(d->dev, &g->instances_gpu, g->count * (UINT)sizeof(hg_glyph_instance));
    D3D11_MAPPED_SUBRESOURCE map;
    if (r == HG_OK &&
        FAILED(ID3D11DeviceContext_Map(d->imm, (ID3D11Resource*)g->instances_gpu.buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, &map)))
        r = hg__fail(HG_ERR_DEVICE, "d3d11: Map de glyphs falhou");
    if (r != HG_OK) return r;
    memcpy(map.pData, g->instances, sizeof(hg_glyph_instance) * g->count);
    ID3D11DeviceContext_Unmap(d->imm, (ID3D11Resource*)g->instances_gpu.buffer, 0);
    return HG_OK;
}

void d3d11_glyphs_bind(d3d11_device* d) {
    ID3D11DeviceContext* c = d->imm;
    d3d11_glyphs* g = &d->glyphs;
    UINT stride = sizeof(hg_glyph_instance), offset = 0;
    ID3D11DeviceContext_IASetInputLayout(c, g->layout);
    ID3D11DeviceContext_IASetVertexBuffers(c, 0, 1, &g->instances_gpu.buffer, &stride, &offset);
    ID3D11DeviceContext_VSSetShader(c, g->vs, NULL, 0);
    ID3D11DeviceContext_PSSetShader(c, g->ps, NULL, 0);
    ID3D11DeviceContext_PSSetConstantBuffers(c, 0, 1, &d->pipeline.globals);
    ID3D11DeviceContext_PSSetShaderResources(c, 0, 1, &g->atlas_srv);
    ID3D11DeviceContext_PSSetSamplers(c, 0, 1, &g->sampler);
}

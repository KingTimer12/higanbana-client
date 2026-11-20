/* d3d11_images.c — texturas de imagem, pipeline de imagem e desenho agrupado por textura. */
#include "d3d11_internal.h"
#include "draw_list.h"
#include "error.h"
#include "image.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include "image_vs.h" /* gerado: hg_image_vs[] */
#include "image_ps.h" /* gerado: hg_image_ps[] */

#define INSTANCE(name, field) \
    { name, 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 0, (UINT)offsetof(hg_image_draw, field), D3D11_INPUT_PER_INSTANCE_DATA, 1 }

static const D3D11_INPUT_ELEMENT_DESC k_image_layout[] = {
    INSTANCE("RECT", rect), INSTANCE("UV", uv), INSTANCE("TINT", tint), INSTANCE("RADIUS", radius),
};

typedef struct d3d11_texture {
    ID3D11Texture2D*          tex;
    ID3D11ShaderResourceView* srv;
} d3d11_texture;

hg_result d3d11_texture_create(d3d11_device* d, hg_image* img) {
    d3d11_texture* t = calloc(1, sizeof *t);
    if (!t) return hg__fail(HG_ERR_OOM, "d3d11: sem memória para textura");
    D3D11_TEXTURE2D_DESC td = { img->width, img->height, 1, 1, DXGI_FORMAT_R8G8B8A8_UNORM, { 1, 0 },
                                D3D11_USAGE_IMMUTABLE, D3D11_BIND_SHADER_RESOURCE, 0, 0 };
    D3D11_SUBRESOURCE_DATA init = { img->pixels, img->width * 4, 0 };
    HRESULT hr = ID3D11Device_CreateTexture2D(d->dev, &td, &init, &t->tex);
    if (SUCCEEDED(hr)) hr = ID3D11Device_CreateShaderResourceView(d->dev, (ID3D11Resource*)t->tex, NULL, &t->srv);
    if (FAILED(hr)) {
        D3D11_RELEASE(t->tex);
        free(t);
        return hg__fail(HG_ERR_OOM, "d3d11: textura de imagem falhou");
    }
    img->gpu = t;
    d->images.texture_bytes += (uint64_t)img->width * img->height * 4;
    return HG_OK;
}

void d3d11_texture_destroy(d3d11_device* d, hg_image* img) {
    d3d11_texture* t = img->gpu;
    if (!t) return;
    D3D11_RELEASE(t->srv);
    D3D11_RELEASE(t->tex);
    free(t);
    img->gpu = NULL;
    d->images.texture_bytes -= (uint64_t)img->width * img->height * 4;
}

static hg_result ensure_pipeline(ID3D11Device* dev, d3d11_images* p) {
    if (p->sampler) return HG_OK; /* último objeto criado */
    D3D11_SAMPLER_DESC sd = { 0 };
    sd.Filter = D3D11_FILTER_MIN_MAG_MIP_LINEAR;
    sd.AddressU = sd.AddressV = sd.AddressW = D3D11_TEXTURE_ADDRESS_CLAMP;
    sd.MaxLOD = D3D11_FLOAT32_MAX;
    HRESULT hr = ID3D11Device_CreateVertexShader(dev, hg_image_vs, sizeof hg_image_vs, NULL, &p->vs);
    if (SUCCEEDED(hr)) hr = ID3D11Device_CreatePixelShader(dev, hg_image_ps, sizeof hg_image_ps, NULL, &p->ps);
    if (SUCCEEDED(hr))
        hr = ID3D11Device_CreateInputLayout(dev, k_image_layout, ARRAYSIZE(k_image_layout), hg_image_vs,
                                            sizeof hg_image_vs, &p->layout);
    if (SUCCEEDED(hr)) hr = ID3D11Device_CreateSamplerState(dev, &sd, &p->sampler);
    p->instances.bind = D3D11_BIND_VERTEX_BUFFER;
    if (SUCCEEDED(hr)) return HG_OK;
    d3d11_images_release(p);
    return hg__fail(HG_ERR_DEVICE, "d3d11: pipeline de imagem falhou");
}

void d3d11_images_release(d3d11_images* p) {
    d3d11_buffer_release(&p->instances);
    D3D11_RELEASE(p->sampler);
    D3D11_RELEASE(p->layout);
    D3D11_RELEASE(p->ps);
    D3D11_RELEASE(p->vs);
}

hg_result d3d11_images_prepare(d3d11_device* d, const hg_draw_list* dl) {
    if (dl->image_count == 0) return HG_OK;
    hg_result r = ensure_pipeline(d->dev, &d->images);
    for (const hg_batch* b = dl->first; b && r == HG_OK; b = b->next)
        for (const hg_image_chunk* c = b->images_first; c && r == HG_OK; c = c->next)
            for (uint32_t i = 0; i < c->count && r == HG_OK; i++) r = hg__image_gpu(c->images[i]);
    if (r == HG_OK) r = d3d11_buffer_reserve(d->dev, &d->images.instances, dl->image_count * (UINT)sizeof(hg_image_draw));
    D3D11_MAPPED_SUBRESOURCE map;
    if (r == HG_OK &&
        FAILED(ID3D11DeviceContext_Map(d->imm, (ID3D11Resource*)d->images.instances.buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, &map)))
        r = hg__fail(HG_ERR_DEVICE, "d3d11: Map de imagens falhou");
    if (r != HG_OK) return r;
    hg_image_draw* dst = map.pData;
    for (const hg_batch* b = dl->first; b; b = b->next)
        for (const hg_image_chunk* c = b->images_first; c; c = c->next) {
            memcpy(dst, c->items, sizeof(hg_image_draw) * c->count);
            dst += c->count;
        }
    ID3D11DeviceContext_Unmap(d->imm, (ID3D11Resource*)d->images.instances.buffer, 0);
    return HG_OK;
}

/* Desenha as imagens do lote; vizinhas com a mesma textura viram um único draw. */
UINT d3d11_images_draw(d3d11_device* d, const hg_batch* b, UINT start) {
    ID3D11DeviceContext* c = d->imm;
    const d3d11_images* p = &d->images;
    UINT stride = sizeof(hg_image_draw), offset = 0, calls = 0;
    ID3D11DeviceContext_IASetInputLayout(c, p->layout);
    ID3D11DeviceContext_IASetVertexBuffers(c, 0, 1, &p->instances.buffer, &stride, &offset);
    ID3D11DeviceContext_VSSetShader(c, p->vs, NULL, 0);
    ID3D11DeviceContext_PSSetShader(c, p->ps, NULL, 0);
    ID3D11DeviceContext_PSSetSamplers(c, 0, 1, &p->sampler);
    const hg_image* run_img = NULL;
    UINT run_start = start, run_len = 0;
    for (const hg_image_chunk* ch = b->images_first;; ch = ch->next) {
        for (uint32_t i = 0; ch && i < ch->count; i++) {
            if (ch->images[i] == run_img) { run_len++; continue; }
            if (run_len) {
                ID3D11DeviceContext_PSSetShaderResources(c, 0, 1, &((d3d11_texture*)run_img->gpu)->srv);
                ID3D11DeviceContext_DrawInstanced(c, 4, run_len, 0, run_start);
                calls++;
            }
            run_img = ch->images[i];
            run_start += run_len;
            run_len = 1;
        }
        if (!ch) break;
    }
    if (run_len) {
        ID3D11DeviceContext_PSSetShaderResources(c, 0, 1, &((d3d11_texture*)run_img->gpu)->srv);
        ID3D11DeviceContext_DrawInstanced(c, 4, run_len, 0, run_start);
        calls++;
    }
    return calls;
}

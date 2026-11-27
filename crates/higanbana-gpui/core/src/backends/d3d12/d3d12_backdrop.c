/* d3d12_backdrop.c — textura por superfície com o que já foi desenhado atrás de um shader.
 * Só existe depois do 1º shader com HG_SHADER_BACKDROP; cada desenho copia apenas o
 * retângulo dele + margem (barreiras RENDER_TARGET->COPY_SOURCE->RENDER_TARGET no backbuffer),
 * então dois vidros sobrepostos veem um ao outro. */
#include "d3d12_internal.h"
#include <math.h>

static int ensure_texture(d3d12_device* d, d3d12_surface* s) {
    d3d12_backdrop* b = &s->backdrop;
    if (b->tex) return 1; /* resize solta a textura: se existe, tem o tamanho atual */
    D3D12_HEAP_PROPERTIES heap = { D3D12_HEAP_TYPE_DEFAULT, 0, 0, 1, 1 };
    D3D12_RESOURCE_DESC rd = { 0 };
    rd.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
    rd.Width = s->width;
    rd.Height = s->height;
    rd.DepthOrArraySize = rd.MipLevels = 1;
    rd.Format = DXGI_FORMAT_B8G8R8A8_UNORM; /* mesmo formato do swapchain */
    rd.SampleDesc.Count = 1;
    UINT srv = 0;
    if (d3d12_srv_alloc(&d->srvs, &srv) != HG_OK) return 0;
    if (FAILED(ID3D12Device_CreateCommittedResource(d->dev, &heap, D3D12_HEAP_FLAG_NONE, &rd,
                                                    D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, NULL,
                                                    &IID_ID3D12Resource, (void**)&b->tex))) {
        d3d12_srv_free(&d->srvs, srv);
        return 0;
    }
    d3d12_srv_write_bgra(d->dev, &d->srvs, srv, b->tex);
    b->srv = srv;
    return 1;
}

static UINT clamp_px(float v, UINT max) {
    if (!(v > 0.0f)) return 0; /* também pega NaN */
    if (v > (float)max) return max;
    return (UINT)v;
}

static void transition(ID3D12GraphicsCommandList* l, ID3D12Resource* back, ID3D12Resource* copy,
                       D3D12_RESOURCE_STATES back_from, D3D12_RESOURCE_STATES back_to,
                       D3D12_RESOURCE_STATES copy_from, D3D12_RESOURCE_STATES copy_to) {
    D3D12_RESOURCE_BARRIER b[2] = { 0 };
    ID3D12Resource* res[2] = { back, copy };
    D3D12_RESOURCE_STATES from[2] = { back_from, copy_from }, to[2] = { back_to, copy_to };
    for (int i = 0; i < 2; i++) {
        b[i].Type = D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;
        b[i].Transition.pResource = res[i];
        b[i].Transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
        b[i].Transition.StateBefore = from[i];
        b[i].Transition.StateAfter = to[i];
    }
    ID3D12GraphicsCommandList_ResourceBarrier(l, 2, b);
}

UINT d3d12_backdrop_capture(d3d12_device* d, d3d12_surface* s, const hg_rect* r) {
    const float m = D3D12_BACKDROP_MARGIN;
    UINT left = clamp_px(floorf(r->x - m), s->width);
    UINT top = clamp_px(floorf(r->y - m), s->height);
    UINT right = clamp_px(ceilf(r->x + r->w + m), s->width);
    UINT bottom = clamp_px(ceilf(r->y + r->h + m), s->height);
    if (left >= right || top >= bottom) return 0; /* fora da superfície: nada a copiar */
    if (!ensure_texture(d, s)) return 0;

    ID3D12GraphicsCommandList* l = d->list;
    ID3D12Resource* back = d3d12_surface_current(s);
    ID3D12Resource* copy = s->backdrop.tex;
    transition(l, back, copy, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE,
               D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_COPY_DEST);
    D3D12_TEXTURE_COPY_LOCATION dst = { copy, D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, { 0 } };
    D3D12_TEXTURE_COPY_LOCATION src = { back, D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, { 0 } };
    D3D12_BOX box = { left, top, 0, right, bottom, 1 };
    ID3D12GraphicsCommandList_CopyTextureRegion(l, &dst, left, top, 0, &src, &box);
    transition(l, back, copy, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET,
               D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
    return s->backdrop.srv;
}

void d3d12_backdrop_release(d3d12_device* d, d3d12_backdrop* b) {
    D3D12_RELEASE(b->tex);
    if (b->srv) d3d12_srv_free(&d->srvs, b->srv);
    b->srv = 0;
}

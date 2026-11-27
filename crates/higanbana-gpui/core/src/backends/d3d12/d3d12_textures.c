/* d3d12_textures.c — texturas de imagem: recurso DEFAULT RGBA8, cópia pelo anel de upload
 * na lista do frame e SRV no heap do device. A soltura espera a fence do último uso. */
#include "d3d12_internal.h"
#include "error.h"
#include "image.h"
#include <stdlib.h>
#include <string.h>

static hg_result create_resource(ID3D12Device* dev, const hg_image* img, ID3D12Resource** out) {
    D3D12_HEAP_PROPERTIES heap = { D3D12_HEAP_TYPE_DEFAULT, 0, 0, 1, 1 };
    D3D12_RESOURCE_DESC rd = { 0 };
    rd.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
    rd.Width = img->width;
    rd.Height = img->height;
    rd.DepthOrArraySize = rd.MipLevels = 1;
    rd.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
    rd.SampleDesc.Count = 1;
    if (FAILED(ID3D12Device_CreateCommittedResource(dev, &heap, D3D12_HEAP_FLAG_NONE, &rd,
                                                    D3D12_RESOURCE_STATE_COPY_DEST, NULL, &IID_ID3D12Resource,
                                                    (void**)out)))
        return hg__fail(HG_ERR_OOM, "d3d12: textura de imagem falhou");
    return HG_OK;
}

/* Copia os pixels para o anel (linhas alinhadas a 256) e grava a cópia + barreira. */
static hg_result record_upload(d3d12_device* d, const hg_image* img, ID3D12Resource* tex) {
    UINT w = img->width, h = img->height, src_row = w * 4;
    UINT row = (src_row + D3D12_TEXTURE_DATA_PITCH_ALIGNMENT - 1) & ~(D3D12_TEXTURE_DATA_PITCH_ALIGNMENT - 1);
    d3d12_alloc a;
    hg_result r = d3d12_upload_alloc(d->dev, d3d12_ring(d), (UINT64)row * h, D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT, &a);
    if (r != HG_OK) return r;
    for (UINT y = 0; y < h; y++) memcpy(a.cpu + (size_t)y * row, img->pixels + (size_t)y * src_row, src_row);
    D3D12_TEXTURE_COPY_LOCATION dst = { tex, D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, { 0 } };
    D3D12_TEXTURE_COPY_LOCATION src = { a.resource, D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, { 0 } };
    src.PlacedFootprint.Offset = a.offset;
    src.PlacedFootprint.Footprint = (D3D12_SUBRESOURCE_FOOTPRINT){ DXGI_FORMAT_R8G8B8A8_UNORM, w, h, 1, row };
    ID3D12GraphicsCommandList_CopyTextureRegion(d->list, &dst, 0, 0, 0, &src, NULL);
    d3d12_barrier(d->list, tex, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
    return HG_OK;
}

/* Adia a soltura até a fence do último uso possível: a lista aberta (sinalizada em
 * fence_value+1) ou a última enviada. Sem memória para a fila: espera a GPU, se der.
 * Serve também para PSOs de shaders do usuário (srv 0). */
void d3d12_retire(d3d12_device* d, IUnknown* obj, UINT srv) {
    d3d12_images* im = &d->images;
    if (im->retired_count == im->retired_cap) {
        UINT cap = im->retired_cap ? im->retired_cap * 2 : 16;
        d3d12_retired_tex* n = realloc(im->retired, cap * sizeof *n);
        if (n) { im->retired = n; im->retired_cap = cap; }
    }
    if (im->retired_count < im->retired_cap) {
        UINT64 last_use = d->fence_value + (d->recording ? 1 : 0);
        im->retired[im->retired_count++] = (d3d12_retired_tex){ obj, srv, last_use };
    } else if (!d->recording) {
        d3d12_wait_idle(d);
        D3D12_RELEASE(obj);
        d3d12_srv_free(&d->srvs, srv);
    } /* senão: vaza o recurso — melhor que soltá-lo em uso */
}

hg_result d3d12_texture_create(d3d12_device* d, hg_image* img) {
    if (!d->recording) return hg__fail(HG_ERR_INVALID, "d3d12: textura criada fora de frame");
    d3d12_texture* t = calloc(1, sizeof *t);
    if (!t) return hg__fail(HG_ERR_OOM, "d3d12: sem memória para textura");
    hg_result r = d3d12_srv_alloc(&d->srvs, &t->srv);
    if (r == HG_OK) r = create_resource(d->dev, img, &t->tex);
    if (r == HG_OK) r = record_upload(d, img, t->tex);
    if (r != HG_OK) {
        if (t->tex) d3d12_retire(d, (IUnknown*)t->tex, t->srv);
        else if (t->srv) d3d12_srv_free(&d->srvs, t->srv);
        free(t);
        return r;
    }
    ID3D12Device_CreateShaderResourceView(d->dev, t->tex, NULL, d3d12_srv_cpu(&d->srvs, t->srv));
    img->gpu = t;
    d->images.texture_bytes += (uint64_t)img->width * img->height * 4;
    return HG_OK;
}

void d3d12_texture_destroy(d3d12_device* d, hg_image* img) {
    d3d12_texture* t = img->gpu;
    if (!t) return;
    d3d12_retire(d, (IUnknown*)t->tex, t->srv);
    d->images.texture_bytes -= (uint64_t)img->width * img->height * 4;
    free(t);
    img->gpu = NULL;
}

void d3d12_textures_collect(d3d12_device* d, int all) {
    d3d12_images* im = &d->images;
    UINT64 done = d->fence ? ID3D12Fence_GetCompletedValue(d->fence) : UINT64_MAX;
    for (UINT i = 0; i < im->retired_count;) {
        d3d12_retired_tex* r = &im->retired[i];
        if (!all && r->fence > done) { i++; continue; }
        D3D12_RELEASE(r->obj);
        d3d12_srv_free(&d->srvs, r->srv);
        *r = im->retired[--im->retired_count];
    }
}

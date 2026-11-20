/* d3d12_glyphs.c — atlas R8 (heap DEFAULT, criado sob demanda), cópia dos glyphs
 * via anel de upload e sink das instâncias coletadas por glyph_frame. */
#include "d3d12_internal.h"
#include "draw_list.h"
#include "error.h"
#include "glyph_frame.h"
#include <string.h>

static hg_result create_atlas(d3d12_device* d) {
    ID3D12Device* dev = d->dev;
    d3d12_glyphs* g = &d->glyphs;
    D3D12_HEAP_PROPERTIES heap = { D3D12_HEAP_TYPE_DEFAULT, 0, 0, 1, 1 };
    D3D12_RESOURCE_DESC rd = { 0 };
    rd.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
    rd.Width = rd.Height = HG_GLYPH_ATLAS_SIZE;
    rd.DepthOrArraySize = rd.MipLevels = 1;
    rd.Format = DXGI_FORMAT_R8_UNORM;
    rd.SampleDesc.Count = 1;
    HRESULT hr = ID3D12Device_CreateCommittedResource(dev, &heap, D3D12_HEAP_FLAG_NONE, &rd,
                                                      D3D12_RESOURCE_STATE_COPY_DEST, NULL, &IID_ID3D12Resource,
                                                      (void**)&g->atlas);
    if (FAILED(hr)) {
        d3d12_glyphs_release(g);
        return hg__fail(HG_ERR_OOM, "d3d12: textura do atlas falhou");
    }
    ID3D12Device_CreateShaderResourceView(dev, g->atlas, NULL, d3d12_srv_cpu(&d->srvs, 0));
    g->copying = 1;
    return HG_OK;
}

static hg_result upload_r8(void* user, hg_rect at, const uint8_t* r8, uint32_t pitch) {
    d3d12_device* d = user;
    d3d12_glyphs* g = &d->glyphs;
    UINT w = (UINT)at.w, h = (UINT)at.h;
    if (w == 0 || h == 0) return HG_OK;
    UINT row = (w + D3D12_TEXTURE_DATA_PITCH_ALIGNMENT - 1) & ~(D3D12_TEXTURE_DATA_PITCH_ALIGNMENT - 1);
    d3d12_alloc a;
    hg_result r = d3d12_upload_alloc(d->dev, d3d12_ring(d), (UINT64)row * h, D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT, &a);
    if (r != HG_OK) return r;
    for (UINT y = 0; y < h; y++) memcpy(a.cpu + (size_t)y * row, r8 + (size_t)y * pitch, w);
    if (!g->copying) {
        d3d12_barrier(d->list, g->atlas, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_COPY_DEST);
        g->copying = 1;
    }
    D3D12_TEXTURE_COPY_LOCATION dst = { g->atlas, D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, { 0 } };
    D3D12_TEXTURE_COPY_LOCATION src = { a.resource, D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, { 0 } };
    src.PlacedFootprint.Offset = a.offset;
    src.PlacedFootprint.Footprint = (D3D12_SUBRESOURCE_FOOTPRINT){ DXGI_FORMAT_R8_UNORM, w, h, 1, row };
    ID3D12GraphicsCommandList_CopyTextureRegion(d->list, &dst, (UINT)at.x, (UINT)at.y, 0, &src, NULL);
    return HG_OK;
}

hg_result d3d12_glyphs_prepare(d3d12_device* d, hg_context* ctx, const hg_draw_list* dl) {
    d3d12_glyphs* g = &d->glyphs;
    g->frame.count = 0;
    if (!hg__draw_list_has_text(dl)) return HG_OK;
    hg_result r = g->atlas ? HG_OK : create_atlas(d);
    if (r == HG_OK) r = hg__glyph_frame_collect(&g->frame, ctx, dl, upload_r8, d);
    if (g->copying) {
        d3d12_barrier(d->list, g->atlas, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
        g->copying = 0;
    }
    if (r != HG_OK || g->frame.count == 0) return r;

    UINT bytes = g->frame.count * (UINT)sizeof(hg_glyph_instance);
    d3d12_alloc a;
    r = d3d12_upload_alloc(d->dev, d3d12_ring(d), bytes, 16, &a);
    if (r != HG_OK) return r;
    memcpy(a.cpu, g->frame.instances, bytes);
    g->instances = a.gpu;
    return HG_OK;
}

void d3d12_glyphs_bind(d3d12_device* d) {
    d3d12_glyphs* g = &d->glyphs;
    ID3D12GraphicsCommandList* l = d->list;
    D3D12_VERTEX_BUFFER_VIEW vb = { g->instances, g->frame.count * (UINT)sizeof(hg_glyph_instance),
                                    sizeof(hg_glyph_instance) };
    ID3D12GraphicsCommandList_SetPipelineState(l, d->pipeline.glyph);
    ID3D12GraphicsCommandList_SetGraphicsRootDescriptorTable(l, 1, d3d12_srv_gpu(&d->srvs, 0));
    ID3D12GraphicsCommandList_IASetVertexBuffers(l, 0, 1, &vb);
}

void d3d12_glyphs_release(d3d12_glyphs* g) {
    D3D12_RELEASE(g->atlas);
    g->copying = 0;
    hg__glyph_frame_free(&g->frame);
}

uint64_t d3d12_gpu_bytes(const d3d12_device* d) {
    uint64_t bytes = 0;
    for (int i = 0; i < D3D12_FRAMES; i++) bytes += d->uploads[i].capacity;
    if (d->glyphs.atlas) bytes += (uint64_t)HG_GLYPH_ATLAS_SIZE * HG_GLYPH_ATLAS_SIZE; /* R8 */
    bytes += d->images.texture_bytes;
    return bytes;
}

/* d3d12_readback.c — leitura de pixel do backbuffer (usada por testes). Envia o que foi
 * gravado até aqui, espera a GPU e reabre a lista para o frame continuar. */
#include "d3d12_internal.h"
#include "error.h"

static ID3D12Resource* create_readback(ID3D12Device* dev) {
    D3D12_HEAP_PROPERTIES heap = { D3D12_HEAP_TYPE_READBACK, 0, 0, 1, 1 };
    D3D12_RESOURCE_DESC rd = { 0 };
    rd.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
    rd.Width = D3D12_TEXTURE_DATA_PITCH_ALIGNMENT;
    rd.Height = rd.DepthOrArraySize = rd.MipLevels = 1;
    rd.SampleDesc.Count = 1;
    rd.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
    ID3D12Resource* r = NULL;
    ID3D12Device_CreateCommittedResource(dev, &heap, D3D12_HEAP_FLAG_NONE, &rd, D3D12_RESOURCE_STATE_COPY_DEST, NULL,
                                         &IID_ID3D12Resource, (void**)&r);
    return r;
}

static void record_copy(d3d12_device* d, ID3D12Resource* back, ID3D12Resource* rb, UINT x, UINT y) {
    D3D12_TEXTURE_COPY_LOCATION src = { back, D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, { 0 } };
    D3D12_TEXTURE_COPY_LOCATION dst = { rb, D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, { 0 } };
    dst.PlacedFootprint.Footprint =
        (D3D12_SUBRESOURCE_FOOTPRINT){ DXGI_FORMAT_B8G8R8A8_UNORM, 1, 1, 1, D3D12_TEXTURE_DATA_PITCH_ALIGNMENT };
    D3D12_BOX box = { x, y, 0, x + 1, y + 1, 1 };
    d3d12_barrier(d->list, back, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE);
    ID3D12GraphicsCommandList_CopyTextureRegion(d->list, &dst, 0, 0, 0, &src, &box);
    d3d12_barrier(d->list, back, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET);
}

hg_result d3d12_surface_read_pixel(d3d12_device* d, d3d12_surface* s, UINT x, UINT y, uint32_t* out) {
    if (!d->recording) return hg__fail(HG_ERR_INVALID, "d3d12: read_pixel só dentro de um frame");
    ID3D12Resource* rb = create_readback(d->dev);
    if (!rb) return hg__fail(HG_ERR_OOM, "d3d12: buffer de readback falhou");
    record_copy(d, d3d12_surface_current(s), rb, x, y);
    hg_result r = d3d12_list_execute(d);
    if (r == HG_OK) d3d12_wait_idle(d);
    hg_result reopen = d3d12_list_open(d);
    if (reopen == HG_OK) d3d12_frame_bind_target(d, s);
    void* p = NULL;
    D3D12_RANGE range = { 0, 4 };
    if (r == HG_OK && SUCCEEDED(ID3D12Resource_Map(rb, 0, &range, &p))) {
        *out = *(const uint32_t*)p;
        D3D12_RANGE none = { 0, 0 };
        ID3D12Resource_Unmap(rb, 0, &none);
    } else if (r == HG_OK) {
        r = hg__fail(HG_ERR_DEVICE, "d3d12: readback falhou");
    }
    D3D12_RELEASE(rb);
    return r != HG_OK ? r : reopen;
}

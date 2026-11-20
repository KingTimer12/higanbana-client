/* d3d12_upload.c — anel de upload por frame: buffer UPLOAD mapeado de forma persistente,
 * cresce em potência de 2. O buffer antigo fica vivo até o slot ser reusado. */
#include "d3d12_internal.h"
#include "error.h"

static hg_result grow(ID3D12Device* dev, d3d12_upload* u, UINT64 need) {
    if (u->buffer && u->retired_count == D3D12_RETIRED_MAX)
        return hg__fail(HG_ERR_OOM, "d3d12: anel de upload cresceu demais num frame");
    UINT64 cap = u->capacity ? u->capacity * 2 : 256 * 1024;
    while (cap < need) cap *= 2;
    D3D12_HEAP_PROPERTIES heap = { D3D12_HEAP_TYPE_UPLOAD, 0, 0, 1, 1 };
    D3D12_RESOURCE_DESC rd = { 0 };
    rd.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
    rd.Width = cap;
    rd.Height = rd.DepthOrArraySize = rd.MipLevels = 1;
    rd.SampleDesc.Count = 1;
    rd.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
    ID3D12Resource* fresh = NULL;
    void* cpu = NULL;
    if (FAILED(ID3D12Device_CreateCommittedResource(dev, &heap, D3D12_HEAP_FLAG_NONE, &rd,
                                                    D3D12_RESOURCE_STATE_GENERIC_READ, NULL, &IID_ID3D12Resource,
                                                    (void**)&fresh)))
        return hg__fail(HG_ERR_OOM, "d3d12: buffer de upload falhou");
    D3D12_RANGE none = { 0, 0 };
    if (FAILED(ID3D12Resource_Map(fresh, 0, &none, &cpu))) {
        D3D12_RELEASE(fresh);
        return hg__fail(HG_ERR_DEVICE, "d3d12: Map do upload falhou");
    }
    if (u->buffer) u->retired[u->retired_count++] = u->buffer;
    u->buffer = fresh;
    u->cpu = cpu;
    u->capacity = cap;
    u->offset = 0;
    return HG_OK;
}

hg_result d3d12_upload_alloc(ID3D12Device* dev, d3d12_upload* u, UINT64 bytes, UINT64 align, d3d12_alloc* out) {
    UINT64 at = (u->offset + align - 1) & ~(align - 1);
    if (!u->buffer || at + bytes > u->capacity) {
        hg_result r = grow(dev, u, bytes);
        if (r != HG_OK) return r;
        at = 0;
    }
    u->offset = at + bytes;
    out->resource = u->buffer;
    out->offset = at;
    out->cpu = u->cpu + at;
    out->gpu = ID3D12Resource_GetGPUVirtualAddress(u->buffer) + at;
    return HG_OK;
}

void d3d12_upload_reset(d3d12_upload* u) {
    for (UINT i = 0; i < u->retired_count; i++) D3D12_RELEASE(u->retired[i]);
    u->retired_count = 0;
    u->offset = 0;
}

void d3d12_upload_release(d3d12_upload* u) {
    d3d12_upload_reset(u);
    D3D12_RELEASE(u->buffer); /* Release desmapeia */
    u->cpu = NULL;
    u->capacity = 0;
}

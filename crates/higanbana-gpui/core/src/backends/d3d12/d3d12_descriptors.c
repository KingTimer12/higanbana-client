/* d3d12_descriptors.c — heap SRV único visível ao shader, de tamanho fixo (D3D12_SRV_MAX).
 * Slot 0 é do atlas de glyphs; os demais saem de uma lista livre para as imagens. */
#include "d3d12_internal.h"
#include "error.h"

hg_result d3d12_srv_create(ID3D12Device* dev, d3d12_srv_heap* h) {
    D3D12_DESCRIPTOR_HEAP_DESC hd = { D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, D3D12_SRV_MAX,
                                      D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0 };
    if (FAILED(ID3D12Device_CreateDescriptorHeap(dev, &hd, &IID_ID3D12DescriptorHeap, (void**)&h->heap)))
        return hg__fail(HG_ERR_DEVICE, "d3d12: heap de descritores falhou");
    h->stride = ID3D12Device_GetDescriptorHandleIncrementSize(dev, D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
    h->free_count = 0;
    for (UINT i = D3D12_SRV_MAX - 1; i >= 1; i--) h->free_list[h->free_count++] = i; /* menores saem primeiro */
    return HG_OK;
}

void d3d12_srv_release(d3d12_srv_heap* h) {
    D3D12_RELEASE(h->heap);
    h->free_count = 0;
}

hg_result d3d12_srv_alloc(d3d12_srv_heap* h, UINT* index) {
    if (h->free_count == 0) return hg__fail(HG_ERR_OOM, "d3d12: limite de texturas (D3D12_SRV_MAX) atingido");
    *index = h->free_list[--h->free_count];
    return HG_OK;
}

void d3d12_srv_free(d3d12_srv_heap* h, UINT index) {
    if (index > 0 && h->free_count < D3D12_SRV_MAX) h->free_list[h->free_count++] = index;
}

D3D12_CPU_DESCRIPTOR_HANDLE d3d12_srv_cpu(const d3d12_srv_heap* h, UINT index) {
    D3D12_CPU_DESCRIPTOR_HANDLE c;
    ID3D12DescriptorHeap_GetCPUDescriptorHandleForHeapStart(h->heap, &c);
    c.ptr += (SIZE_T)index * h->stride;
    return c;
}

D3D12_GPU_DESCRIPTOR_HANDLE d3d12_srv_gpu(const d3d12_srv_heap* h, UINT index) {
    D3D12_GPU_DESCRIPTOR_HANDLE g;
    ID3D12DescriptorHeap_GetGPUDescriptorHandleForHeapStart(h->heap, &g);
    g.ptr += (UINT64)index * h->stride;
    return g;
}

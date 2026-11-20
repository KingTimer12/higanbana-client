/* d3d12_sync.c — fence, slots de frame em voo e abertura/envio da lista de comandos. */
#include "d3d12_internal.h"
#include "error.h"

static void wait_for(d3d12_device* d, UINT64 value) {
    if (ID3D12Fence_GetCompletedValue(d->fence) >= value) return;
    if (SUCCEEDED(ID3D12Fence_SetEventOnCompletion(d->fence, value, d->fence_event)))
        WaitForSingleObject(d->fence_event, INFINITE);
}

void d3d12_wait_idle(d3d12_device* d) {
    UINT64 v = ++d->fence_value;
    if (SUCCEEDED(ID3D12CommandQueue_Signal(d->queue, d->fence, v))) wait_for(d, v);
}

hg_result d3d12_list_open(d3d12_device* d) {
    wait_for(d, d->slot_values[d->slot]);
    d3d12_upload_reset(d3d12_ring(d));
    d3d12_textures_collect(d, 0);
    ID3D12CommandAllocator* a = d->allocators[d->slot];
    if (FAILED(ID3D12CommandAllocator_Reset(a)) || FAILED(ID3D12GraphicsCommandList_Reset(d->list, a, NULL)))
        return hg__fail(HG_ERR_DEVICE, "d3d12: reset da lista de comandos falhou");
    d->recording = 1;
    return HG_OK;
}

hg_result d3d12_list_execute(d3d12_device* d) {
    d->recording = 0;
    if (FAILED(ID3D12GraphicsCommandList_Close(d->list)))
        return hg__fail(HG_ERR_DEVICE, "d3d12: Close da lista falhou");
    ID3D12CommandList* lists[] = { (ID3D12CommandList*)d->list };
    ID3D12CommandQueue_ExecuteCommandLists(d->queue, 1, lists);
    d->slot_values[d->slot] = ++d->fence_value;
    if (FAILED(ID3D12CommandQueue_Signal(d->queue, d->fence, d->fence_value)))
        return hg__fail(HG_ERR_DEVICE, "d3d12: Signal falhou (device perdido?)");
    return HG_OK;
}

void d3d12_barrier(ID3D12GraphicsCommandList* l, ID3D12Resource* r, D3D12_RESOURCE_STATES from,
                   D3D12_RESOURCE_STATES to) {
    D3D12_RESOURCE_BARRIER b = { 0 };
    b.Type = D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;
    b.Transition.pResource = r;
    b.Transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
    b.Transition.StateBefore = from;
    b.Transition.StateAfter = to;
    ID3D12GraphicsCommandList_ResourceBarrier(l, 1, &b);
}

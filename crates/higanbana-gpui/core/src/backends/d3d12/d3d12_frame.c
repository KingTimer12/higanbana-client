/* d3d12_frame.c — começo de frame (barreira para RT, clear) e fim (barreira, envio, present). */
#include "d3d12_internal.h"
#include "error.h"

void d3d12_frame_bind_target(d3d12_device* d, d3d12_surface* s) {
    D3D12_CPU_DESCRIPTOR_HANDLE rtv = d3d12_surface_rtv(s);
    ID3D12GraphicsCommandList_OMSetRenderTargets(d->list, 1, &rtv, FALSE, NULL);
}

hg_result d3d12_frame_begin(d3d12_device* d, d3d12_surface* s, hg_color clear) {
    if (d->recording) return hg__fail(HG_ERR_INVALID, "d3d12: frame já aberto");
    hg_result r = d3d12_list_open(d);
    if (r != HG_OK) return r;
    d3d12_barrier(d->list, d3d12_surface_current(s), D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET);
    d3d12_frame_bind_target(d, s);
    const float c[4] = { clear.r, clear.g, clear.b, clear.a };
    ID3D12GraphicsCommandList_ClearRenderTargetView(d->list, d3d12_surface_rtv(s), c, 0, NULL);
    return HG_OK;
}

hg_result d3d12_frame_end(d3d12_device* d, d3d12_surface* s, int vsync, int* occluded) {
    if (!d->recording) return hg__fail(HG_ERR_INVALID, "d3d12: frame não aberto");
    d3d12_barrier(d->list, d3d12_surface_current(s), D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PRESENT);
    hg_result r = d3d12_list_execute(d);
    d->slot = (d->slot + 1) % D3D12_FRAMES;
    if (r != HG_OK) return r;
    HRESULT hr = IDXGISwapChain3_Present(s->swapchain, vsync ? 1 : 0, 0);
    *occluded = hr == DXGI_STATUS_OCCLUDED; /* volta na hora, sem esperar o vsync */
    if (*occluded) return HG_OK;
    return SUCCEEDED(hr) ? HG_OK : hg__fail(HG_ERR_DEVICE, "d3d12: Present falhou (device perdido?)");
}

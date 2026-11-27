/* d3d12_swapchain.c — swapchain flip-discard por janela e RTVs dos backbuffers. */
#include "d3d12_internal.h"
#include "error.h"
#include <string.h>

static void release_buffers(d3d12_surface* s) {
    for (int i = 0; i < D3D12_BUFFERS; i++) D3D12_RELEASE(s->buffers[i]);
}

static hg_result create_rtvs(d3d12_device* d, d3d12_surface* s) {
    D3D12_CPU_DESCRIPTOR_HANDLE h;
    ID3D12DescriptorHeap_GetCPUDescriptorHandleForHeapStart(s->rtv_heap, &h);
    for (UINT i = 0; i < D3D12_BUFFERS; i++, h.ptr += s->rtv_size) {
        if (FAILED(IDXGISwapChain3_GetBuffer(s->swapchain, i, &IID_ID3D12Resource, (void**)&s->buffers[i]))) {
            release_buffers(s);
            return hg__fail(HG_ERR_DEVICE, "d3d12: falha ao criar render target");
        }
        ID3D12Device_CreateRenderTargetView(d->dev, s->buffers[i], NULL, h);
    }
    return HG_OK;
}

hg_result d3d12_surface_create(d3d12_device* d, HWND hwnd, UINT w, UINT h, d3d12_surface* s) {
    memset(s, 0, sizeof *s);
    D3D12_DESCRIPTOR_HEAP_DESC hd = { D3D12_DESCRIPTOR_HEAP_TYPE_RTV, D3D12_BUFFERS, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0 };
    if (FAILED(ID3D12Device_CreateDescriptorHeap(d->dev, &hd, &IID_ID3D12DescriptorHeap, (void**)&s->rtv_heap)))
        return hg__fail(HG_ERR_DEVICE, "d3d12: heap de RTV falhou");
    s->rtv_size = ID3D12Device_GetDescriptorHandleIncrementSize(d->dev, D3D12_DESCRIPTOR_HEAP_TYPE_RTV);
    DXGI_SWAP_CHAIN_DESC1 desc = { 0 };
    desc.Width = w ? w : 1;
    desc.Height = h ? h : 1;
    desc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    desc.SampleDesc.Count = 1;
    desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    desc.BufferCount = D3D12_BUFFERS;
    desc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
    desc.AlphaMode = DXGI_ALPHA_MODE_IGNORE;
    IDXGISwapChain1* sc1 = NULL;
    HRESULT hr = IDXGIFactory4_CreateSwapChainForHwnd(d->factory, (IUnknown*)d->queue, hwnd, &desc, NULL, NULL, &sc1);
    if (SUCCEEDED(hr)) hr = IDXGISwapChain1_QueryInterface(sc1, &IID_IDXGISwapChain3, (void**)&s->swapchain);
    D3D12_RELEASE(sc1);
    if (FAILED(hr)) return hg__fail(HG_ERR_DEVICE, "d3d12: CreateSwapChainForHwnd falhou");
    IDXGIFactory4_MakeWindowAssociation(d->factory, hwnd, DXGI_MWA_NO_ALT_ENTER);
    s->width = desc.Width;
    s->height = desc.Height;
    return create_rtvs(d, s);
}

hg_result d3d12_surface_resize(d3d12_device* d, d3d12_surface* s, UINT w, UINT h) {
    if (w == 0 || h == 0 || (s->buffers[0] && w == s->width && h == s->height)) return HG_OK;
    d3d12_wait_idle(d);
    release_buffers(s);
    d3d12_backdrop_release(d, &s->backdrop); /* tamanho velho: recriada no próximo uso */
    if (FAILED(IDXGISwapChain3_ResizeBuffers(s->swapchain, 0, w, h, DXGI_FORMAT_UNKNOWN, 0)))
        return hg__fail(HG_ERR_DEVICE, "d3d12: ResizeBuffers falhou");
    hg_result r = create_rtvs(d, s);
    /* Só marca o tamanho como atual se os RTVs existem: falha é refeita no próximo frame. */
    if (r == HG_OK) { s->width = w; s->height = h; }
    return r;
}

ID3D12Resource* d3d12_surface_current(d3d12_surface* s) {
    return s->buffers[IDXGISwapChain3_GetCurrentBackBufferIndex(s->swapchain)];
}

D3D12_CPU_DESCRIPTOR_HANDLE d3d12_surface_rtv(d3d12_surface* s) {
    D3D12_CPU_DESCRIPTOR_HANDLE h;
    ID3D12DescriptorHeap_GetCPUDescriptorHandleForHeapStart(s->rtv_heap, &h);
    h.ptr += (SIZE_T)IDXGISwapChain3_GetCurrentBackBufferIndex(s->swapchain) * s->rtv_size;
    return h;
}

void d3d12_surface_release(d3d12_device* d, d3d12_surface* s) {
    if (s->swapchain) d3d12_wait_idle(d);
    release_buffers(s);
    d3d12_backdrop_release(d, &s->backdrop);
    D3D12_RELEASE(s->swapchain);
    D3D12_RELEASE(s->rtv_heap);
}

/* d3d11_swapchain.c — swapchain flip-discard por janela. */
#include "d3d11_internal.h"
#include "error.h"
#include <string.h>

static hg_result create_rtv(d3d11_device* d, d3d11_surface* s) {
    ID3D11Texture2D* back = NULL;
    HRESULT hr = IDXGISwapChain1_GetBuffer(s->swapchain, 0, &IID_ID3D11Texture2D, (void**)&back);
    if (SUCCEEDED(hr)) hr = ID3D11Device_CreateRenderTargetView(d->dev, (ID3D11Resource*)back, NULL, &s->rtv);
    D3D11_RELEASE(back);
    return SUCCEEDED(hr) ? HG_OK : hg__fail(HG_ERR_DEVICE, "d3d11: falha ao criar render target");
}

hg_result d3d11_surface_create(d3d11_device* d, HWND hwnd, UINT w, UINT h, d3d11_surface* s) {
    memset(s, 0, sizeof *s);
    DXGI_SWAP_CHAIN_DESC1 desc = { 0 };
    desc.Width = w ? w : 1;
    desc.Height = h ? h : 1;
    desc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    desc.SampleDesc.Count = 1;
    desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    desc.BufferCount = 2;
    desc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
    desc.AlphaMode = DXGI_ALPHA_MODE_IGNORE;
    HRESULT hr = IDXGIFactory2_CreateSwapChainForHwnd(d->factory, (IUnknown*)d->dev, hwnd, &desc, NULL, NULL, &s->swapchain);
    if (FAILED(hr)) return hg__fail(HG_ERR_DEVICE, "d3d11: CreateSwapChainForHwnd falhou");
    IDXGIFactory2_MakeWindowAssociation(d->factory, hwnd, DXGI_MWA_NO_ALT_ENTER);
    s->width = desc.Width;
    s->height = desc.Height;
    return create_rtv(d, s);
}

hg_result d3d11_surface_resize(d3d11_device* d, d3d11_surface* s, UINT w, UINT h) {
    if (w == 0 || h == 0 || (s->rtv && w == s->width && h == s->height)) return HG_OK;
    ID3D11DeviceContext_OMSetRenderTargets(d->imm, 0, NULL, NULL);
    D3D11_RELEASE(s->rtv);
    d3d11_backdrop_release(&s->backdrop); /* tamanho velho: recriada no próximo uso */
    if (FAILED(IDXGISwapChain1_ResizeBuffers(s->swapchain, 0, w, h, DXGI_FORMAT_UNKNOWN, 0)))
        return hg__fail(HG_ERR_DEVICE, "d3d11: ResizeBuffers falhou");
    hg_result r = create_rtv(d, s);
    /* Só marca o tamanho como atual se o RTV existe: falha é refeita no próximo frame. */
    if (r == HG_OK) { s->width = w; s->height = h; }
    return r;
}

hg_result d3d11_surface_present(d3d11_surface* s, int vsync, int* occluded) {
    HRESULT hr = IDXGISwapChain1_Present(s->swapchain, vsync ? 1 : 0, 0);
    *occluded = hr == DXGI_STATUS_OCCLUDED; /* volta na hora, sem esperar o vsync */
    if (*occluded) return HG_OK;
    return SUCCEEDED(hr) ? HG_OK : hg__fail(HG_ERR_DEVICE, "d3d11: Present falhou (device perdido?)");
}

void d3d11_surface_release(d3d11_surface* s) {
    d3d11_backdrop_release(&s->backdrop);
    D3D11_RELEASE(s->rtv);
    D3D11_RELEASE(s->swapchain);
}

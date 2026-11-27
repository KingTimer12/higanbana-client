/* d3d11_device.c — device (hardware, com WARP como reserva) e fábrica DXGI. */
#include "d3d11_internal.h"
#include "error.h"
#include <string.h>

static HRESULT create_with(d3d11_device* d, D3D_DRIVER_TYPE type) {
    static const D3D_FEATURE_LEVEL levels[] = { D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_0 };
    /* Sem SINGLETHREADED: o alvo de vídeo usa o device a partir da thread de render (interop GL). */
    UINT flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
    HRESULT hr = d->loader.create_device(NULL, type, NULL, flags, levels, 3, D3D11_SDK_VERSION, &d->dev, NULL, &d->imm);
    if (hr == E_INVALIDARG) /* runtime sem 11_1 */
        hr = d->loader.create_device(NULL, type, NULL, flags, levels + 1, 2, D3D11_SDK_VERSION, &d->dev, NULL, &d->imm);
    return hr;
}

static int fetch_factory(d3d11_device* d, hg_backend_info* info) {
    IDXGIDevice* dxgi = NULL;
    IDXGIAdapter* adapter = NULL;
    int ok = 0;
    if (SUCCEEDED(ID3D11Device_QueryInterface(d->dev, &IID_IDXGIDevice, (void**)&dxgi)) &&
        SUCCEEDED(IDXGIDevice_GetAdapter(dxgi, &adapter)) &&
        SUCCEEDED(IDXGIAdapter_GetParent(adapter, &IID_IDXGIFactory2, (void**)&d->factory))) {
        DXGI_ADAPTER_DESC desc;
        if (info && SUCCEEDED(IDXGIAdapter_GetDesc(adapter, &desc)))
            WideCharToMultiByte(CP_UTF8, 0, desc.Description, -1, info->adapter, sizeof info->adapter, NULL, NULL);
        ok = 1;
    }
    D3D11_RELEASE(adapter);
    D3D11_RELEASE(dxgi);
    return ok;
}

hg_result d3d11_device_create(d3d11_device* d, hg_backend_info* info) {
    memset(d, 0, sizeof *d);
    if (!d3d11_loader_open(&d->loader)) return hg__fail(HG_ERR_UNAVAILABLE, "d3d11: d3d11.dll ausente");
    if (FAILED(create_with(d, D3D_DRIVER_TYPE_HARDWARE)) && FAILED(create_with(d, D3D_DRIVER_TYPE_WARP))) {
        d3d11_device_release(d);
        return hg__fail(HG_ERR_DEVICE, "d3d11: D3D11CreateDevice falhou");
    }
    if (!fetch_factory(d, info)) {
        d3d11_device_release(d);
        return hg__fail(HG_ERR_DEVICE, "d3d11: sem IDXGIFactory2");
    }
    d->quads.bind = D3D11_BIND_VERTEX_BUFFER;
    hg_result r = d3d11_pipeline_create(d->dev, &d->pipeline);
    if (r != HG_OK) d3d11_device_release(d);
    return r;
}

void d3d11_device_release(d3d11_device* d) {
    d3d11_user_shaders_release(&d->user_shaders);
    d3d11_glyphs_release(&d->glyphs);
    d3d11_images_release(&d->images);
    d3d11_buffer_release(&d->quads);
    d3d11_pipeline_release(&d->pipeline);
    D3D11_RELEASE(d->factory);
    D3D11_RELEASE(d->imm);
    D3D11_RELEASE(d->dev);
    d3d11_loader_close(&d->loader);
}

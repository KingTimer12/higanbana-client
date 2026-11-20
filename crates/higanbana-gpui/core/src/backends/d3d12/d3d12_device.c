/* d3d12_device.c — adaptador (hardware, WARP como reserva), fila, fence e lista. */
#include "d3d12_internal.h"
#include "error.h"
#include <string.h>

void d3d12_describe_adapter(IDXGIAdapter1* a, hg_backend_info* info) {
    DXGI_ADAPTER_DESC1 desc;
    if (info && SUCCEEDED(IDXGIAdapter1_GetDesc1(a, &desc)))
        WideCharToMultiByte(CP_UTF8, 0, desc.Description, -1, info->adapter, sizeof info->adapter, NULL, NULL);
}

static int try_adapter(d3d12_device* d, IDXGIAdapter1* a, hg_backend_info* info) {
    if (FAILED(d->loader.create_device((IUnknown*)a, D3D_FEATURE_LEVEL_11_0, &IID_ID3D12Device, (void**)&d->dev)))
        return 0;
    d3d12_describe_adapter(a, info);
    return 1;
}

static int pick_device(d3d12_device* d, hg_backend_info* info) {
    IDXGIAdapter1* a = NULL;
    for (UINT i = 0; !d->dev && IDXGIFactory4_EnumAdapters1(d->factory, i, &a) != DXGI_ERROR_NOT_FOUND; i++) {
        DXGI_ADAPTER_DESC1 desc;
        if (SUCCEEDED(IDXGIAdapter1_GetDesc1(a, &desc)) && !(desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE))
            try_adapter(d, a, info);
        D3D12_RELEASE(a);
    }
    if (!d->dev && SUCCEEDED(IDXGIFactory4_EnumWarpAdapter(d->factory, &IID_IDXGIAdapter1, (void**)&a))) {
        try_adapter(d, a, info);
        D3D12_RELEASE(a);
    }
    return d->dev != NULL;
}

static HRESULT create_queue_and_sync(d3d12_device* d) {
    D3D12_COMMAND_QUEUE_DESC qd = { D3D12_COMMAND_LIST_TYPE_DIRECT, 0, D3D12_COMMAND_QUEUE_FLAG_NONE, 0 };
    HRESULT hr = ID3D12Device_CreateCommandQueue(d->dev, &qd, &IID_ID3D12CommandQueue, (void**)&d->queue);
    for (int i = 0; SUCCEEDED(hr) && i < D3D12_FRAMES; i++)
        hr = ID3D12Device_CreateCommandAllocator(d->dev, D3D12_COMMAND_LIST_TYPE_DIRECT, &IID_ID3D12CommandAllocator,
                                                 (void**)&d->allocators[i]);
    if (SUCCEEDED(hr))
        hr = ID3D12Device_CreateCommandList(d->dev, 0, D3D12_COMMAND_LIST_TYPE_DIRECT, d->allocators[0], NULL,
                                            &IID_ID3D12GraphicsCommandList, (void**)&d->list);
    if (SUCCEEDED(hr)) hr = ID3D12GraphicsCommandList_Close(d->list);
    if (SUCCEEDED(hr)) hr = ID3D12Device_CreateFence(d->dev, 0, D3D12_FENCE_FLAG_NONE, &IID_ID3D12Fence, (void**)&d->fence);
    if (SUCCEEDED(hr)) {
        d->fence_event = CreateEventW(NULL, FALSE, FALSE, NULL);
        if (!d->fence_event) hr = E_FAIL;
    }
    return hr;
}

hg_result d3d12_device_create(d3d12_device* d, hg_backend_info* info) {
    memset(d, 0, sizeof *d);
    if (!d3d12_loader_open(&d->loader)) return hg__fail(HG_ERR_UNAVAILABLE, "d3d12: d3d12.dll/dxgi.dll ausente");
    hg_result r = HG_OK;
    if (FAILED(d->loader.create_factory2(0, &IID_IDXGIFactory4, (void**)&d->factory)))
        r = hg__fail(HG_ERR_DEVICE, "d3d12: CreateDXGIFactory2 falhou");
    else if (!pick_device(d, info))
        r = hg__fail(HG_ERR_DEVICE, "d3d12: D3D12CreateDevice falhou");
    else if (FAILED(create_queue_and_sync(d)))
        r = hg__fail(HG_ERR_DEVICE, "d3d12: falha ao criar fila/fence");
    else
        r = d3d12_pipeline_create(d->dev, &d->loader, &d->pipeline);
    if (r != HG_OK) d3d12_device_release(d);
    return r;
}

void d3d12_device_release(d3d12_device* d) {
    if (d->queue && d->fence && d->fence_event) d3d12_wait_idle(d);
    d3d12_glyphs_release(&d->glyphs);
    for (int i = 0; i < D3D12_FRAMES; i++) {
        d3d12_upload_release(&d->uploads[i]);
        D3D12_RELEASE(d->allocators[i]);
    }
    d3d12_pipeline_release(&d->pipeline);
    D3D12_RELEASE(d->list);
    D3D12_RELEASE(d->fence);
    if (d->fence_event) CloseHandle(d->fence_event);
    d->fence_event = NULL;
    D3D12_RELEASE(d->queue);
    D3D12_RELEASE(d->dev);
    D3D12_RELEASE(d->factory);
    d3d12_loader_close(&d->loader);
}

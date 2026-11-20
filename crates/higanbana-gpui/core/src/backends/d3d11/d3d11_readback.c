/* d3d11_readback.c — leitura de pixel do backbuffer (usada por testes). */
#include "d3d11_internal.h"
#include "error.h"

hg_result d3d11_surface_read_pixel(d3d11_device* d, d3d11_surface* s, UINT x, UINT y, uint32_t* out) {
    ID3D11Texture2D* back = NULL;
    ID3D11Texture2D* staging = NULL;
    D3D11_TEXTURE2D_DESC desc;
    hg_result r = hg__fail(HG_ERR_DEVICE, "d3d11: readback falhou");
    if (FAILED(IDXGISwapChain1_GetBuffer(s->swapchain, 0, &IID_ID3D11Texture2D, (void**)&back))) return r;
    ID3D11Texture2D_GetDesc(back, &desc);
    desc.Usage = D3D11_USAGE_STAGING;
    desc.BindFlags = 0;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
    desc.MiscFlags = 0;
    D3D11_MAPPED_SUBRESOURCE map;
    if (SUCCEEDED(ID3D11Device_CreateTexture2D(d->dev, &desc, NULL, &staging))) {
        ID3D11DeviceContext_CopyResource(d->imm, (ID3D11Resource*)staging, (ID3D11Resource*)back);
        if (SUCCEEDED(ID3D11DeviceContext_Map(d->imm, (ID3D11Resource*)staging, 0, D3D11_MAP_READ, 0, &map))) {
            *out = *(const uint32_t*)((const uint8_t*)map.pData + (size_t)y * map.RowPitch + (size_t)x * 4);
            ID3D11DeviceContext_Unmap(d->imm, (ID3D11Resource*)staging, 0);
            r = HG_OK;
        }
    }
    D3D11_RELEASE(staging);
    D3D11_RELEASE(back);
    return r;
}

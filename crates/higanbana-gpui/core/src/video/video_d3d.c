/* video_d3d.c — lado D3D11 do alvo de vídeo: device da UI, texturas e embrulho na hg_image. */
#include "d3d11_internal.h"
#include <d3d11_4.h>
#include "video_target.h"
#include "backend.h"
#include "context.h"
#include "error.h"
#include "image.h"

extern const hg_backend_vtbl hg__d3d11_backend;

hg_result video_d3d_device(hg_context* ctx, void** dev) {
    if (ctx->backend != &hg__d3d11_backend || !ctx->backend_data)
        return hg__fail(HG_ERR_UNAVAILABLE, "alvo de vídeo: só no backend D3D11");
    d3d11_device* dd = ctx->backend_data;
    ID3D11Device* d = dd->dev;
    /* A interop usa o device a partir da thread de render: serializa o contexto imediato. */
    ID3D11Multithread* mt = NULL;
    if (SUCCEEDED(ID3D11DeviceContext_QueryInterface(dd->imm, &IID_ID3D11Multithread, (void**)&mt))) {
        ID3D11Multithread_SetMultithreadProtected(mt, TRUE);
        ID3D11Multithread_Release(mt);
    } else {
        return hg__fail(HG_ERR_UNAVAILABLE, "alvo de vídeo: device sem ID3D11Multithread");
    }
    ID3D11Device_AddRef(d);
    *dev = d;
    return HG_OK;
}

hg_result video_d3d_texture(void* dev, uint32_t w, uint32_t h, void** tex, void** srv) {
    D3D11_TEXTURE2D_DESC td = { w, h, 1, 1, DXGI_FORMAT_R8G8B8A8_UNORM, { 1, 0 }, D3D11_USAGE_DEFAULT,
                                D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET, 0, 0 };
    ID3D11Texture2D* t = NULL;
    ID3D11ShaderResourceView* v = NULL;
    HRESULT hr = ID3D11Device_CreateTexture2D((ID3D11Device*)dev, &td, NULL, &t);
    if (SUCCEEDED(hr)) hr = ID3D11Device_CreateShaderResourceView((ID3D11Device*)dev, (ID3D11Resource*)t, NULL, &v);
    if (FAILED(hr)) {
        D3D11_RELEASE(t);
        return hg__fail(HG_ERR_OOM, "alvo de vídeo: textura D3D11 falhou");
    }
    *tex = t;
    *srv = v;
    return HG_OK;
}

void video_d3d_addref(void* unk) {
    if (unk) IUnknown_AddRef((IUnknown*)unk);
}

void video_d3d_release(void* unk) {
    if (unk) IUnknown_Release((IUnknown*)unk);
}

int video_d3d_same_device(hg_context* ctx, void* dev) {
    return ctx->backend == &hg__d3d11_backend && ctx->backend_data && ((d3d11_device*)ctx->backend_data)->dev == dev;
}

hg_result video_d3d_wrap(hg_image* img, void* tex, void* srv) {
    hg_context* ctx = img->ctx;
    if (ctx->backend != &hg__d3d11_backend || !ctx->backend_data)
        return hg__fail(HG_ERR_UNAVAILABLE, "alvo de vídeo: backend trocado");
    return d3d11_texture_wrap(ctx->backend_data, img, tex, srv);
}

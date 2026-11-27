/* d3d11_backdrop.c — textura por superfície com o que já foi desenhado atrás de um shader.
 * Só existe depois do 1º shader com HG_SHADER_BACKDROP; cada desenho copia apenas o
 * retângulo dele + margem, então dois vidros sobrepostos veem um ao outro. */
#include "d3d11_internal.h"
#include <math.h>

static int ensure_texture(d3d11_device* d, d3d11_surface* s) {
    d3d11_backdrop* b = &s->backdrop;
    if (b->tex && b->width == s->width && b->height == s->height) return 1;
    d3d11_backdrop_release(b);
    D3D11_TEXTURE2D_DESC desc = { 0 };
    desc.Width = s->width;
    desc.Height = s->height;
    desc.MipLevels = 1;
    desc.ArraySize = 1;
    desc.Format = DXGI_FORMAT_B8G8R8A8_UNORM; /* mesmo formato do swapchain */
    desc.SampleDesc.Count = 1;
    desc.Usage = D3D11_USAGE_DEFAULT;
    desc.BindFlags = D3D11_BIND_SHADER_RESOURCE;
    if (FAILED(ID3D11Device_CreateTexture2D(d->dev, &desc, NULL, &b->tex)) ||
        FAILED(ID3D11Device_CreateShaderResourceView(d->dev, (ID3D11Resource*)b->tex, NULL, &b->srv))) {
        d3d11_backdrop_release(b);
        return 0;
    }
    b->width = s->width;
    b->height = s->height;
    return 1;
}

static LONG clamp_px(float v, UINT max) {
    if (!(v > 0.0f)) return 0; /* também pega NaN */
    if (v > (float)max) return (LONG)max;
    return (LONG)v;
}

ID3D11ShaderResourceView* d3d11_backdrop_capture(d3d11_device* d, d3d11_surface* s, const hg_rect* r) {
    const float m = D3D11_BACKDROP_MARGIN;
    LONG left = clamp_px(floorf(r->x - m), s->width);
    LONG top = clamp_px(floorf(r->y - m), s->height);
    LONG right = clamp_px(ceilf(r->x + r->w + m), s->width);
    LONG bottom = clamp_px(ceilf(r->y + r->h + m), s->height);
    if (left >= right || top >= bottom || !s->rtv) return NULL; /* fora da superfície: nada a copiar */
    if (!ensure_texture(d, s)) return NULL;

    ID3D11Resource* back = NULL;
    ID3D11RenderTargetView_GetResource(s->rtv, &back);
    if (!back) return NULL;
    D3D11_BOX box = { (UINT)left, (UINT)top, 0, (UINT)right, (UINT)bottom, 1 };
    ID3D11DeviceContext_CopySubresourceRegion(d->imm, (ID3D11Resource*)s->backdrop.tex, 0, (UINT)left, (UINT)top, 0,
                                              back, 0, &box);
    D3D11_RELEASE(back);
    /* A cópia não mexe no OM, mas o lote seguinte conta com o RTV ligado: religa por segurança. */
    ID3D11DeviceContext_OMSetRenderTargets(d->imm, 1, &s->rtv, NULL);
    return s->backdrop.srv;
}

uint64_t d3d11_backdrop_bytes(const d3d11_backdrop* b) {
    return b->tex ? (uint64_t)b->width * b->height * 4 : 0; /* B8G8R8A8 */
}

void d3d11_backdrop_release(d3d11_backdrop* b) {
    D3D11_RELEASE(b->srv);
    D3D11_RELEASE(b->tex);
    b->width = b->height = 0;
}

/* d3d11_pipeline.c — shaders pré-compilados (fxc no build) e estados fixos. */
#include "d3d11_internal.h"
#include "error.h"
#include <stddef.h>
#include "quad_vs.h" /* gerado em OUT_DIR: hg_quad_vs[] */
#include "quad_ps.h" /* gerado em OUT_DIR: hg_quad_ps[] */

#define INSTANCE(name, fmt, field) \
    { name, 0, fmt, 0, (UINT)offsetof(hg_quad, field), D3D11_INPUT_PER_INSTANCE_DATA, 1 }

static const D3D11_INPUT_ELEMENT_DESC k_quad_layout[] = {
    INSTANCE("RECT", DXGI_FORMAT_R32G32B32A32_FLOAT, rect),
    INSTANCE("FILL", DXGI_FORMAT_R32G32B32A32_FLOAT, fill),
    INSTANCE("BORDER", DXGI_FORMAT_R32G32B32A32_FLOAT, border),
    INSTANCE("RADIUS", DXGI_FORMAT_R32G32B32A32_FLOAT, radius),
    INSTANCE("BWIDTH", DXGI_FORMAT_R32_FLOAT, border_width),
    INSTANCE("BLUR", DXGI_FORMAT_R32_FLOAT, shadow_blur),
    INSTANCE("SHADOW", DXGI_FORMAT_R32G32B32A32_FLOAT, shadow),
};

static HRESULT create_states(ID3D11Device* dev, d3d11_pipeline* p) {
    D3D11_BLEND_DESC blend = { 0 };
    D3D11_RENDER_TARGET_BLEND_DESC* rt = &blend.RenderTarget[0];
    rt->BlendEnable = TRUE; /* alfa pré-multiplicado */
    rt->SrcBlend = D3D11_BLEND_ONE;
    rt->DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
    rt->BlendOp = D3D11_BLEND_OP_ADD;
    rt->SrcBlendAlpha = D3D11_BLEND_ONE;
    rt->DestBlendAlpha = D3D11_BLEND_INV_SRC_ALPHA;
    rt->BlendOpAlpha = D3D11_BLEND_OP_ADD;
    rt->RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL;
    HRESULT hr = ID3D11Device_CreateBlendState(dev, &blend, &p->blend);
    if (FAILED(hr)) return hr;

    D3D11_RASTERIZER_DESC raster = { 0 };
    raster.FillMode = D3D11_FILL_SOLID;
    raster.CullMode = D3D11_CULL_NONE;
    raster.ScissorEnable = TRUE;
    raster.DepthClipEnable = TRUE;
    hr = ID3D11Device_CreateRasterizerState(dev, &raster, &p->raster);
    if (FAILED(hr)) return hr;

    D3D11_BUFFER_DESC cb = { 16, D3D11_USAGE_DYNAMIC, D3D11_BIND_CONSTANT_BUFFER, D3D11_CPU_ACCESS_WRITE, 0, 0 };
    return ID3D11Device_CreateBuffer(dev, &cb, NULL, &p->globals);
}

hg_result d3d11_pipeline_create(ID3D11Device* dev, d3d11_pipeline* p) {
    HRESULT hr = ID3D11Device_CreateVertexShader(dev, hg_quad_vs, sizeof hg_quad_vs, NULL, &p->quad_vs);
    if (SUCCEEDED(hr)) hr = ID3D11Device_CreatePixelShader(dev, hg_quad_ps, sizeof hg_quad_ps, NULL, &p->quad_ps);
    if (SUCCEEDED(hr))
        hr = ID3D11Device_CreateInputLayout(dev, k_quad_layout, ARRAYSIZE(k_quad_layout), hg_quad_vs,
                                            sizeof hg_quad_vs, &p->quad_layout);
    if (SUCCEEDED(hr)) hr = create_states(dev, p);
    if (FAILED(hr)) {
        d3d11_pipeline_release(p);
        return hg__fail(HG_ERR_DEVICE, "d3d11: falha ao criar pipeline");
    }
    return HG_OK;
}

void d3d11_pipeline_release(d3d11_pipeline* p) {
    D3D11_RELEASE(p->globals);
    D3D11_RELEASE(p->raster);
    D3D11_RELEASE(p->blend);
    D3D11_RELEASE(p->quad_layout);
    D3D11_RELEASE(p->quad_ps);
    D3D11_RELEASE(p->quad_vs);
}

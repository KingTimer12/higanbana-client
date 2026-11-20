/* d3d12_pipeline.c — root signature (constantes b0, atlas t0, sampler estático s0)
 * e PSOs de quad e glyph com os mesmos shaders fxc do D3D11. */
#include "d3d12_internal.h"
#include "error.h"
#include "text.h"
#include <stddef.h>
/* Os headers do fxc definem arrays globais; renomeia para não colidir com o D3D11. */
#define hg_quad_vs hg_d3d12_quad_vs
#define hg_quad_ps hg_d3d12_quad_ps
#define hg_glyph_vs hg_d3d12_glyph_vs
#define hg_glyph_ps hg_d3d12_glyph_ps
#include "quad_vs.h"
#include "quad_ps.h"
#include "glyph_vs.h"
#include "glyph_ps.h"

#define F4 DXGI_FORMAT_R32G32B32A32_FLOAT
#define INST(name, fmt, type, field) \
    { name, 0, fmt, 0, (UINT)offsetof(type, field), D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA, 1 }

static const D3D12_INPUT_ELEMENT_DESC k_quad_layout[] = {
    INST("RECT", F4, hg_quad, rect),     INST("FILL", F4, hg_quad, fill),
    INST("BORDER", F4, hg_quad, border), INST("RADIUS", F4, hg_quad, radius),
    INST("BWIDTH", DXGI_FORMAT_R32_FLOAT, hg_quad, border_width),
    INST("BLUR", DXGI_FORMAT_R32_FLOAT, hg_quad, shadow_blur),
    INST("SHADOW", F4, hg_quad, shadow),
};

static const D3D12_INPUT_ELEMENT_DESC k_glyph_layout[] = {
    INST("RECT", F4, hg_glyph_instance, rect),
    INST("UV", F4, hg_glyph_instance, uv),
    INST("COLOR", F4, hg_glyph_instance, color),
};

static HRESULT create_root(ID3D12Device* dev, const d3d12_loader* l, d3d12_pipeline* p) {
    D3D12_DESCRIPTOR_RANGE range = { D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 1, 0, 0, 0 };
    D3D12_ROOT_PARAMETER params[2] = { 0 };
    params[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
    params[0].Constants.Num32BitValues = 4; /* viewport w,h + atlas w,h */
    params[0].ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
    params[1].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    params[1].DescriptorTable.NumDescriptorRanges = 1;
    params[1].DescriptorTable.pDescriptorRanges = &range;
    params[1].ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
    D3D12_STATIC_SAMPLER_DESC sampler = { 0 };
    sampler.Filter = D3D12_FILTER_MIN_MAG_MIP_POINT;
    sampler.AddressU = sampler.AddressV = sampler.AddressW = D3D12_TEXTURE_ADDRESS_MODE_CLAMP;
    sampler.MaxLOD = D3D12_FLOAT32_MAX;
    sampler.ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
    D3D12_ROOT_SIGNATURE_DESC desc = { 2, params, 1, &sampler,
                                       D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT };
    ID3DBlob* blob = NULL;
    HRESULT hr = l->serialize_root_signature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &blob, NULL);
    if (SUCCEEDED(hr))
        hr = ID3D12Device_CreateRootSignature(dev, 0, ID3D10Blob_GetBufferPointer(blob), ID3D10Blob_GetBufferSize(blob),
                                              &IID_ID3D12RootSignature, (void**)&p->root);
    D3D12_RELEASE(blob);
    return hr;
}

static HRESULT create_pso(ID3D12Device* dev, ID3D12RootSignature* root, D3D12_SHADER_BYTECODE vs,
                          D3D12_SHADER_BYTECODE ps, const D3D12_INPUT_ELEMENT_DESC* layout, UINT n,
                          ID3D12PipelineState** out) {
    D3D12_GRAPHICS_PIPELINE_STATE_DESC d = { 0 };
    d.pRootSignature = root;
    d.VS = vs;
    d.PS = ps;
    D3D12_RENDER_TARGET_BLEND_DESC* rt = &d.BlendState.RenderTarget[0];
    rt->BlendEnable = TRUE; /* alfa pré-multiplicado */
    rt->SrcBlend = rt->SrcBlendAlpha = D3D12_BLEND_ONE;
    rt->DestBlend = rt->DestBlendAlpha = D3D12_BLEND_INV_SRC_ALPHA;
    rt->BlendOp = rt->BlendOpAlpha = D3D12_BLEND_OP_ADD;
    rt->LogicOp = D3D12_LOGIC_OP_NOOP;
    rt->RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL;
    d.SampleMask = UINT_MAX;
    d.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
    d.RasterizerState.CullMode = D3D12_CULL_MODE_NONE;
    d.RasterizerState.DepthClipEnable = TRUE; /* scissor é sempre ativo no D3D12 */
    d.InputLayout.pInputElementDescs = layout;
    d.InputLayout.NumElements = n;
    d.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
    d.NumRenderTargets = 1;
    d.RTVFormats[0] = DXGI_FORMAT_B8G8R8A8_UNORM;
    d.SampleDesc.Count = 1;
    return ID3D12Device_CreateGraphicsPipelineState(dev, &d, &IID_ID3D12PipelineState, (void**)out);
}

#define BYTECODE(a) ((D3D12_SHADER_BYTECODE){ a, sizeof a })

hg_result d3d12_pipeline_create(ID3D12Device* dev, const d3d12_loader* l, d3d12_pipeline* p) {
    HRESULT hr = create_root(dev, l, p);
    if (SUCCEEDED(hr))
        hr = create_pso(dev, p->root, BYTECODE(hg_quad_vs), BYTECODE(hg_quad_ps), k_quad_layout,
                        ARRAYSIZE(k_quad_layout), &p->quad);
    if (SUCCEEDED(hr))
        hr = create_pso(dev, p->root, BYTECODE(hg_glyph_vs), BYTECODE(hg_glyph_ps), k_glyph_layout,
                        ARRAYSIZE(k_glyph_layout), &p->glyph);
    if (FAILED(hr)) {
        d3d12_pipeline_release(p);
        return hg__fail(HG_ERR_DEVICE, "d3d12: falha ao criar pipeline");
    }
    return HG_OK;
}

void d3d12_pipeline_release(d3d12_pipeline* p) {
    D3D12_RELEASE(p->glyph);
    D3D12_RELEASE(p->quad);
    D3D12_RELEASE(p->root);
}

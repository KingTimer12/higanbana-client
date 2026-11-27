/* d3d12_user_shader.c — shaders do usuário: o mesmo HLSL do naga do D3D11, compilado com
 * D3DCompile (vs_5_0/ps_5_0) no 1º uso, um PSO por shader, desenhado como um quad (strip de 4
 * vértices, sem vertex buffer). Root signature própria: CBV raiz b0 (uniform no anel de upload
 * do quadro), tabela SRV t1 (backdrop ou descritor nulo), sampler estático s2 linear/clamp. */
#include "d3d12_internal.h"
#include "draw_list.h"
#include "error.h"
#include "shader.h"
#include <stdlib.h>
#include <string.h>

static HRESULT create_root(d3d12_device* d, ID3D12RootSignature** out) {
    D3D12_DESCRIPTOR_RANGE range = { D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 1, 1, 0, 0 }; /* t1 */
    D3D12_ROOT_PARAMETER params[2] = { 0 };
    params[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_CBV; /* b0 */
    params[0].ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
    params[1].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    params[1].DescriptorTable.NumDescriptorRanges = 1;
    params[1].DescriptorTable.pDescriptorRanges = &range;
    params[1].ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
    D3D12_STATIC_SAMPLER_DESC sampler = { 0 };
    sampler.Filter = D3D12_FILTER_MIN_MAG_MIP_LINEAR;
    sampler.AddressU = sampler.AddressV = sampler.AddressW = D3D12_TEXTURE_ADDRESS_MODE_CLAMP;
    sampler.MaxLOD = D3D12_FLOAT32_MAX;
    sampler.ShaderRegister = 2; /* s2 */
    sampler.ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
    D3D12_ROOT_SIGNATURE_DESC desc = { 2, params, 1, &sampler, D3D12_ROOT_SIGNATURE_FLAG_NONE };
    ID3DBlob* blob = NULL;
    HRESULT hr = d->loader.serialize_root_signature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &blob, NULL);
    if (SUCCEEDED(hr))
        hr = ID3D12Device_CreateRootSignature(d->dev, 0, ID3D10Blob_GetBufferPointer(blob),
                                              ID3D10Blob_GetBufferSize(blob), &IID_ID3D12RootSignature, (void**)out);
    D3D12_RELEASE(blob);
    return hr;
}

/* Root signature e descritor nulo: criados uma vez, no 1º shader do contexto. */
static hg_result ensure_common(d3d12_device* d) {
    d3d12_user_shaders* u = &d->user_shaders;
    if (u->root) return HG_OK;
    UINT srv = 0;
    hg_result r = d3d12_srv_alloc(&d->srvs, &srv);
    if (r != HG_OK) return r;
    if (FAILED(create_root(d, &u->root))) {
        d3d12_srv_free(&d->srvs, srv);
        return hg__fail(HG_ERR_DEVICE, "d3d12: falha ao criar root signature dos shaders do usuário");
    }
    d3d12_srv_write_bgra(d->dev, &d->srvs, srv, NULL); /* t1 sem backdrop: lê 0 (transparente) */
    u->empty_srv = srv;
    return HG_OK;
}

/* Compila um estágio; em falha grava a mensagem do compilador no shader. */
static hg_result compile(d3d12_device* d, hg_shader* sh, const char* entry, const char* target, ID3DBlob** out) {
    ID3DBlob* errors = NULL;
    HRESULT hr = d->user_shaders.compiler.compile(sh->hlsl, strlen(sh->hlsl), "hg_shader", NULL, NULL, entry, target,
                                                  D3DCOMPILE_OPTIMIZATION_LEVEL3, 0, out, &errors);
    if (SUCCEEDED(hr)) { D3D12_RELEASE(errors); return HG_OK; }
    hg_result r;
    if (errors) {
        /* O blob nem sempre termina em zero: copia com o tamanho. */
        size_t n = ID3D10Blob_GetBufferSize(errors);
        char* msg = malloc(n + 1);
        if (msg) {
            memcpy(msg, ID3D10Blob_GetBufferPointer(errors), n);
            msg[n] = '\0';
        }
        r = hg__shader_set_error(sh, msg ? msg : "d3d12: D3DCompile falhou");
        free(msg);
    } else {
        r = hg__shader_set_error(sh, "d3d12: D3DCompile falhou sem mensagem");
    }
    D3D12_RELEASE(errors);
    D3D12_RELEASE(*out);
    return r;
}

static D3D12_SHADER_BYTECODE bytecode(ID3DBlob* b) {
    return (D3D12_SHADER_BYTECODE){ ID3D10Blob_GetBufferPointer(b), ID3D10Blob_GetBufferSize(b) };
}

hg_result d3d12_user_shader_prepare(d3d12_device* d, hg_shader* sh) {
    if (!sh->hlsl) return hg__shader_set_error(sh, "d3d12: shader sem fonte HLSL");
    if (!d3d12_compiler_open(&d->user_shaders.compiler))
        return hg__shader_set_error(sh, "d3d12: d3dcompiler_47.dll ausente");
    hg_result r = ensure_common(d);
    if (r != HG_OK) return r;

    /* D3D12 exige feature level 11_0 (pick_device): SM 5.0 sempre disponível. */
    ID3DBlob* vs = NULL;
    ID3DBlob* ps = NULL;
    r = compile(d, sh, "vs_main", "vs_5_0", &vs);
    if (r == HG_OK) r = compile(d, sh, "ps_main", "ps_5_0", &ps);
    ID3D12PipelineState* pso = NULL;
    if (r == HG_OK &&
        FAILED(d3d12_pso_create(d->dev, d->user_shaders.root, bytecode(vs), bytecode(ps), NULL, 0, &pso)))
        r = hg__shader_set_error(sh, "d3d12: CreateGraphicsPipelineState recusou o shader");
    D3D12_RELEASE(vs);
    D3D12_RELEASE(ps);
    if (r != HG_OK) return r;
    sh->gpu = pso;
    return HG_OK;
}

void d3d12_user_shader_release(d3d12_device* d, hg_shader* sh) {
    if (!sh->gpu) return;
    /* A lista aberta ou uma em voo pode usar o PSO: solta depois da fence. */
    d3d12_retire(d, (IUnknown*)sh->gpu, 0);
    sh->gpu = NULL;
}

int d3d12_user_shader_draw(d3d12_device* d, d3d12_surface* s, const hg_batch* b) {
    hg_shader* sh = b->shader;
    const hg_shader_draw* sd = &b->shader_draw;
    if (!(sd->rect.w > 0.0f) || !(sd->rect.h > 0.0f)) return 0;
    /* Falha de compilação: sh->error guardado, o quadro segue sem este lote. */
    if (hg__shader_gpu(sh) != HG_OK || !sh->gpu) return 0;
    d3d12_user_shaders* u = &d->user_shaders;

    UINT srv = u->empty_srv;
    if (sd->flags & HG_SHADER_BACKDROP) {
        UINT copy = d3d12_backdrop_capture(d, s, &sd->rect);
        if (copy) srv = copy;
    }
    d3d12_alloc a;
    if (d3d12_upload_alloc(d->dev, d3d12_ring(d), HG_SHADER_UNIFORM_BYTES,
                           D3D12_CONSTANT_BUFFER_DATA_PLACEMENT_ALIGNMENT, &a) != HG_OK)
        return 0;
    hg__shader_uniforms(sd, (float)s->width, (float)s->height, 1.0f, (float*)a.cpu);

    ID3D12GraphicsCommandList* l = d->list;
    ID3D12GraphicsCommandList_SetGraphicsRootSignature(l, u->root);
    ID3D12GraphicsCommandList_SetPipelineState(l, (ID3D12PipelineState*)sh->gpu);
    ID3D12GraphicsCommandList_SetGraphicsRootConstantBufferView(l, 0, a.gpu);
    ID3D12GraphicsCommandList_SetGraphicsRootDescriptorTable(l, 1, d3d12_srv_gpu(&d->srvs, srv));
    ID3D12GraphicsCommandList_DrawInstanced(l, 4, 1, 0, 0);
    return 1;
}

/* No device_release, com a GPU ociosa e os PSOs já soltos. */
void d3d12_user_shaders_release(d3d12_device* d) {
    d3d12_user_shaders* u = &d->user_shaders;
    D3D12_RELEASE(u->root);
    if (u->empty_srv) d3d12_srv_free(&d->srvs, u->empty_srv);
    u->empty_srv = 0;
    d3d12_compiler_close(&u->compiler);
}

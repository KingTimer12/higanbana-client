/* d3d11_user_shader.c — shaders do usuário: HLSL do naga compilado com D3DCompile no 1º uso,
 * desenhados como um quad (strip de 4 vértices, sem vertex buffer) com o uniform em b0,
 * backdrop em t1 e sampler em s2 (mapa fixo de translate.rs). */
#include "d3d11_internal.h"
#include "draw_list.h"
#include "error.h"
#include "shader.h"
#include <stdlib.h>
#include <string.h>

typedef struct d3d11_user_shader {
    ID3D11VertexShader* vs;
    ID3D11PixelShader*  ps;
} d3d11_user_shader;

/* cbuffer, sampler e textura vazia: criados uma vez, no 1º shader do contexto. */
static hg_result ensure_common(d3d11_device* d) {
    d3d11_user_shaders* u = &d->user_shaders;
    if (u->empty_srv) return HG_OK;
    D3D11_BUFFER_DESC cb = { HG_SHADER_UNIFORM_BYTES, D3D11_USAGE_DYNAMIC, D3D11_BIND_CONSTANT_BUFFER,
                             D3D11_CPU_ACCESS_WRITE, 0, 0 };
    D3D11_SAMPLER_DESC sd = { 0 };
    sd.Filter = D3D11_FILTER_MIN_MAG_MIP_LINEAR;
    sd.AddressU = sd.AddressV = sd.AddressW = D3D11_TEXTURE_ADDRESS_CLAMP;
    sd.MaxLOD = D3D11_FLOAT32_MAX;
    const uint32_t transparent = 0;
    D3D11_SUBRESOURCE_DATA init = { &transparent, 4, 4 };
    D3D11_TEXTURE2D_DESC td = { 0 };
    td.Width = td.Height = td.MipLevels = td.ArraySize = 1;
    td.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    td.SampleDesc.Count = 1;
    td.Usage = D3D11_USAGE_IMMUTABLE;
    td.BindFlags = D3D11_BIND_SHADER_RESOURCE;
    if (FAILED(ID3D11Device_CreateBuffer(d->dev, &cb, NULL, &u->uniforms)) ||
        FAILED(ID3D11Device_CreateSamplerState(d->dev, &sd, &u->sampler)) ||
        FAILED(ID3D11Device_CreateTexture2D(d->dev, &td, &init, &u->empty)) ||
        FAILED(ID3D11Device_CreateShaderResourceView(d->dev, (ID3D11Resource*)u->empty, NULL, &u->empty_srv))) {
        D3D11_RELEASE(u->empty_srv); /* compiler fica: é soltado em d3d11_user_shaders_release */
        D3D11_RELEASE(u->empty);
        D3D11_RELEASE(u->sampler);
        D3D11_RELEASE(u->uniforms);
        return hg__fail(HG_ERR_DEVICE, "d3d11: falha ao criar estado dos shaders do usuário");
    }
    return HG_OK;
}

/* Compila um estágio; em falha grava a mensagem do compilador no shader. */
static hg_result compile(d3d11_device* d, hg_shader* sh, const char* entry, const char* target, ID3DBlob** out) {
    ID3DBlob* errors = NULL;
    HRESULT hr = d->user_shaders.compiler.compile(sh->hlsl, strlen(sh->hlsl), "hg_shader", NULL, NULL, entry, target,
                                                  D3DCOMPILE_OPTIMIZATION_LEVEL3, 0, out, &errors);
    if (SUCCEEDED(hr)) { D3D11_RELEASE(errors); return HG_OK; }
    hg_result r;
    if (errors) {
        /* O blob nem sempre termina em zero: copia com o tamanho. */
        size_t n = ID3D10Blob_GetBufferSize(errors);
        char* msg = malloc(n + 1);
        if (msg) {
            memcpy(msg, ID3D10Blob_GetBufferPointer(errors), n);
            msg[n] = '\0';
        }
        r = hg__shader_set_error(sh, msg ? msg : "d3d11: D3DCompile falhou");
        free(msg);
    } else {
        r = hg__shader_set_error(sh, "d3d11: D3DCompile falhou sem mensagem");
    }
    D3D11_RELEASE(errors);
    D3D11_RELEASE(*out);
    return r;
}

hg_result d3d11_user_shader_prepare(d3d11_device* d, hg_shader* sh) {
    if (!sh->hlsl) return hg__shader_set_error(sh, "d3d11: shader sem fonte HLSL");
    if (!d3d11_compiler_open(&d->user_shaders.compiler))
        return hg__shader_set_error(sh, "d3d11: d3dcompiler_47.dll ausente");
    hg_result r = ensure_common(d);
    if (r != HG_OK) return r;

    int sm5 = ID3D11Device_GetFeatureLevel(d->dev) >= D3D_FEATURE_LEVEL_11_0;
    ID3DBlob* vs = NULL;
    ID3DBlob* ps = NULL;
    r = compile(d, sh, "vs_main", sm5 ? "vs_5_0" : "vs_4_0", &vs);
    if (r == HG_OK) r = compile(d, sh, "ps_main", sm5 ? "ps_5_0" : "ps_4_0", &ps);
    d3d11_user_shader* gpu = r == HG_OK ? calloc(1, sizeof *gpu) : NULL;
    if (r == HG_OK && !gpu) r = hg__fail(HG_ERR_OOM, "d3d11: sem memória");
    if (r == HG_OK) {
        HRESULT hr = ID3D11Device_CreateVertexShader(d->dev, ID3D10Blob_GetBufferPointer(vs), ID3D10Blob_GetBufferSize(vs),
                                                     NULL, &gpu->vs);
        if (SUCCEEDED(hr))
            hr = ID3D11Device_CreatePixelShader(d->dev, ID3D10Blob_GetBufferPointer(ps), ID3D10Blob_GetBufferSize(ps),
                                                NULL, &gpu->ps);
        if (FAILED(hr)) r = hg__shader_set_error(sh, "d3d11: CreateVertexShader/CreatePixelShader recusou o bytecode");
    }
    D3D11_RELEASE(vs);
    D3D11_RELEASE(ps);
    if (r != HG_OK) {
        if (gpu) { D3D11_RELEASE(gpu->ps); D3D11_RELEASE(gpu->vs); free(gpu); }
        return r;
    }
    sh->gpu = gpu;
    return HG_OK;
}

void d3d11_user_shader_release(hg_shader* sh) {
    d3d11_user_shader* gpu = sh->gpu;
    if (!gpu) return;
    D3D11_RELEASE(gpu->ps);
    D3D11_RELEASE(gpu->vs);
    free(gpu);
    sh->gpu = NULL;
}

static int upload_uniforms(d3d11_device* d, const d3d11_surface* s, const hg_shader_draw* sd) {
    ID3D11Buffer* cb = d->user_shaders.uniforms;
    D3D11_MAPPED_SUBRESOURCE map;
    if (FAILED(ID3D11DeviceContext_Map(d->imm, (ID3D11Resource*)cb, 0, D3D11_MAP_WRITE_DISCARD, 0, &map))) return 0;
    hg__shader_uniforms(sd, (float)s->width, (float)s->height, 1.0f, map.pData);
    ID3D11DeviceContext_Unmap(d->imm, (ID3D11Resource*)cb, 0);
    return 1;
}

int d3d11_user_shader_draw(d3d11_device* d, d3d11_surface* s, const hg_batch* b) {
    hg_shader* sh = b->shader;
    const hg_shader_draw* sd = &b->shader_draw;
    if (!(sd->rect.w > 0.0f) || !(sd->rect.h > 0.0f)) return 0;
    /* Falha de compilação: sh->error guardado, o quadro segue sem este lote. */
    if (hg__shader_gpu(sh) != HG_OK || !sh->gpu) return 0;
    const d3d11_user_shader* gpu = sh->gpu;
    d3d11_user_shaders* u = &d->user_shaders;

    ID3D11ShaderResourceView* backdrop = u->empty_srv;
    if (sd->flags & HG_SHADER_BACKDROP) {
        ID3D11ShaderResourceView* copy = d3d11_backdrop_capture(d, s, &sd->rect);
        if (copy) backdrop = copy;
    }
    if (!upload_uniforms(d, s, sd)) return 0;

    ID3D11DeviceContext* c = d->imm;
    ID3D11DeviceContext_IASetInputLayout(c, NULL);
    ID3D11DeviceContext_VSSetShader(c, gpu->vs, NULL, 0);
    ID3D11DeviceContext_PSSetShader(c, gpu->ps, NULL, 0);
    ID3D11DeviceContext_VSSetConstantBuffers(c, 0, 1, &u->uniforms);
    ID3D11DeviceContext_PSSetConstantBuffers(c, 0, 1, &u->uniforms);
    ID3D11DeviceContext_PSSetShaderResources(c, 1, 1, &backdrop);
    ID3D11DeviceContext_PSSetSamplers(c, 2, 1, &u->sampler);
    ID3D11DeviceContext_Draw(c, 4, 0);
    return 1;
}

void d3d11_user_shaders_release(d3d11_user_shaders* u) {
    D3D11_RELEASE(u->empty_srv);
    D3D11_RELEASE(u->empty);
    D3D11_RELEASE(u->sampler);
    D3D11_RELEASE(u->uniforms);
    d3d11_compiler_close(&u->compiler);
}

/* d3d12_loader.c — carrega d3d12.dll, dxgi.dll e (no 1º shader do usuário) d3dcompiler_47.dll dinamicamente. */
#include "d3d12_internal.h"

#define LOAD(lib, type, name) (type)(void*)GetProcAddress(lib, name)

int d3d12_loader_open(d3d12_loader* l) {
    l->d3d12 = LoadLibraryW(L"d3d12.dll");
    l->dxgi = LoadLibraryW(L"dxgi.dll");
    if (!l->d3d12 || !l->dxgi) { d3d12_loader_close(l); return 0; }
    l->create_device = LOAD(l->d3d12, hg_d3d12_create_device_fn, "D3D12CreateDevice");
    l->serialize_root_signature = LOAD(l->d3d12, hg_d3d12_serialize_rs_fn, "D3D12SerializeRootSignature");
    l->create_factory2 = LOAD(l->dxgi, hg_dxgi_create_factory2_fn, "CreateDXGIFactory2");
    if (!l->create_device || !l->serialize_root_signature || !l->create_factory2) {
        d3d12_loader_close(l);
        return 0;
    }
    return 1;
}

void d3d12_loader_close(d3d12_loader* l) {
    if (l->d3d12) FreeLibrary(l->d3d12);
    if (l->dxgi) FreeLibrary(l->dxgi);
    l->d3d12 = l->dxgi = NULL;
    l->create_device = NULL;
    l->serialize_root_signature = NULL;
    l->create_factory2 = NULL;
}

int d3d12_compiler_open(d3d12_compiler* c) {
    if (c->compile) return 1;
    c->lib = LoadLibraryW(L"d3dcompiler_47.dll");
    if (!c->lib) return 0;
    c->compile = LOAD(c->lib, pD3DCompile, "D3DCompile");
    if (!c->compile) { d3d12_compiler_close(c); return 0; }
    return 1;
}

void d3d12_compiler_close(d3d12_compiler* c) {
    if (c->lib) FreeLibrary(c->lib);
    c->lib = NULL;
    c->compile = NULL;
}

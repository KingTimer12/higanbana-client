/* d3d11_loader.c — carrega d3d11.dll e d3dcompiler_47.dll dinamicamente. */
#include "d3d11_internal.h"

int d3d11_loader_open(d3d11_loader* l) {
    l->lib = LoadLibraryW(L"d3d11.dll");
    if (!l->lib) return 0;
    l->create_device = (hg_d3d11_create_device_fn)(void*)GetProcAddress(l->lib, "D3D11CreateDevice");
    if (!l->create_device) { d3d11_loader_close(l); return 0; }
    return 1;
}

void d3d11_loader_close(d3d11_loader* l) {
    if (l->lib) FreeLibrary(l->lib);
    l->lib = NULL;
    l->create_device = NULL;
}

int d3d11_compiler_open(d3d11_compiler* c) {
    if (c->compile) return 1;
    c->lib = LoadLibraryW(L"d3dcompiler_47.dll");
    if (!c->lib) return 0;
    c->compile = (pD3DCompile)(void*)GetProcAddress(c->lib, "D3DCompile");
    if (!c->compile) { d3d11_compiler_close(c); return 0; }
    return 1;
}

void d3d11_compiler_close(d3d11_compiler* c) {
    if (c->lib) FreeLibrary(c->lib);
    c->lib = NULL;
    c->compile = NULL;
}

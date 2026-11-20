/* d3d11_loader.c — carrega d3d11.dll dinamicamente. */
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

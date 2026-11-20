/* d3d11_backend.c — vtable do backend D3D11 e probe. */
#include "backend.h"
#include "context.h"
#include <windows.h>

static int d3d11_probe(hg_backend_info* info) {
    HMODULE lib = LoadLibraryW(L"d3d11.dll");
    if (!lib) return 0;
    int ok = GetProcAddress(lib, "D3D11CreateDevice") != NULL;
    FreeLibrary(lib);
    (void)info;
    return ok;
}

static hg_result d3d11_create(hg_context* ctx) {
    (void)ctx;
    return HG_OK;
}

static void d3d11_destroy(hg_context* ctx) { (void)ctx; }

const hg_backend_vtbl hg__d3d11_backend = {
    .kind = HG_BACKEND_D3D11,
    .name = "Direct3D 11",
    .probe = d3d11_probe,
    .create = d3d11_create,
    .destroy = d3d11_destroy,
};

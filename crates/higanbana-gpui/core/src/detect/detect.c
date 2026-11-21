/* detect.c — checagens baratas de disponibilidade: não carregam driver de GPU.
 * Usadas pela enumeração; o probe completo (que carrega driver) fica para a criação. */
#define COBJMACROS
#include <windows.h>
#include <dxgi.h>
#include "hgui_internal.h"

int hg__detect_dll(const wchar_t* name) {
    HMODULE m = LoadLibraryExW(name, NULL, LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (!m) return 0;
    FreeLibrary(m);
    return 1;
}

typedef HRESULT(WINAPI* create_factory1_fn)(REFIID, void**);

/* Primeiro adaptador de hardware (pula o WARP/Basic Render) via DXGI. Só
 * enumera: o UMD da GPU só carrega quando um device é criado. */
int hg__detect_adapter(hg_backend_info* info) {
    HMODULE dxgi = LoadLibraryExW(L"dxgi.dll", NULL, LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (!dxgi) return 0;
    create_factory1_fn create = (create_factory1_fn)(void*)GetProcAddress(dxgi, "CreateDXGIFactory1");
    IDXGIFactory1* f = NULL;
    int found = 0;
    if (create && SUCCEEDED(create(&IID_IDXGIFactory1, (void**)&f))) {
        IDXGIAdapter1* a = NULL;
        for (UINT i = 0; !found && IDXGIFactory1_EnumAdapters1(f, i, &a) != DXGI_ERROR_NOT_FOUND; i++) {
            DXGI_ADAPTER_DESC1 desc;
            if (SUCCEEDED(IDXGIAdapter1_GetDesc1(a, &desc)) && !(desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE)) {
                found = 1;
                if (info) WideCharToMultiByte(CP_UTF8, 0, desc.Description, -1, info->adapter,
                                              sizeof info->adapter, NULL, NULL);
            }
            IDXGIAdapter1_Release(a);
        }
        IDXGIFactory1_Release(f);
    }
    FreeLibrary(dxgi);
    return found;
}

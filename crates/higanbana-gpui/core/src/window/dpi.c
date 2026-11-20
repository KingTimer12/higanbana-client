/* dpi.c — DPI awareness carregada dinamicamente (Windows 10 1703+). */
#include "window.h"

typedef BOOL(WINAPI* set_ctx_fn)(DPI_AWARENESS_CONTEXT);
typedef UINT(WINAPI* dpi_for_window_fn)(HWND);

static INIT_ONCE g_once = INIT_ONCE_STATIC_INIT;
static dpi_for_window_fn g_dpi_for_window;

static BOOL CALLBACK init_dpi(PINIT_ONCE once, PVOID p, PVOID* c) {
    (void)once; (void)p; (void)c;
    HMODULE user32 = GetModuleHandleW(L"user32.dll");
    set_ctx_fn set_ctx = (set_ctx_fn)(void*)GetProcAddress(user32, "SetProcessDpiAwarenessContext");
    if (set_ctx) set_ctx(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    g_dpi_for_window = (dpi_for_window_fn)(void*)GetProcAddress(user32, "GetDpiForWindow");
    return TRUE;
}

void hg__dpi_enable_awareness(void) { InitOnceExecuteOnce(&g_once, init_dpi, NULL, NULL); }

float hg__dpi_scale_for(HWND hwnd) {
    hg__dpi_enable_awareness();
    UINT dpi = g_dpi_for_window ? g_dpi_for_window(hwnd) : 96;
    return dpi ? (float)dpi / 96.0f : 1.0f;
}

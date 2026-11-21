/* window_win32.c — ciclo de vida da janela Win32 e bomba de mensagens. */
#include "window.h"
#include "error.h"
#include "hgui_internal.h"
#include <stdlib.h>

static const WCHAR k_class_name[] = L"HiganbanaGpuiWindow";
#define WM_HG_WAKE (WM_APP + 7)

static LRESULT CALLBACK wndproc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
    hg_window* win = (hg_window*)GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if (!win) return DefWindowProcW(hwnd, msg, wp, lp);
    LRESULT handled;
    if (hg__decor_wndproc(win, msg, wp, lp, &handled)) return handled;
    switch (msg) {
    case WM_CLOSE: {
        hg_event ev = { .kind = HG_EVENT_CLOSE };
        hg_queue_push(&win->events, &ev);
        return 0; /* app decide quando destruir */
    }
    case WM_SIZE:
        hg__window_apply_size(win, LOWORD(lp), HIWORD(lp));
        hg__decor_update_state(win);
        return 0;
    case WM_ACTIVATE: {
        win->active = LOWORD(wp) != WA_INACTIVE;
        LRESULT r = DefWindowProcW(hwnd, msg, wp, lp);
        hg__decor_update_state(win);
        return r;
    }
    case WM_DPICHANGED: {
        win->dpi_scale = (float)HIWORD(wp) / 96.0f;
        const RECT* r = (const RECT*)lp;
        SetWindowPos(hwnd, NULL, r->left, r->top, r->right - r->left, r->bottom - r->top,
                     SWP_NOZORDER | SWP_NOACTIVATE);
        return 0;
    }
    case WM_SETCURSOR:
        if (hg__window_on_setcursor(win, lp)) return TRUE;
        return DefWindowProcW(hwnd, msg, wp, lp);
    case WM_HG_WAKE: {
        hg_event ev = { .kind = HG_EVENT_WAKE };
        hg_queue_push(&win->events, &ev);
        return 0;
    }
    case WM_ERASEBKGND:
        return 1; /* evita flicker: backend pinta tudo */
    default:
        if (hg__window_translate(win, msg, wp, lp)) return 0;
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
}

static INIT_ONCE g_class_once = INIT_ONCE_STATIC_INIT;

static BOOL CALLBACK register_class(PINIT_ONCE once, PVOID p, PVOID* c) {
    (void)once; (void)p; (void)c;
    WNDCLASSEXW wc = { sizeof wc };
    wc.style = CS_HREDRAW | CS_VREDRAW;
    wc.lpfnWndProc = wndproc;
    wc.hInstance = GetModuleHandleW(NULL);
    wc.hCursor = LoadCursorW(NULL, (LPCWSTR)IDC_ARROW);
    wc.lpszClassName = k_class_name;
    return RegisterClassExW(&wc) != 0;
}

static WCHAR* utf8_to_wide(const char* s) {
    if (!s) s = "";
    int n = MultiByteToWideChar(CP_UTF8, 0, s, -1, NULL, 0);
    WCHAR* w = n > 0 ? malloc(sizeof(WCHAR) * n) : NULL;
    if (w) MultiByteToWideChar(CP_UTF8, 0, s, -1, w, n);
    return w;
}

static HWND create_hwnd(const hg_window_desc* desc) {
    DWORD style = WS_OVERLAPPEDWINDOW;
    if (!desc->resizable) style &= ~(WS_THICKFRAME | WS_MAXIMIZEBOX);
    float scale = hg__dpi_scale_for(NULL);
    RECT r = { 0, 0, (LONG)(desc->width * scale), (LONG)(desc->height * scale) };
    AdjustWindowRect(&r, style, FALSE);
    WCHAR* title = utf8_to_wide(desc->title_utf8);
    HWND hwnd = CreateWindowExW(0, k_class_name, title ? title : L"", style, CW_USEDEFAULT, CW_USEDEFAULT,
                                r.right - r.left, r.bottom - r.top, NULL, NULL, GetModuleHandleW(NULL), NULL);
    free(title);
    return hwnd;
}

hg_result hg_window_create(hg_context* ctx, const hg_window_desc* desc, hg_window** out) {
    if (!ctx || !desc || !out) return hg__fail(HG_ERR_INVALID, "hg_window_create: argumento nulo");
    *out = NULL;
    hg__dpi_enable_awareness();
    if (!InitOnceExecuteOnce(&g_class_once, register_class, NULL, NULL))
        return hg__fail(HG_ERR_DEVICE, "hg_window_create: RegisterClassExW falhou");

    hg_window* win = calloc(1, sizeof *win);
    if (!win) return hg__fail(HG_ERR_OOM, "hg_window_create: sem memória");
    win->ctx = ctx;
    win->hwnd = create_hwnd(desc);
    if (!win->hwnd) { free(win); return hg__fail(HG_ERR_DEVICE, "hg_window_create: CreateWindowExW falhou"); }
    win->dpi_scale = hg__dpi_scale_for(win->hwnd);

    RECT client;
    GetClientRect(win->hwnd, &client);
    win->width = (uint32_t)(client.right - client.left);
    win->height = (uint32_t)(client.bottom - client.top);

    ShowWindow(win->hwnd, SW_SHOW);
    hg_result r = ctx->backend->attach(ctx, win);
    if (r != HG_OK) { DestroyWindow(win->hwnd); free(win); return r; }
    /* USERDATA só depois do attach: WM_SIZE antes disso não toca no backend. */
    SetWindowLongPtrW(win->hwnd, GWLP_USERDATA, (LONG_PTR)win);
    win->active = GetForegroundWindow() == win->hwnd; /* WM_ACTIVATE do show veio antes */
    win->next = ctx->windows;
    ctx->windows = win;
    *out = win;
    return HG_OK;
}

/* Troca a HWND por uma nova igual (posição, estado, título, estilo). Usado antes de
 * prender o OpenGL numa troca de backend: uma HWND que já teve swapchain flip-model
 * (DXGI ou Vulkan) não volta a mostrar o SwapBuffers do GL — a imagem congelava. */
hg_result hg__window_recreate_hwnd(hg_window* win) {
    HWND old = win->hwnd;
    WINDOWPLACEMENT place = { sizeof place };
    GetWindowPlacement(old, &place);
    WCHAR title[256];
    GetWindowTextW(old, title, (int)(sizeof title / sizeof title[0]));
    DWORD style = (DWORD)GetWindowLongPtrW(old, GWL_STYLE) & ~WS_VISIBLE;
    DWORD ex = (DWORD)GetWindowLongPtrW(old, GWL_EXSTYLE);
    HWND fresh = CreateWindowExW(ex, k_class_name, title, style, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT,
                                 CW_USEDEFAULT, NULL, NULL, GetModuleHandleW(NULL), NULL);
    if (!fresh) return hg__fail(HG_ERR_DEVICE, "recreate_hwnd: CreateWindowExW falhou");
    int active = GetForegroundWindow() == old;
    SetWindowPlacement(fresh, &place); /* mostra no mesmo lugar e estado */
    if (active) SetForegroundWindow(fresh);
    /* USERDATA só agora: as mensagens da criação não tocam no backend (que está trocando). */
    SetWindowLongPtrW(fresh, GWLP_USERDATA, (LONG_PTR)win);
    SetWindowLongPtrW(old, GWLP_USERDATA, 0);
    win->hwnd = fresh;
    win->nc_tracking = win->nc_hover = 0;
    DestroyWindow(old);
    win->active = GetForegroundWindow() == fresh;
    hg__decor_refresh_frame(win);
    return HG_OK;
}

void hg_window_destroy(hg_window* win) {
    if (!win) return;
    SetWindowLongPtrW(win->hwnd, GWLP_USERDATA, 0);
    for (hg_window** p = &win->ctx->windows; *p; p = &(*p)->next)
        if (*p == win) { *p = win->next; break; }
    win->ctx->backend->detach(win->ctx, win);
    DestroyWindow(win->hwnd);
    hg__decor_free(win);
    free(win);
}

void hg__window_apply_size(hg_window* win, uint32_t width, uint32_t height) {
    win->width = width;
    win->height = height;
    if (width > 0 && height > 0 && win->surface) win->ctx->backend->resize(win, width, height);
    hg_event ev = { .kind = HG_EVENT_RESIZE };
    ev.u.resize.width = width;
    ev.u.resize.height = height;
    ev.u.resize.dpi_scale = win->dpi_scale;
    hg_queue_push(&win->events, &ev);
}

static void pump(MSG* msg) {
    TranslateMessage(msg);
    DispatchMessageW(msg);
}

int hg_window_poll_event(hg_window* win, hg_event* out) {
    if (!win || !out) return 0;
    MSG msg;
    while (win->events.count == 0 && PeekMessageW(&msg, NULL, 0, 0, PM_REMOVE)) pump(&msg);
    return hg_queue_pop(&win->events, out);
}

hg_result hg_window_wait_event(hg_window* win, hg_event* out) {
    if (!win || !out) return hg__fail(HG_ERR_INVALID, "hg_window_wait_event: argumento nulo");
    MSG msg;
    while (win->events.count == 0) {
        if (GetMessageW(&msg, NULL, 0, 0) <= 0) {
            out->kind = HG_EVENT_CLOSE;
            return HG_OK;
        }
        pump(&msg);
    }
    hg_queue_pop(&win->events, out);
    return HG_OK;
}

void hg_window_wake(hg_window* win) {
    if (win) PostMessageW(win->hwnd, WM_HG_WAKE, 0, 0);
}

float hg_window_dpi_scale(const hg_window* win) { return win ? win->dpi_scale : 1.0f; }

void hg__window_inject_resize(hg_window* win, uint32_t width, uint32_t height) {
    hg__window_apply_size(win, width, height);
}

void hg__window_request_close(hg_window* win) { PostMessageW(win->hwnd, WM_CLOSE, 0, 0); }

hg_result hg__window_read_pixel(hg_window* win, uint32_t x, uint32_t y, uint32_t* out_bgra) {
    if (!win || !out_bgra || x >= win->width || y >= win->height)
        return hg__fail(HG_ERR_INVALID, "read_pixel: fora da janela");
    return win->ctx->backend->read_pixel(win, x, y, out_bgra);
}

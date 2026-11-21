/* decor.c — janela sem barra nativa: área cliente total, hit test por regiões do app,
 * comandos (min/max/fechar/fullscreen) e flags de estado. */
#include "window.h"
#include "error.h"
#include <stdlib.h>
#include <string.h>
#include <windowsx.h>

#define RESIZE_BORDER 6.0f /* pixels lógicos */

static uint32_t compute_state(const hg_window* win) {
    uint32_t s = 0;
    if (IsZoomed(win->hwnd)) s |= HG_WINDOW_MAXIMIZED;
    if (IsIconic(win->hwnd)) s |= HG_WINDOW_MINIMIZED;
    if (win->fullscreen) s |= HG_WINDOW_FULLSCREEN;
    if (GetForegroundWindow() == win->hwnd) s |= HG_WINDOW_FOCUSED;
    return s;
}

void hg__decor_update_state(hg_window* win) {
    uint32_t s = compute_state(win);
    if (s == win->state) return;
    win->state = s;
    hg_event ev = { .kind = HG_EVENT_WINDOW_STATE };
    ev.u.state.flags = s;
    hg_queue_push(&win->events, &ev);
}

static void frame_changed(HWND hwnd) {
    SetWindowPos(hwnd, NULL, 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
}

void hg__decor_refresh_frame(hg_window* win) {
    if (win->custom) frame_changed(win->hwnd);
}

void hg__decor_free(hg_window* win) {
    free(win->regions);
    win->regions = NULL;
    win->region_count = win->region_cap = 0;
}

/* Maximizada, a janela passa da área de trabalho pela espessura da borda invisível. */
static void inset_maximized(hg_window* win, RECT* r) {
    UINT dpi = GetDpiForWindow(win->hwnd);
    int f = GetSystemMetricsForDpi(SM_CXFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
    int fy = GetSystemMetricsForDpi(SM_CYFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
    r->left += f;
    r->right -= f;
    r->top += fy;
    r->bottom -= fy;
}

static LRESULT border_hit(hg_window* win, float x, float y, float w, float h) {
    LONG_PTR style = GetWindowLongPtrW(win->hwnd, GWL_STYLE);
    if (!(style & WS_THICKFRAME) || win->fullscreen || IsZoomed(win->hwnd)) return HTNOWHERE;
    float b = RESIZE_BORDER;
    int left = x < b, right = x >= w - b, top = y < b, bottom = y >= h - b;
    if (top && left) return HTTOPLEFT;
    if (top && right) return HTTOPRIGHT;
    if (bottom && left) return HTBOTTOMLEFT;
    if (bottom && right) return HTBOTTOMRIGHT;
    if (top) return HTTOP;
    if (bottom) return HTBOTTOM;
    if (left) return HTLEFT;
    if (right) return HTRIGHT;
    return HTNOWHERE;
}

static LRESULT hit_test(hg_window* win, LPARAM lp) {
    POINT p = { GET_X_LPARAM(lp), GET_Y_LPARAM(lp) };
    ScreenToClient(win->hwnd, &p);
    RECT c;
    GetClientRect(win->hwnd, &c);
    float s = win->dpi_scale > 0 ? win->dpi_scale : 1.0f;
    float x = (float)p.x / s, y = (float)p.y / s;
    LRESULT border = border_hit(win, x, y, (float)c.right / s, (float)c.bottom / s);
    if (border != HTNOWHERE) return border;
    for (uint32_t i = win->region_count; i-- > 0;) {
        const hg_hit_region* r = &win->regions[i];
        if (x < r->x || y < r->y || x >= r->x + r->w || y >= r->y + r->h) continue;
        switch (r->kind) {
        case HG_HIT_CAPTION:  return HTCAPTION;
        case HG_HIT_MAXIMIZE: return HTMAXBUTTON;
        default:              return HTCLIENT;
        }
    }
    return HTCLIENT;
}

/* Eventos não-cliente sobre o botão maximizar viram mouse comum (hover/clique do app). */
static void push_nc_mouse(hg_window* win, hg_event_kind kind, LPARAM lp) {
    POINT p = { GET_X_LPARAM(lp), GET_Y_LPARAM(lp) };
    ScreenToClient(win->hwnd, &p);
    hg__window_push_mouse(win, kind, p.x, p.y, 0);
}

static void track_nc_leave(hg_window* win) {
    if (win->nc_tracking) return;
    TRACKMOUSEEVENT t = { sizeof t, TME_LEAVE | TME_NONCLIENT, win->hwnd, 0 };
    win->nc_tracking = TrackMouseEvent(&t) != 0;
}

int hg__decor_wndproc(hg_window* win, UINT msg, WPARAM wp, LPARAM lp, LRESULT* out) {
    if (!win->custom) return 0;
    switch (msg) {
    case WM_NCCALCSIZE:
        if (!wp) return 0;
        if (IsZoomed(win->hwnd) && !win->fullscreen) inset_maximized(win, &((NCCALCSIZE_PARAMS*)lp)->rgrc[0]);
        *out = 0;
        return 1;
    case WM_NCHITTEST:
        *out = hit_test(win, lp);
        return 1;
    case WM_NCMOUSEMOVE:
        if (wp != HTMAXBUTTON) return 0;
        track_nc_leave(win);
        push_nc_mouse(win, HG_EVENT_MOUSE_MOVE, lp);
        *out = 0;
        return 1;
    case WM_NCLBUTTONDOWN:
    case WM_NCLBUTTONDBLCLK:
        if (wp != HTMAXBUTTON) return 0;
        push_nc_mouse(win, HG_EVENT_MOUSE_DOWN, lp);
        *out = 0;
        return 1;
    case WM_NCLBUTTONUP:
        if (wp != HTMAXBUTTON) return 0;
        push_nc_mouse(win, HG_EVENT_MOUSE_UP, lp);
        *out = 0;
        return 1;
    case WM_NCMOUSELEAVE:
        win->nc_tracking = 0;
        hg__window_push_mouse(win, HG_EVENT_MOUSE_MOVE, -100000, -100000, 0);
        *out = 0;
        return 1;
    default:
        return 0;
    }
}

hg_result hg_window_set_decorations(hg_window* win, int custom) {
    if (!win) return hg__fail(HG_ERR_INVALID, "set_decorations: janela nula");
    win->custom = custom != 0;
    frame_changed(win->hwnd);
    return HG_OK;
}

hg_result hg_window_set_hit_regions(hg_window* win, const hg_hit_region* regions, uint32_t count) {
    if (!win || (count && !regions)) return hg__fail(HG_ERR_INVALID, "set_hit_regions: argumento nulo");
    if (count > win->region_cap) {
        hg_hit_region* grown = realloc(win->regions, sizeof *grown * count);
        if (!grown) return hg__fail(HG_ERR_OOM, "set_hit_regions: sem memória");
        win->regions = grown;
        win->region_cap = count;
    }
    if (count) memcpy(win->regions, regions, sizeof *regions * count);
    win->region_count = count;
    return HG_OK;
}

static void enter_fullscreen(hg_window* win) {
    if (win->fullscreen) return;
    win->saved_place.length = sizeof win->saved_place;
    GetWindowPlacement(win->hwnd, &win->saved_place);
    win->saved_style = GetWindowLongPtrW(win->hwnd, GWL_STYLE);
    MONITORINFO mi = { sizeof mi };
    GetMonitorInfoW(MonitorFromWindow(win->hwnd, MONITOR_DEFAULTTONEAREST), &mi);
    win->fullscreen = 1;
    SetWindowLongPtrW(win->hwnd, GWL_STYLE, (win->saved_style & ~WS_OVERLAPPEDWINDOW) | WS_POPUP);
    RECT m = mi.rcMonitor;
    SetWindowPos(win->hwnd, HWND_TOP, m.left, m.top, m.right - m.left, m.bottom - m.top, SWP_FRAMECHANGED | SWP_NOOWNERZORDER);
}

static void exit_fullscreen(hg_window* win) {
    if (!win->fullscreen) return;
    win->fullscreen = 0;
    SetWindowLongPtrW(win->hwnd, GWL_STYLE, win->saved_style);
    SetWindowPlacement(win->hwnd, &win->saved_place);
    frame_changed(win->hwnd);
}

hg_result hg_window_command(hg_window* win, hg_window_cmd cmd) {
    if (!win) return hg__fail(HG_ERR_INVALID, "window_command: janela nula");
    switch (cmd) {
    case HG_WINDOW_MINIMIZE: ShowWindow(win->hwnd, SW_MINIMIZE); break;
    case HG_WINDOW_MAXIMIZE: ShowWindow(win->hwnd, SW_MAXIMIZE); break;
    case HG_WINDOW_RESTORE:  ShowWindow(win->hwnd, SW_RESTORE); break;
    case HG_WINDOW_TOGGLE_MAXIMIZE: ShowWindow(win->hwnd, IsZoomed(win->hwnd) ? SW_RESTORE : SW_MAXIMIZE); break;
    case HG_WINDOW_CLOSE: SendMessageW(win->hwnd, WM_CLOSE, 0, 0); break;
    case HG_WINDOW_ENTER_FULLSCREEN: enter_fullscreen(win); break;
    case HG_WINDOW_EXIT_FULLSCREEN: exit_fullscreen(win); break;
    default: return hg__fail(HG_ERR_INVALID, "window_command: comando desconhecido");
    }
    hg__decor_update_state(win);
    return HG_OK;
}

uint32_t hg_window_state(const hg_window* win) {
    return win ? compute_state(win) : 0;
}

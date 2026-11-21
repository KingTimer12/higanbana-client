/* input.c — tradução de mensagens Win32 de entrada para hg_event. */
#include "window.h"
#include <windowsx.h>

static uint32_t current_mods(void) {
    uint32_t m = 0;
    if (GetKeyState(VK_SHIFT) & 0x8000) m |= HG_MOD_SHIFT;
    if (GetKeyState(VK_CONTROL) & 0x8000) m |= HG_MOD_CTRL;
    if (GetKeyState(VK_MENU) & 0x8000) m |= HG_MOD_ALT;
    return m;
}

static void push_mouse(hg_window* win, hg_event_kind kind, LPARAM lp, uint32_t button) {
    hg_event ev = { .kind = kind, .mods = current_mods() };
    ev.u.mouse.x = (float)GET_X_LPARAM(lp) / win->dpi_scale;
    ev.u.mouse.y = (float)GET_Y_LPARAM(lp) / win->dpi_scale;
    ev.u.mouse.button = button;
    hg_queue_push(&win->events, &ev);
}

void hg__window_push_mouse(hg_window* win, hg_event_kind kind, int x, int y, uint32_t button) {
    /* Sem passar por LPARAM: coordenadas fora de 16 bits (ponteiro "fora da janela"). */
    hg_event ev = { .kind = kind, .mods = current_mods() };
    ev.u.mouse.x = (float)x / win->dpi_scale;
    ev.u.mouse.y = (float)y / win->dpi_scale;
    ev.u.mouse.button = button;
    hg_queue_push(&win->events, &ev);
}

static void push_key(hg_window* win, hg_event_kind kind, WPARAM wp, LPARAM lp) {
    hg_event ev = { .kind = kind, .mods = current_mods() };
    ev.u.key.key = (uint32_t)wp;
    ev.u.key.repeat = (lp & (1 << 30)) != 0;
    hg_queue_push(&win->events, &ev);
}

/* WM_CHAR entrega UTF-16; junta pares surrogate num codepoint. */
static void push_char(hg_window* win, WPARAM wp) {
    static __declspec(thread) WCHAR high;
    WCHAR c = (WCHAR)wp;
    if (c >= 0xD800 && c <= 0xDBFF) { high = c; return; }
    uint32_t cp = c;
    if (c >= 0xDC00 && c <= 0xDFFF && high) cp = 0x10000 + (((uint32_t)high - 0xD800) << 10) + (c - 0xDC00);
    high = 0;
    if (cp < 0x20 && cp != '\t' && cp != '\r') return; /* controles viram KEY_DOWN */
    hg_event ev = { .kind = HG_EVENT_CHAR, .mods = current_mods() };
    ev.u.text.codepoint = cp;
    hg_queue_push(&win->events, &ev);
}

static void push_scroll(hg_window* win, float dx, float dy) {
    hg_event ev = { .kind = HG_EVENT_SCROLL, .mods = current_mods() };
    ev.u.scroll.dx = dx;
    ev.u.scroll.dy = dy;
    hg_queue_push(&win->events, &ev);
}

int hg__window_translate(hg_window* win, UINT msg, WPARAM wp, LPARAM lp) {
    switch (msg) {
    case WM_MOUSEMOVE:   push_mouse(win, HG_EVENT_MOUSE_MOVE, lp, 0); return 1;
    case WM_LBUTTONDOWN: SetCapture(win->hwnd); push_mouse(win, HG_EVENT_MOUSE_DOWN, lp, 0); return 1;
    case WM_LBUTTONUP:   ReleaseCapture(); push_mouse(win, HG_EVENT_MOUSE_UP, lp, 0); return 1;
    case WM_RBUTTONDOWN: push_mouse(win, HG_EVENT_MOUSE_DOWN, lp, 1); return 1;
    case WM_RBUTTONUP:   push_mouse(win, HG_EVENT_MOUSE_UP, lp, 1); return 1;
    case WM_MBUTTONDOWN: push_mouse(win, HG_EVENT_MOUSE_DOWN, lp, 2); return 1;
    case WM_MBUTTONUP:   push_mouse(win, HG_EVENT_MOUSE_UP, lp, 2); return 1;
    /* Laterais: XBUTTON1 = 3 (voltar), XBUTTON2 = 4 (avançar). */
    case WM_XBUTTONDOWN: push_mouse(win, HG_EVENT_MOUSE_DOWN, lp, GET_XBUTTON_WPARAM(wp) == XBUTTON1 ? 3 : 4); return 1;
    case WM_XBUTTONUP:   push_mouse(win, HG_EVENT_MOUSE_UP, lp, GET_XBUTTON_WPARAM(wp) == XBUTTON1 ? 3 : 4); return 1;
    case WM_MOUSEWHEEL:  push_scroll(win, 0, (float)GET_WHEEL_DELTA_WPARAM(wp) / WHEEL_DELTA); return 1;
    case WM_MOUSEHWHEEL: push_scroll(win, (float)GET_WHEEL_DELTA_WPARAM(wp) / WHEEL_DELTA, 0); return 1;
    case WM_KEYDOWN: case WM_SYSKEYDOWN: push_key(win, HG_EVENT_KEY_DOWN, wp, lp); return 0;
    case WM_KEYUP:   case WM_SYSKEYUP:   push_key(win, HG_EVENT_KEY_UP, wp, lp); return 0;
    case WM_CHAR:        push_char(win, wp); return 1;
    default:             return 0;
    }
}

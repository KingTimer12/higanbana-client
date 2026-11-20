/* cursor.c — cursor do mouse por janela, aplicado em WM_SETCURSOR. */
#include "window.h"
#include "error.h"

static LPCWSTR system_id(hg_cursor c) {
    switch (c) {
    case HG_CURSOR_HAND:        return (LPCWSTR)IDC_HAND;
    case HG_CURSOR_IBEAM:       return (LPCWSTR)IDC_IBEAM;
    case HG_CURSOR_CROSSHAIR:   return (LPCWSTR)IDC_CROSS;
    case HG_CURSOR_NOT_ALLOWED: return (LPCWSTR)IDC_NO;
    case HG_CURSOR_RESIZE_EW:   return (LPCWSTR)IDC_SIZEWE;
    case HG_CURSOR_RESIZE_NS:   return (LPCWSTR)IDC_SIZENS;
    case HG_CURSOR_MOVE:        return (LPCWSTR)IDC_SIZEALL;
    case HG_CURSOR_WAIT:        return (LPCWSTR)IDC_WAIT;
    default:                    return (LPCWSTR)IDC_ARROW;
    }
}

hg_result hg_window_set_cursor(hg_window* win, hg_cursor cursor) {
    if (!win || (unsigned)cursor > HG_CURSOR_WAIT) return hg__fail(HG_ERR_INVALID, "hg_window_set_cursor: argumento inválido");
    if (win->cursor == cursor) return HG_OK;
    win->cursor = cursor;
    /* Aplica já se o mouse está sobre a área cliente (WM_SETCURSOR só vem ao mover). */
    POINT p;
    RECT r;
    if (GetCursorPos(&p) && ScreenToClient(win->hwnd, &p) && GetClientRect(win->hwnd, &r) && PtInRect(&r, p))
        SetCursor(LoadCursorW(NULL, system_id(cursor)));
    return HG_OK;
}

hg_cursor hg_window_cursor(const hg_window* win) { return win ? win->cursor : HG_CURSOR_ARROW; }

int hg__window_on_setcursor(hg_window* win, LPARAM lp) {
    if (LOWORD(lp) != HTCLIENT) return 0;
    SetCursor(LoadCursorW(NULL, system_id(win->cursor)));
    return 1;
}

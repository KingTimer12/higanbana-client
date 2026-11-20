/* clipboard.c — texto UTF-8 na área de transferência do Windows (CF_UNICODETEXT). */
#include "hgui.h"
#include "error.h"
#include <windows.h>

/* Janela só de mensagens: SetClipboardData exige um dono não nulo. */
static HWND g_owner;

static HWND owner(void) {
    if (!g_owner) g_owner = CreateWindowExW(0, L"STATIC", L"", 0, 0, 0, 0, 0, HWND_MESSAGE, NULL, NULL, NULL);
    return g_owner;
}

/* Outro processo pode estar segurando a área de transferência: tenta algumas vezes. */
static int open_clipboard(void) {
    for (int i = 0; i < 10; i++) {
        if (OpenClipboard(owner())) return 1;
        Sleep(2);
    }
    return 0;
}

hg_result hg_clipboard_set_text(const char* utf8, size_t len) {
    if (!utf8 && len) return hg__fail(HG_ERR_INVALID, "hg_clipboard_set_text: argumento nulo");
    if (len > 0x7fffffff) return hg__fail(HG_ERR_INVALID, "hg_clipboard_set_text: texto grande demais");
    int n = len ? MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, utf8, (int)len, NULL, 0) : 0;
    if (len && n <= 0) return hg__fail(HG_ERR_INVALID, "hg_clipboard_set_text: UTF-8 inválido");
    HGLOBAL mem = GlobalAlloc(GMEM_MOVEABLE, sizeof(WCHAR) * ((size_t)n + 1));
    if (!mem) return hg__fail(HG_ERR_OOM, "hg_clipboard_set_text: sem memória");
    WCHAR* w = GlobalLock(mem);
    if (n) MultiByteToWideChar(CP_UTF8, 0, utf8, (int)len, w, n);
    w[n] = 0;
    GlobalUnlock(mem);
    if (!open_clipboard()) {
        GlobalFree(mem);
        return hg__fail(HG_ERR_DEVICE, "hg_clipboard_set_text: OpenClipboard falhou");
    }
    EmptyClipboard();
    HANDLE ok = SetClipboardData(CF_UNICODETEXT, mem);
    CloseClipboard();
    if (!ok) {
        GlobalFree(mem);
        return hg__fail(HG_ERR_DEVICE, "hg_clipboard_set_text: SetClipboardData falhou");
    }
    return HG_OK; /* o sistema agora é dono de `mem` */
}

hg_result hg_clipboard_get_text(char* out, size_t cap, size_t* out_len) {
    if (!out_len) return hg__fail(HG_ERR_INVALID, "hg_clipboard_get_text: argumento nulo");
    *out_len = 0;
    if (!IsClipboardFormatAvailable(CF_UNICODETEXT)) return HG_OK;
    if (!open_clipboard()) return hg__fail(HG_ERR_DEVICE, "hg_clipboard_get_text: OpenClipboard falhou");
    hg_result r = HG_OK;
    HANDLE h = GetClipboardData(CF_UNICODETEXT);
    const WCHAR* w = h ? GlobalLock(h) : NULL;
    if (w) {
        int need = WideCharToMultiByte(CP_UTF8, 0, w, -1, NULL, 0, NULL, NULL);
        size_t len = need > 0 ? (size_t)need - 1 : 0;
        *out_len = len;
        if (out && cap >= len && len) WideCharToMultiByte(CP_UTF8, 0, w, -1, out, (int)len, NULL, NULL);
        GlobalUnlock(h);
    } else if (h) {
        r = hg__fail(HG_ERR_DEVICE, "hg_clipboard_get_text: GlobalLock falhou");
    }
    CloseClipboard();
    return r;
}

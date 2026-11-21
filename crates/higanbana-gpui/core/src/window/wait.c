/* wait.c — espera por evento com prazo (laço com animações/timers pendentes). */
#include "window.h"
#include "error.h"

hg_result hg_window_wait_event_timeout(hg_window* win, uint32_t timeout_ms, hg_event* out) {
    if (!win || !out) return hg__fail(HG_ERR_INVALID, "hg_window_wait_event_timeout: argumento nulo");
    ULONGLONG deadline = GetTickCount64() + timeout_ms;
    for (;;) {
        if (hg_window_poll_event(win, out)) return HG_OK;
        ULONGLONG now = GetTickCount64();
        if (now >= deadline) break;
        /* Acorda com qualquer mensagem nova (inclusive as já vistas mas não retiradas). */
        MsgWaitForMultipleObjectsEx(0, NULL, (DWORD)(deadline - now), QS_ALLINPUT, MWMO_INPUTAVAILABLE);
    }
    out->kind = HG_EVENT_NONE;
    return HG_OK;
}

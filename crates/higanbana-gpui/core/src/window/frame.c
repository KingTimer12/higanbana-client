/* frame.c — começo/envio/fim de frame, roteado para o backend da janela. */
#include "window.h"
#include "error.h"

hg_result hg_frame_begin(hg_window* win, hg_color clear) {
    if (!win) return hg__fail(HG_ERR_INVALID, "hg_frame_begin: janela nula");
    win->skip_frame = win->width == 0 || win->height == 0 || IsIconic(win->hwnd);
    if (win->skip_frame) return HG_OK;
    return win->ctx->backend->begin_frame(win, clear);
}

hg_result hg_frame_submit(hg_window* win, const hg_draw_list* dl) {
    if (!win || !dl) return hg__fail(HG_ERR_INVALID, "hg_frame_submit: argumento nulo");
    if (win->skip_frame) return HG_OK;
    return win->ctx->backend->submit(win, dl);
}

hg_result hg_frame_end(hg_window* win, int vsync) {
    if (!win) return hg__fail(HG_ERR_INVALID, "hg_frame_end: janela nula");
    if (win->skip_frame) return HG_OK;
    hg_result r = win->ctx->backend->end_frame(win, vsync);
    win->ctx->frame_index++;
    return r;
}

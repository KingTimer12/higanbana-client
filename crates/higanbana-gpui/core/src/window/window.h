/* window.h — estado interno de uma janela, compartilhado com os backends. */
#ifndef HG_WINDOW_H
#define HG_WINDOW_H
#include "event_queue.h"
#include "context.h"
#include <windows.h>

struct hg_window {
    hg_context*    ctx;
    HWND           hwnd;
    hg_event_queue events;
    uint32_t       width, height; /* pixels físicos da área cliente */
    float          dpi_scale;
    int            skip_frame;    /* frame atual vira no-op (minimizada) */
    hg_cursor      cursor;        /* calloc = HG_CURSOR_ARROW */
    void*          surface;       /* dono: backend->attach/detach */
    hg_window*     next;          /* lista de janelas do contexto */
};

/* Aplica novo tamanho: atualiza estado, avisa backend e enfileira evento. */
void hg__window_apply_size(hg_window* win, uint32_t width, uint32_t height);
/* Converte uma mensagem Win32 em evento; 1 se tratou. */
int  hg__window_translate(hg_window* win, UINT msg, WPARAM wp, LPARAM lp);
/* WM_SETCURSOR: aplica o cursor da janela na área cliente; 1 se tratou. */
int  hg__window_on_setcursor(hg_window* win, LPARAM lp);
/* Ativa DPI Per-Monitor V2 uma única vez por processo. */
void hg__dpi_enable_awareness(void);
float hg__dpi_scale_for(HWND hwnd);
#endif

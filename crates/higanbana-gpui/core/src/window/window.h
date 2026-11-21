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
    /* decorations (decor.c) */
    int            custom;        /* sem barra nativa */
    hg_hit_region* regions;
    uint32_t       region_count, region_cap;
    uint32_t       state;         /* últimas flags HG_WINDOW_* enviadas */
    int            fullscreen;
    int            nc_tracking;   /* TrackMouseEvent(TME_NONCLIENT) ativo */
    WINDOWPLACEMENT saved_place;  /* antes do fullscreen */
    LONG_PTR       saved_style;
};

/* Aplica novo tamanho: atualiza estado, avisa backend e enfileira evento. */
void hg__window_apply_size(hg_window* win, uint32_t width, uint32_t height);
/* Converte uma mensagem Win32 em evento; 1 se tratou. */
int  hg__window_translate(hg_window* win, UINT msg, WPARAM wp, LPARAM lp);
/* WM_SETCURSOR: aplica o cursor da janela na área cliente; 1 se tratou. */
int  hg__window_on_setcursor(hg_window* win, LPARAM lp);
/* Troca a HWND por uma idêntica (antes de prender o OpenGL depois de outro backend). */
hg_result hg__window_recreate_hwnd(hg_window* win);
/* Mensagens das decorations; 1 se tratou (resultado em *out). */
int  hg__decor_wndproc(hg_window* win, UINT msg, WPARAM wp, LPARAM lp, LRESULT* out);
/* Reavalia as flags de estado e enfileira HG_EVENT_WINDOW_STATE se mudaram. */
void hg__decor_update_state(hg_window* win);
/* Reaplica o quadro (SWP_FRAMECHANGED) depois de criar/recriar a HWND. */
void hg__decor_refresh_frame(hg_window* win);
void hg__decor_free(hg_window* win);
/* Enfileira evento de mouse com coordenadas de cliente em pixels físicos. */
void hg__window_push_mouse(hg_window* win, hg_event_kind kind, int x, int y, uint32_t button);
/* Ativa DPI Per-Monitor V2 uma única vez por processo. */
void hg__dpi_enable_awareness(void);
float hg__dpi_scale_for(HWND hwnd);
#endif

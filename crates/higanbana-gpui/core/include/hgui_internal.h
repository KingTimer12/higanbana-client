/* hgui_internal.h — símbolos internos (prefixo hg__) expostos só para testes.
 * Não fazem parte da API estável. */
#ifndef HGUI_INTERNAL_H
#define HGUI_INTERNAL_H
#include "hgui.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Atlas (skyline + LRU) ------------------------------------------ */
typedef struct hg_atlas hg_atlas;
typedef void (*hg_atlas_evict_fn)(void* user, uint64_t key);

hg_atlas* hg__atlas_create(uint32_t width, uint32_t height);
void      hg__atlas_destroy(hg_atlas* atlas);
void      hg__atlas_on_evict(hg_atlas* atlas, hg_atlas_evict_fn fn, void* user);
/* out_uv em pixels. out_inserted=1 quando o espaço é novo (precisa upload). */
hg_result hg__atlas_get_or_insert(hg_atlas* atlas, uint64_t key, uint32_t w, uint32_t h,
                                  uint64_t frame, hg_rect* out_uv, int* out_inserted);
int       hg__atlas_contains(const hg_atlas* atlas, uint64_t key);
uint32_t  hg__atlas_entry_count(const hg_atlas* atlas);
/* Dado livre por entrada (ex.: offset do glyph). 0 se a chave não existe. */
int       hg__atlas_get_user(const hg_atlas* atlas, uint64_t key, uint64_t* out);
void      hg__atlas_set_user(hg_atlas* atlas, uint64_t key, uint64_t user);
/* Esquece todas as entradas (ex.: textura de GPU recriada na troca de backend). */
void      hg__atlas_clear(hg_atlas* atlas);

/* ---- Backends externos ------------------------------------------------ */
/* Registra um backend implementado fora do core (ex.: WGPU em Rust). Ponteiro
 * para `hg_backend_vtbl` (backend.h). Chamar antes do primeiro
 * hg_enumerate_backends/hg_context_create; depois disso devolve HG_ERR_INVALID. */
hg_result hg__backend_register(const void* vtbl);

/* Checagens baratas para o `detect` dos backends (não carregam driver de GPU). */
/* 1 se a DLL de sistema `name` existe (carrega e solta, sem DllMain de driver). */
int hg__detect_dll(const wchar_t* name);
/* 1 se há adaptador de hardware; escreve o nome em info->adapter (info pode ser NULL). */
int hg__detect_adapter(hg_backend_info* info);

/* ---- Janela (testes) -------------------------------------------------- */
void hg__window_inject_resize(hg_window* win, uint32_t width, uint32_t height);
void hg__window_request_close(hg_window* win);
/* Troca a HWND por uma idêntica (como na troca para OpenGL). */
hg_result hg__window_recreate_hwnd(hg_window* win);
/* Lê um pixel (BGRA8) do backbuffer atual; chamar entre submit e end. */
hg_result hg__window_read_pixel(hg_window* win, uint32_t x, uint32_t y, uint32_t* out_bgra);

#ifdef __cplusplus
}
#endif
#endif

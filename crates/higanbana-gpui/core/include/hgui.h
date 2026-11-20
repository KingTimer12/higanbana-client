/* hgui.h — API pública do core C do higanbana-gpui. */
#ifndef HGUI_H
#define HGUI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Erros --------------------------------------------------------- */
typedef int32_t hg_result;
#define HG_OK              0
#define HG_ERR_UNAVAILABLE 1
#define HG_ERR_DEVICE      2
#define HG_ERR_OOM         3
#define HG_ERR_INVALID     4

const char* hg_last_error(void);

/* ---- Geometria ----------------------------------------------------- */
typedef struct hg_rect  { float x, y, w, h; } hg_rect;
typedef struct hg_color { float r, g, b, a; } hg_color;

typedef struct hg_quad {
    hg_rect  rect;
    hg_color fill;
    hg_color border;
    float    radius[4];    /* tl, tr, br, bl */
    float    border_width;
    float    shadow_blur;
    hg_color shadow;
} hg_quad;

/* ---- Draw list ----------------------------------------------------- */
typedef struct hg_draw_list hg_draw_list;

hg_draw_list* hg_draw_list_create(void);
void          hg_draw_list_destroy(hg_draw_list* dl);
void          hg_draw_list_reset(hg_draw_list* dl);
hg_result     hg_draw_quad(hg_draw_list* dl, const hg_quad* quad);
hg_result     hg_push_clip(hg_draw_list* dl, hg_rect clip);
void          hg_pop_clip(hg_draw_list* dl);
uint32_t      hg_draw_list_quad_count(const hg_draw_list* dl);
/* Abre uma camada nova: tudo desenhado depois fica por cima de tudo antes.
 * Dentro de uma camada a ordem é fixa: quads, imagens, texto. */
hg_result     hg_draw_list_layer(hg_draw_list* dl);

/* ---- Backends e contexto --------------------------------------------- */
typedef enum hg_backend_kind {
    HG_BACKEND_AUTO = 0,
    HG_BACKEND_D3D11,
    HG_BACKEND_D3D12,
    HG_BACKEND_VULKAN,
    HG_BACKEND_OPENGL,
    HG_BACKEND_WGPU
} hg_backend_kind;

typedef struct hg_backend_info {
    hg_backend_kind kind;
    char            name[32];
    char            adapter[128];
} hg_backend_info;

typedef struct hg_context hg_context;

/* Preenche até `max` backends disponíveis nesta máquina; devolve quantos. */
int             hg_enumerate_backends(hg_backend_info* out, int max);
/* Usa `preferred` se disponível; senão D3D11 -> OpenGL -> WGPU. */
hg_result       hg_context_create(hg_backend_kind preferred, hg_context** out);
void            hg_context_destroy(hg_context* ctx);
hg_backend_kind hg_context_backend(const hg_context* ctx);
/* Troca o backend sem fechar janelas: recria superfícies e o atlas de glyphs.
 * Se `kind` falhar, volta ao backend anterior e devolve o erro. */
hg_result       hg_context_switch_backend(hg_context* ctx, hg_backend_kind kind);
/* Adaptador do backend ativo (nome, GPU). */
hg_backend_info hg_context_backend_info(const hg_context* ctx);

/* ---- Janela e eventos ------------------------------------------------- */
typedef struct hg_window hg_window;

typedef struct hg_window_desc {
    const char* title_utf8;
    uint32_t    width, height; /* em pixels lógicos */
    int         resizable;
} hg_window_desc;

typedef enum hg_event_kind {
    HG_EVENT_NONE = 0,
    HG_EVENT_CLOSE,
    HG_EVENT_RESIZE,
    HG_EVENT_MOUSE_MOVE,
    HG_EVENT_MOUSE_DOWN,
    HG_EVENT_MOUSE_UP,
    HG_EVENT_SCROLL,
    HG_EVENT_KEY_DOWN,
    HG_EVENT_KEY_UP,
    HG_EVENT_CHAR
} hg_event_kind;

#define HG_MOD_SHIFT 1u
#define HG_MOD_CTRL  2u
#define HG_MOD_ALT   4u

typedef struct hg_event {
    hg_event_kind kind;
    uint32_t      mods;
    union {
        struct { float x, y; uint32_t button; } mouse;
        struct { float dx, dy; } scroll;
        struct { uint32_t key; int repeat; } key; /* virtual key do Windows */
        struct { uint32_t codepoint; } text;
        struct { uint32_t width, height; float dpi_scale; } resize;
    } u;
} hg_event;

hg_result hg_window_create(hg_context* ctx, const hg_window_desc* desc, hg_window** out);
void      hg_window_destroy(hg_window* win);
/* 1 = evento escrito em *out; 0 = fila vazia. Não bloqueia. */
int       hg_window_poll_event(hg_window* win, hg_event* out);
/* Bloqueia até chegar um evento (app ocioso não gasta CPU). */
hg_result hg_window_wait_event(hg_window* win, hg_event* out);
float     hg_window_dpi_scale(const hg_window* win);
/* Espera um evento por até `timeout_ms`. Sem evento no prazo: out->kind = HG_EVENT_NONE.
 * Usado pelo laço enquanto há animação/timer pendente. */
hg_result hg_window_wait_event_timeout(hg_window* win, uint32_t timeout_ms, hg_event* out);

/* ---- Cursor ------------------------------------------------------------ */
typedef enum hg_cursor {
    HG_CURSOR_ARROW = 0,
    HG_CURSOR_HAND,
    HG_CURSOR_IBEAM,
    HG_CURSOR_CROSSHAIR,
    HG_CURSOR_NOT_ALLOWED,
    HG_CURSOR_RESIZE_EW,
    HG_CURSOR_RESIZE_NS,
    HG_CURSOR_MOVE,
    HG_CURSOR_WAIT
} hg_cursor;

/* Cursor mostrado enquanto o mouse está na área cliente (aplicado via WM_SETCURSOR). */
hg_result hg_window_set_cursor(hg_window* win, hg_cursor cursor);
hg_cursor hg_window_cursor(const hg_window* win);

/* ---- Área de transferência -------------------------------------------- */
/* Copia `len` bytes UTF-8 como CF_UNICODETEXT. */
hg_result hg_clipboard_set_text(const char* utf8, size_t len);
/* Escreve em *out_len o tamanho UTF-8 do texto da área de transferência (0 se vazia).
 * Copia para `out` só se `cap >= *out_len` (sem terminador); senão só informa o tamanho. */
hg_result hg_clipboard_get_text(char* out, size_t cap, size_t* out_len);

/* ---- Frame ------------------------------------------------------------ */
hg_result hg_frame_begin(hg_window* win, hg_color clear);
hg_result hg_frame_submit(hg_window* win, const hg_draw_list* dl);
hg_result hg_frame_end(hg_window* win, int vsync);

/* ---- Imagens ---------------------------------------------------------- */
typedef struct hg_image hg_image;

/* Pixels RGBA8 com alfa pré-multiplicado; `stride` = bytes por linha. O core copia
 * os pixels (reenvio à GPU na troca de backend) e cria a textura no primeiro uso.
 * Destruir só depois de enviar (hg_frame_submit) os frames que a usam. */
hg_result hg_image_create(hg_context* ctx, uint32_t width, uint32_t height, const uint8_t* rgba,
                          uint32_t stride, hg_image** out);
void      hg_image_destroy(hg_image* img);
void      hg_image_size(const hg_image* img, uint32_t* width, uint32_t* height);

typedef struct hg_image_draw {
    hg_rect  rect;      /* destino em pixels */
    hg_rect  uv;        /* região da imagem, 0..1 ({0,0,1,1} = inteira) */
    hg_color tint;      /* multiplica a imagem; {1,1,1,1} = original */
    float    radius[4]; /* tl, tr, br, bl */
} hg_image_draw;

hg_result hg_draw_image(hg_draw_list* dl, hg_image* img, const hg_image_draw* draw);

/* ---- Estatísticas ----------------------------------------------------- */
typedef struct hg_stats {
    float    frame_ms;      /* intervalo entre os dois últimos hg_frame_end */
    uint64_t gpu_bytes;     /* buffers + texturas do backend */
    uint64_t arena_bytes;   /* memória reservada pela última draw list enviada */
    uint32_t draw_calls;    /* do último frame */
    uint32_t atlas_entries; /* glyphs residentes no atlas */
} hg_stats;

void hg_context_stats(const hg_context* ctx, hg_stats* out);

/* ---- Texto ------------------------------------------------------------ */
typedef uint32_t hg_font_id;

typedef struct hg_text_metrics {
    float width, height;
    float baseline; /* distância do topo até a linha de base da 1ª linha */
} hg_text_metrics;

/* weight: 100..900 (400 normal, 700 negrito). Fontes do sistema, com fallback (CJK etc.). */
hg_result hg_font_load(hg_context* ctx, const char* family_utf8, float weight, int italic, hg_font_id* out);
/* max_width <= 0: sem quebra de linha. */
hg_result hg_text_measure(hg_context* ctx, hg_font_id font, float size_px, const char* utf8, size_t len,
                          float max_width, hg_text_metrics* out);
/* (x, y) = canto superior esquerdo da caixa de texto. */
hg_result hg_draw_text(hg_draw_list* dl, hg_font_id font, float size_px, float x, float y, float max_width,
                       hg_color color, const char* utf8, size_t len);

/* x (px, relativo ao início, linha única) do cursor antes do byte `byte_index`
 * (fronteira de caractere; == len = fim do texto). */
hg_result hg_text_caret_x(hg_context* ctx, hg_font_id font, float size_px, const char* utf8, size_t len,
                          size_t byte_index, float* out_x);
/* Fronteira de caractere (índice de byte) mais próxima do ponto x (linha única). */
hg_result hg_text_hit_test(hg_context* ctx, hg_font_id font, float size_px, const char* utf8, size_t len,
                           float x, size_t* out_byte_index);
/* x de todas as fronteiras num só layout: `out_x[k]` = x antes do k-ésimo codepoint,
 * `out_x[count-1]` = fim. `count` precisa ser (codepoints + 1). */
hg_result hg_text_caret_positions(hg_context* ctx, hg_font_id font, float size_px, const char* utf8, size_t len,
                                  float* out_x, size_t count);

#ifdef __cplusplus
}
#endif
#endif /* HGUI_H */

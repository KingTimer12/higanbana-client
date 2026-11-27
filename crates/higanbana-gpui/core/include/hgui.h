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
    HG_EVENT_CHAR,
    HG_EVENT_WINDOW_STATE, /* u.state.flags (HG_WINDOW_*) mudou */
    HG_EVENT_WAKE          /* hg_window_wake (outra thread pediu para acordar) */
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
        struct { uint32_t flags; } state;
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

/* Thread-safe: acorda hg_window_wait_event* com HG_EVENT_WAKE (ex.: quadro de vídeo novo). */
void      hg_window_wake(hg_window* win);

/* ---- Decorations e estado da janela ----------------------------------- */
/* Região da janela publicada pelo app (pixels lógicos do cliente). */
typedef enum hg_hit { HG_HIT_CLIENT = 0, HG_HIT_CAPTION, HG_HIT_MAXIMIZE } hg_hit;
typedef struct hg_hit_region { float x, y, w, h; hg_hit kind; } hg_hit_region;

typedef enum hg_window_cmd {
    HG_WINDOW_MINIMIZE = 0,
    HG_WINDOW_MAXIMIZE,
    HG_WINDOW_RESTORE,
    HG_WINDOW_TOGGLE_MAXIMIZE,
    HG_WINDOW_CLOSE,
    HG_WINDOW_ENTER_FULLSCREEN,
    HG_WINDOW_EXIT_FULLSCREEN
} hg_window_cmd;

#define HG_WINDOW_MAXIMIZED  1u
#define HG_WINDOW_MINIMIZED  2u
#define HG_WINDOW_FULLSCREEN 4u
#define HG_WINDOW_FOCUSED    8u

/* custom != 0: sem barra nativa; o app desenha a barra e publica as regiões.
 * Mantém snap, sombra e animações do Windows. */
hg_result hg_window_set_decorations(hg_window* win, int custom);
/* Copia as regiões; a última que contém o ponto vence (ordem de pintura).
 * Bordas de redimensionar (6 px) são do core. */
hg_result hg_window_set_hit_regions(hg_window* win, const hg_hit_region* regions, uint32_t count);
hg_result hg_window_command(hg_window* win, hg_window_cmd cmd);
/* Flags HG_WINDOW_* atuais. */
uint32_t  hg_window_state(const hg_window* win);

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
    HG_CURSOR_WAIT,
    HG_CURSOR_NONE  /* oculto (vídeo em tela cheia) */
} hg_cursor;

/* Cursor mostrado enquanto o mouse está na área cliente (aplicado via WM_SETCURSOR). */
hg_result hg_window_set_cursor(hg_window* win, hg_cursor cursor);
hg_cursor hg_window_cursor(const hg_window* win);

/* Tamanho mínimo da área cliente em pixels lógicos (0, 0 = sem limite). */
hg_result hg_window_set_min_size(hg_window* win, float width, float height);

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
 * os pixels, cria a textura no primeiro uso e solta a cópia (a imagem que recebe
 * hg_image_update guarda a dela). Na troca de backend, imagem sem cópia volta vazia
 * até ser recriada por quem tem os pixels.
 * Destruir só depois de enviar (hg_frame_submit) os frames que a usam. */
hg_result hg_image_create(hg_context* ctx, uint32_t width, uint32_t height, const uint8_t* rgba,
                          uint32_t stride, hg_image** out);
void      hg_image_destroy(hg_image* img);
/* Troca os pixels (mesmo tamanho e formato de hg_image_create). A textura é atualizada no
 * lugar quando o backend suporta; senão é recriada no próximo desenho. */
hg_result hg_image_update(hg_image* img, const uint8_t* rgba, uint32_t stride);
void      hg_image_size(const hg_image* img, uint32_t* width, uint32_t* height);

typedef struct hg_image_draw {
    hg_rect  rect;      /* destino em pixels */
    hg_rect  uv;        /* região da imagem, 0..1 ({0,0,1,1} = inteira) */
    hg_color tint;      /* multiplica a imagem; {1,1,1,1} = original */
    float    radius[4]; /* tl, tr, br, bl */
} hg_image_draw;

hg_result hg_draw_image(hg_draw_list* dl, hg_image* img, const hg_image_draw* draw);

/* ---- Shaders do usuário ------------------------------------------------ */
typedef struct hg_shader hg_shader;

/* Código já traduzido por alvo (UTF-8 terminado em 0; qualquer um pode ser NULL).
 * glsl_vs/glsl_fs: um fonte por estágio; wgsl: módulo completo. O core copia tudo. */
typedef struct hg_shader_code {
    const char*     hlsl;
    const char*     glsl_vs;
    const char*     glsl_fs;
    const char*     wgsl;
    const uint32_t* spirv;
    size_t          spirv_words;
} hg_shader_code;

hg_result   hg_shader_create(hg_context* ctx, const hg_shader_code* code, hg_shader** out);
void        hg_shader_destroy(hg_shader* sh);
/* Erro do compilador nativo no backend atual; NULL = ok. */
const char* hg_shader_error(const hg_shader* sh);

#define HG_SHADER_BACKDROP 1u /* flags: o shader lê o que já foi desenhado atrás */
#define HG_SHADER_PARAMS   16

typedef struct hg_shader_draw {
    hg_rect  rect;      /* destino em pixels */
    float    radius[4]; /* tl, tr, br, bl */
    float    time;      /* segundos */
    float    scale;     /* fator de escala da janela */
    float    opacity;
    float    params[HG_SHADER_PARAMS][4];
    uint32_t flags;
} hg_shader_draw;

/* Desenha o shader numa camada própria, herdando o clip atual. */
hg_result hg_draw_shader(hg_draw_list* dl, hg_shader* sh, const hg_shader_draw* d);

/* ---- Alvo de vídeo (OpenGL → textura D3D11, só no backend D3D11) ------- */
/* Uma thread de render (mpv) desenha com OpenGL em texturas D3D11 do device da UI
 * (WGL_NV_DX_interop2); a UI desenha a última publicada como hg_image.
 * create, image e destroy: thread da UI; gl_init, gl_proc, begin, end e gl_release: thread de render. */
typedef struct hg_video_target hg_video_target;

/* HG_ERR_UNAVAILABLE se o backend atual não é D3D11. */
hg_result hg_video_target_create(hg_context* ctx, hg_video_target** out);
/* Contexto OpenGL escondido + interop, corrente nesta thread. HG_ERR_UNAVAILABLE sem a extensão. */
hg_result hg_video_target_gl_init(hg_video_target* t);
/* Função GL/WGL pelo nome (para o mpv). NULL se não existir. */
void*     hg_video_target_gl_proc(hg_video_target* t, const char* name);
/* Começa um quadro w×h: *fbo fica ligado com a textura livre como cor. */
hg_result hg_video_target_begin(hg_video_target* t, uint32_t width, uint32_t height, uint32_t* fbo);
/* Termina o quadro e o publica para a UI. */
hg_result hg_video_target_end(hg_video_target* t);
/* Libera GL e interop (antes de a thread de render sair). */
void      hg_video_target_gl_release(hg_video_target* t);
/* Imagem com o último quadro publicado (*out NULL antes do primeiro); *fresh = 1 se trocou
 * desde a chamada anterior. A imagem pertence ao alvo: vale até hg_video_target_destroy. */
hg_result hg_video_target_image(hg_video_target* t, hg_image** out, int* fresh);
/* Depois de hg_video_target_gl_release (ou se o GL nunca iniciou). */
void      hg_video_target_destroy(hg_video_target* t);

/* ---- Estatísticas ----------------------------------------------------- */
typedef struct hg_stats {
    float    frame_ms;      /* intervalo entre os dois últimos hg_frame_end */
    uint64_t gpu_bytes;     /* buffers + texturas do backend */
    uint64_t arena_bytes;   /* memória reservada pela última draw list enviada */
    uint32_t draw_calls;    /* do último frame */
    uint32_t atlas_entries; /* glyphs residentes no atlas */
    uint64_t image_ram_bytes; /* cópias de pixels de imagem ainda na RAM (dinâmicas ou não enviadas) */
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
/* Soma um .ttf/.otf às famílias deste contexto (nada é instalado no sistema).
 * Depois disso hg_font_load acha a família pelo nome. Precisa do Windows 10 1703+. */
hg_result hg_font_add_file(hg_context* ctx, const char* path_utf8);
/* *out = 1 se a família existe (sistema ou arquivo somado). */
hg_result hg_font_family_exists(hg_context* ctx, const char* family_utf8, int* out);
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

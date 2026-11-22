/* text.h — tipos internos do sistema de texto, independentes de backend. */
#ifndef HG_TEXT_H
#define HG_TEXT_H
#include "hgui_internal.h"
#include "dwrite_c.h"

#define HG_GLYPH_ATLAS_SIZE 2048u
#define HG_FORMAT_CACHE 8

/* Um texto gravado na draw list (UTF-16 já validado, na arena). */
typedef struct hg_text_run {
    struct hg_text_run* next;
    hg_font_id          font;
    float               size, x, y, max_width;
    hg_color            color;
    const WCHAR*        text;
    UINT32              len;
} hg_text_run;

/* Instância de glyph pronta para a GPU (pixels; uv em pixels do atlas). */
typedef struct hg_glyph_instance {
    hg_rect  rect;
    hg_rect  uv;
    hg_color color;
} hg_glyph_instance;

/* Backend recebe uploads de atlas (R8) e instâncias. */
typedef struct hg_glyph_sink {
    void*     user;
    hg_result (*upload)(void* user, hg_rect at, const uint8_t* r8, uint32_t pitch);
    hg_result (*emit)(void* user, const hg_glyph_instance* glyph);
} hg_glyph_sink;

typedef struct hg_font {
    WCHAR*             family;
    UINT32             weight;
    DWRITE_FONT_STYLE  style;
    float              format_sizes[HG_FORMAT_CACHE];
    IDWriteTextFormat* formats[HG_FORMAT_CACHE];
    uint32_t           next_format; /* rodízio do cache */
} hg_font;

typedef struct hg_text_system {
    HMODULE         lib;
    IDWriteFactory* factory;
    hg_font*        fonts;
    uint32_t        font_count, font_capacity;
    hg_atlas*       atlas;
    uint8_t*        scratch; /* raster reaproveitado entre glyphs */
    size_t          scratch_size;
    IUnknown**      faces;   /* font faces vistas, com AddRef: id estável p/ chave do atlas */
    uint32_t        face_count, face_capacity;
    uint64_t        skipped_glyphs; /* glyphs pulados por atlas cheio no frame */
    IUnknown*       file_builder;    /* IDWriteFontSetBuilder1 das fontes de arquivo (font_files.c) */
    IUnknown*       file_collection; /* IDWriteFontCollection1 montada do builder; NULL = só sistema */
} hg_text_system;

/* text_system.c */
hg_result hg__text_system_get(hg_context* ctx, hg_text_system** out); /* cria sob demanda */
void      hg__text_system_destroy(hg_text_system* ts);
hg_result hg__text_format(hg_text_system* ts, hg_font_id font, float size, IDWriteTextFormat** out);
uint8_t*  hg__text_scratch(hg_text_system* ts, size_t bytes);
/* Índice estável da face (segura uma referência). UINT32_MAX = sem memória. */
uint32_t  hg__text_face_id(hg_text_system* ts, IDWriteFontFace* face);

/* font_files.c — coleção das fontes somadas por arquivo (Windows 10 1703+).
 * Coleção para a família: a de arquivos se ela estiver lá, senão NULL (sistema). */
IUnknown* hg__font_collection_for(hg_text_system* ts, const WCHAR* family);
void      hg__font_files_destroy(hg_text_system* ts);

/* text_utf.c — NULL + HG_ERR_INVALID quando o UTF-8 é inválido. */
hg_result hg__utf8_to_utf16(const char* utf8, size_t len, WCHAR* out, int out_cap, int* out_len);

/* text_layout.c */
hg_result hg__text_layout(hg_text_system* ts, hg_font_id font, float size, const WCHAR* text, UINT32 len,
                          float max_width, IDWriteTextLayout** out);

/* text_renderer.c — rasteriza um texto gravado, via sink. */
hg_result hg__text_render_run(hg_text_system* ts, const hg_text_run* run, uint64_t frame, hg_glyph_sink* sink);
#endif

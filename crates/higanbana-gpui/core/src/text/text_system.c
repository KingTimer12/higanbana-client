/* text_system.c — fábrica DirectWrite (dwrite.dll dinâmica), registro de fontes e cache de formatos. */
#include "text.h"
#include "context.h"
#include "error.h"
#include <stdlib.h>
#include <string.h>

static const GUID k_iid_factory = { 0xb859ee5a, 0xd838, 0x4b5b, { 0xa2, 0xe8, 0x1a, 0xdc, 0x7d, 0x93, 0xdb, 0x48 } };

static hg_result system_create(hg_text_system** out) {
    hg_text_system* ts = calloc(1, sizeof *ts);
    if (!ts) return hg__fail(HG_ERR_OOM, "texto: sem memória");
    ts->lib = LoadLibraryW(L"dwrite.dll");
    hg_dwrite_create_factory_fn create =
        ts->lib ? (hg_dwrite_create_factory_fn)(void*)GetProcAddress(ts->lib, "DWriteCreateFactory") : NULL;
    if (!create || FAILED(create(DWRITE_FACTORY_TYPE_SHARED, &k_iid_factory, (IUnknown**)&ts->factory))) {
        hg__text_system_destroy(ts);
        return hg__fail(HG_ERR_UNAVAILABLE, "texto: DirectWrite indisponível");
    }
    ts->atlas = hg__atlas_create(HG_GLYPH_ATLAS_SIZE, HG_GLYPH_ATLAS_SIZE);
    if (!ts->atlas) { hg__text_system_destroy(ts); return hg__fail(HG_ERR_OOM, "texto: sem atlas"); }
    *out = ts;
    return HG_OK;
}

hg_result hg__text_system_get(hg_context* ctx, hg_text_system** out) {
    if (!ctx->text) {
        hg_result r = system_create(&ctx->text);
        if (r != HG_OK) return r;
    }
    *out = ctx->text;
    return HG_OK;
}

void hg__text_system_destroy(hg_text_system* ts) {
    if (!ts) return;
    for (uint32_t i = 0; i < ts->font_count; i++) {
        for (int k = 0; k < HG_FORMAT_CACHE; k++) DW_RELEASE(ts->fonts[i].formats[k]);
        free(ts->fonts[i].family);
    }
    free(ts->fonts);
    for (uint32_t i = 0; i < ts->face_count; i++) ts->faces[i]->lpVtbl->Release(ts->faces[i]);
    free(ts->faces);
    free(ts->scratch);
    hg__font_files_destroy(ts);
    hg__atlas_destroy(ts->atlas);
    DW_RELEASE(ts->factory);
    if (ts->lib) FreeLibrary(ts->lib);
    free(ts);
}

uint8_t* hg__text_scratch(hg_text_system* ts, size_t bytes) {
    if (bytes > ts->scratch_size) {
        uint8_t* grown = realloc(ts->scratch, bytes);
        if (!grown) return NULL;
        ts->scratch = grown;
        ts->scratch_size = bytes;
    }
    return ts->scratch;
}

static WCHAR* wide_copy(const char* utf8) {
    int n = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, utf8, -1, NULL, 0);
    WCHAR* w = n > 0 ? malloc(sizeof(WCHAR) * n) : NULL;
    if (w) MultiByteToWideChar(CP_UTF8, 0, utf8, -1, w, n);
    return w;
}

hg_result hg_font_load(hg_context* ctx, const char* family_utf8, float weight, int italic, hg_font_id* out) {
    if (!ctx || !family_utf8 || !out) return hg__fail(HG_ERR_INVALID, "hg_font_load: argumento nulo");
    hg_text_system* ts;
    hg_result r = hg__text_system_get(ctx, &ts);
    if (r != HG_OK) return r;
    if (ts->font_count == ts->font_capacity) {
        uint32_t cap = ts->font_capacity ? ts->font_capacity * 2 : 8;
        hg_font* grown = realloc(ts->fonts, sizeof *grown * cap);
        if (!grown) return hg__fail(HG_ERR_OOM, "hg_font_load: sem memória");
        ts->fonts = grown;
        ts->font_capacity = cap;
    }
    hg_font* f = &ts->fonts[ts->font_count];
    memset(f, 0, sizeof *f);
    f->family = wide_copy(family_utf8);
    if (!f->family) return hg__fail(HG_ERR_INVALID, "hg_font_load: nome de família inválido");
    f->weight = weight < 1 ? 400 : (weight > 999 ? 999 : (UINT32)weight);
    f->style = italic ? DWRITE_FONT_STYLE_ITALIC : DWRITE_FONT_STYLE_NORMAL;
    *out = ts->font_count++;
    return HG_OK;
}

hg_result hg__text_format(hg_text_system* ts, hg_font_id font, float size, IDWriteTextFormat** out) {
    if (font >= ts->font_count || size <= 0) return hg__fail(HG_ERR_INVALID, "texto: fonte ou tamanho inválido");
    hg_font* f = &ts->fonts[font];
    for (int k = 0; k < HG_FORMAT_CACHE; k++)
        if (f->formats[k] && f->format_sizes[k] == size) { *out = f->formats[k]; return HG_OK; }
    IDWriteTextFormat* fmt = NULL;
    if (FAILED(ts->factory->lpVtbl->CreateTextFormat(ts->factory, f->family, hg__font_collection_for(ts, f->family), f->weight, f->style,
                                                     DWRITE_FONT_STRETCH_NORMAL, size, L"", &fmt)))
        return hg__fail(HG_ERR_DEVICE, "texto: CreateTextFormat falhou");
    uint32_t slot = f->next_format++ % HG_FORMAT_CACHE;
    DW_RELEASE(f->formats[slot]);
    f->formats[slot] = fmt;
    f->format_sizes[slot] = size;
    *out = fmt;
    return HG_OK;
}

uint32_t hg__text_face_id(hg_text_system* ts, IDWriteFontFace* face) {
    IUnknown* unk = (IUnknown*)face;
    for (uint32_t i = 0; i < ts->face_count; i++)
        if (ts->faces[i] == unk) return i;
    if (ts->face_count == ts->face_capacity) {
        uint32_t cap = ts->face_capacity ? ts->face_capacity * 2 : 8;
        IUnknown** grown = realloc(ts->faces, sizeof *grown * cap);
        if (!grown) return UINT32_MAX;
        ts->faces = grown;
        ts->face_capacity = cap;
    }
    unk->lpVtbl->AddRef(unk);
    ts->faces[ts->face_count] = unk;
    return ts->face_count++;
}

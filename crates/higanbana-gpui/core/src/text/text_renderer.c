/* text_renderer.c — IDWriteTextRenderer em C: recebe glyph runs do layout,
 * rasteriza glyphs ausentes no atlas e emite instâncias pelo sink. */
#include "text.h"
#include "error.h"
#include <math.h>

#define HG_SUBPIXEL_STEPS 4

typedef struct draw_ctx {
    hg_text_system*    ts;
    const hg_text_run* run;
    uint64_t           frame;
    hg_glyph_sink*     sink;
    hg_result          result;
} draw_ctx;

/* ---- glyph -> atlas ------------------------------------------------------ */

static uint64_t glyph_key(const IDWriteFontFace* face, float size, UINT16 index, int sub) {
    uint64_t h = (uint64_t)(uintptr_t)face * 0x9E3779B97F4A7C15ULL;
    h ^= ((uint64_t)(uint32_t)lroundf(size * 4.0f) << 20) ^ ((uint64_t)index << 2) ^ (uint64_t)sub;
    return h;
}

static uint64_t pack_offset(LONG left, LONG top) { return ((uint64_t)(uint32_t)left << 32) | (uint32_t)top; }
static LONG offset_left(uint64_t u) { return (LONG)(int32_t)(u >> 32); }
static LONG offset_top(uint64_t u) { return (LONG)(int32_t)(u & 0xffffffffu); }

/* CLEARTYPE_3x1 dá 3 amostras por pixel; média vira cobertura em escala de cinza. */
static void cleartype_to_r8(const BYTE* src, uint8_t* dst, size_t pixels) {
    for (size_t i = 0; i < pixels; i++) dst[i] = (uint8_t)((src[i * 3] + src[i * 3 + 1] + src[i * 3 + 2]) / 3);
}

static hg_result rasterize(draw_ctx* c, const DWRITE_GLYPH_RUN* src, UINT16 index, int sub, uint64_t key) {
    FLOAT zero_advance = 0;
    DWRITE_GLYPH_OFFSET zero_offset = { 0, 0 };
    DWRITE_GLYPH_RUN one = *src;
    one.glyphCount = 1;
    one.glyphIndices = &index;
    one.glyphAdvances = &zero_advance;
    one.glyphOffsets = &zero_offset;

    IDWriteGlyphRunAnalysis* an = NULL;
    IDWriteFactory* f = c->ts->factory;
    if (FAILED(f->lpVtbl->CreateGlyphRunAnalysis(f, &one, 1.0f, NULL, DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC,
                                                 DWRITE_MEASURING_MODE_NATURAL,
                                                 (FLOAT)sub / HG_SUBPIXEL_STEPS, 0, &an)))
        return hg__fail(HG_ERR_DEVICE, "texto: CreateGlyphRunAnalysis falhou");

    RECT b = { 0 };
    an->lpVtbl->GetAlphaTextureBounds(an, DWRITE_TEXTURE_CLEARTYPE_3x1, &b);
    uint32_t w = (uint32_t)(b.right - b.left), h = (uint32_t)(b.bottom - b.top);
    hg_rect uv;
    int inserted = 0;
    hg_result r = hg__atlas_get_or_insert(c->ts->atlas, key, w, h, c->frame, &uv, &inserted);
    if (r == HG_OK) hg__atlas_set_user(c->ts->atlas, key, pack_offset(b.left, b.top));

    size_t pixels = (size_t)w * h;
    if (r == HG_OK && inserted && pixels > 0) {
        uint8_t* buf = hg__text_scratch(c->ts, pixels * 4); /* 3 bytes ClearType + 1 byte R8 */
        if (!buf) r = hg__fail(HG_ERR_OOM, "texto: sem memória para raster");
        else if (FAILED(an->lpVtbl->CreateAlphaTexture(an, DWRITE_TEXTURE_CLEARTYPE_3x1, &b, buf, (UINT32)(pixels * 3))))
            r = hg__fail(HG_ERR_DEVICE, "texto: CreateAlphaTexture falhou");
        else {
            cleartype_to_r8(buf, buf + pixels * 3, pixels);
            r = c->sink->upload(c->sink->user, uv, buf + pixels * 3, w);
        }
    }
    DW_RELEASE(an);
    return r;
}

/* ---- IDWriteTextRenderer ------------------------------------------------ */

static const GUID k_iid_renderer = { 0xef8a8135, 0x5cc6, 0x45fe, { 0x88, 0x25, 0xc5, 0xa0, 0x72, 0x4e, 0xb8, 0x19 } };
static const GUID k_iid_snapping = { 0xeaf3a2da, 0xecf4, 0x4d24, { 0xb6, 0x44, 0xb3, 0x4f, 0x68, 0x42, 0x02, 0x4b } };

/* Só as interfaces que a vtable realmente implementa (nada de IDWriteTextRenderer1). */
static HRESULT STDMETHODCALLTYPE r_query(IDWriteTextRenderer* self, REFIID iid, void** out) {
    if (IsEqualIID(iid, &IID_IUnknown) || IsEqualIID(iid, &k_iid_snapping) || IsEqualIID(iid, &k_iid_renderer)) {
        *out = self;
        return S_OK;
    }
    *out = NULL;
    return E_NOINTERFACE;
}
static ULONG STDMETHODCALLTYPE r_ref(IDWriteTextRenderer* self) { (void)self; return 1; } /* objeto estático */
static HRESULT STDMETHODCALLTYPE r_snapping(IDWriteTextRenderer* s, void* c, BOOL* out) { (void)s; (void)c; *out = FALSE; return S_OK; }
static HRESULT STDMETHODCALLTYPE r_transform(IDWriteTextRenderer* s, void* c, DWRITE_MATRIX* m) {
    (void)s; (void)c;
    *m = (DWRITE_MATRIX){ 1, 0, 0, 1, 0, 0 };
    return S_OK;
}
static HRESULT STDMETHODCALLTYPE r_ppd(IDWriteTextRenderer* s, void* c, FLOAT* out) { (void)s; (void)c; *out = 1.0f; return S_OK; }
static HRESULT STDMETHODCALLTYPE r_line(IDWriteTextRenderer* s, void* c, FLOAT x, FLOAT y, const void* l, IUnknown* e) {
    (void)s; (void)c; (void)x; (void)y; (void)l; (void)e;
    return S_OK;
}
static HRESULT STDMETHODCALLTYPE r_inline(IDWriteTextRenderer* s, void* c, FLOAT x, FLOAT y, IUnknown* o, BOOL a,
                                          BOOL b, IUnknown* e) {
    (void)s; (void)c; (void)x; (void)y; (void)o; (void)a; (void)b; (void)e;
    return S_OK;
}

static hg_result draw_glyph(draw_ctx* c, const DWRITE_GLYPH_RUN* run, UINT16 index, float x, float y) {
    float ix = floorf(x), iy = floorf(y + 0.5f);
    int sub = (int)((x - ix) * HG_SUBPIXEL_STEPS) % HG_SUBPIXEL_STEPS;
    uint64_t key = glyph_key(run->fontFace, run->fontEmSize, index, sub);

    uint64_t user;
    hg_rect uv;
    int inserted;
    hg_result r = HG_OK;
    if (!hg__atlas_contains(c->ts->atlas, key)) r = rasterize(c, run, index, sub, key);
    if (r == HG_OK) r = hg__atlas_get_or_insert(c->ts->atlas, key, 0, 0, c->frame, &uv, &inserted);
    if (r != HG_OK || uv.w == 0 || uv.h == 0 || !hg__atlas_get_user(c->ts->atlas, key, &user)) return r;

    hg_glyph_instance g;
    g.rect = (hg_rect){ ix + (float)offset_left(user), iy + (float)offset_top(user), uv.w, uv.h };
    g.uv = uv;
    g.color = c->run->color;
    return c->sink->emit(c->sink->user, &g);
}

static HRESULT STDMETHODCALLTYPE r_glyph_run(IDWriteTextRenderer* self, void* user, FLOAT x, FLOAT y,
                                             DWRITE_MEASURING_MODE mode, const DWRITE_GLYPH_RUN* run,
                                             const void* desc, IUnknown* effect) {
    (void)self; (void)mode; (void)desc; (void)effect;
    draw_ctx* c = user;
    int rtl = run->bidiLevel & 1;
    float pen = 0;
    for (UINT32 i = 0; i < run->glyphCount && c->result == HG_OK; i++) {
        float adv = run->glyphAdvances ? run->glyphAdvances[i] : 0;
        if (rtl) pen -= adv;
        float ox = run->glyphOffsets ? run->glyphOffsets[i].advanceOffset : 0;
        float oy = run->glyphOffsets ? run->glyphOffsets[i].ascenderOffset : 0;
        c->result = draw_glyph(c, run, run->glyphIndices[i], x + pen + (rtl ? -ox : ox), y - oy);
        if (!rtl) pen += adv;
    }
    return S_OK;
}

static const IDWriteTextRendererVtbl k_renderer_vtbl = {
    r_query, r_ref, r_ref, r_snapping, r_transform, r_ppd, r_glyph_run, r_line, r_line, r_inline,
};
static IDWriteTextRenderer k_renderer = { &k_renderer_vtbl };

hg_result hg__text_render_run(hg_text_system* ts, const hg_text_run* run, uint64_t frame, hg_glyph_sink* sink) {
    IDWriteTextLayout* layout = NULL;
    hg_result r = hg__text_layout(ts, run->font, run->size, run->text, run->len, run->max_width, &layout);
    if (r != HG_OK) return r;
    draw_ctx c = { ts, run, frame, sink, HG_OK };
    if (FAILED(layout->lpVtbl->Draw(layout, &c, &k_renderer, run->x, run->y)))
        c.result = hg__fail(HG_ERR_DEVICE, "texto: IDWriteTextLayout::Draw falhou");
    DW_RELEASE(layout);
    return c.result;
}

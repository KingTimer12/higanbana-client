/* text_hit.c — posições de cursor e hit test em texto de uma linha (IDWriteTextLayout). */
#include "text.h"
#include "error.h"
#include <stdlib.h>

#define HG_STACK_WCHARS 512

/* Bytes do codepoint UTF-8 que começa com `lead` (UTF-8 já validado). */
static size_t utf8_width(unsigned char lead) {
    if (lead < 0x80) return 1;
    if (lead < 0xE0) return 2;
    if (lead < 0xF0) return 3;
    return 4;
}

/* Layout de uma linha com o texto já convertido para UTF-16. */
typedef struct line {
    IDWriteTextLayout* layout;
    WCHAR              stack[HG_STACK_WCHARS];
    WCHAR*             wide;
    UINT32             n;
} line;

static hg_result line_open(line* l, hg_context* ctx, hg_font_id font, float size, const char* utf8, size_t len) {
    l->layout = NULL;
    l->wide = l->stack;
    l->n = 0;
    if (!ctx || (!utf8 && len)) return hg__fail(HG_ERR_INVALID, "texto: argumento nulo");
    hg_text_system* ts;
    int n = 0;
    hg_result r = hg__text_system_get(ctx, &ts);
    if (r == HG_OK) r = hg__utf8_to_utf16(utf8, len, NULL, 0, &n);
    if (r != HG_OK) return r;
    if (n > HG_STACK_WCHARS && !(l->wide = malloc(sizeof(WCHAR) * n))) {
        l->wide = l->stack;
        return hg__fail(HG_ERR_OOM, "texto: sem memória");
    }
    hg__utf8_to_utf16(utf8, len, l->wide, n, &n);
    l->n = (UINT32)n;
    return hg__text_layout(ts, font, size, l->wide, l->n, 0, &l->layout);
}

static void line_close(line* l) {
    DW_RELEASE(l->layout);
    if (l->wide != l->stack) free(l->wide);
}

/* Avança um codepoint: devolve bytes consumidos e soma as unidades UTF-16 em *pos. */
static size_t step(const char* utf8, size_t b, UINT32* pos) {
    size_t w = utf8_width((unsigned char)utf8[b]);
    *pos += w == 4 ? 2 : 1;
    return w;
}

/* x da borda inicial da unidade UTF-16 `pos` (pos >= n: fim do texto). */
static float x_at(line* l, UINT32 pos) {
    if (l->n == 0) return 0.0f;
    FLOAT x = 0, y = 0;
    DWRITE_HIT_TEST_METRICS m;
    BOOL trailing = pos >= l->n;
    if (trailing) pos = l->n - 1;
    l->layout->lpVtbl->HitTestTextPosition(l->layout, pos, trailing, &x, &y, &m);
    return x;
}

hg_result hg_text_caret_x(hg_context* ctx, hg_font_id font, float size_px, const char* utf8, size_t len,
                          size_t byte_index, float* out_x) {
    if (!out_x || byte_index > len) return hg__fail(HG_ERR_INVALID, "hg_text_caret_x: argumento inválido");
    line l;
    hg_result r = line_open(&l, ctx, font, size_px, utf8, len);
    if (r == HG_OK) {
        UINT32 pos = 0;
        for (size_t b = 0; b < byte_index;) b += step(utf8, b, &pos);
        *out_x = x_at(&l, pos);
    }
    line_close(&l);
    return r;
}

hg_result hg_text_hit_test(hg_context* ctx, hg_font_id font, float size_px, const char* utf8, size_t len, float x,
                           size_t* out_byte_index) {
    if (!out_byte_index) return hg__fail(HG_ERR_INVALID, "hg_text_hit_test: argumento nulo");
    *out_byte_index = 0;
    line l;
    hg_result r = line_open(&l, ctx, font, size_px, utf8, len);
    if (r == HG_OK && l.n > 0) {
        BOOL trailing = FALSE, inside = FALSE;
        DWRITE_HIT_TEST_METRICS m;
        l.layout->lpVtbl->HitTestPoint(l.layout, x, 0.0f, &trailing, &inside, &m);
        UINT32 target = m.textPosition + (trailing ? m.length : 0);
        size_t b = 0;
        for (UINT32 pos = 0; b < len && pos < target;) b += step(utf8, b, &pos);
        *out_byte_index = b;
    }
    line_close(&l);
    return r;
}

hg_result hg_text_caret_positions(hg_context* ctx, hg_font_id font, float size_px, const char* utf8, size_t len,
                                  float* out_x, size_t count) {
    if (!out_x || count == 0) return hg__fail(HG_ERR_INVALID, "hg_text_caret_positions: argumento nulo");
    line l;
    hg_result r = line_open(&l, ctx, font, size_px, utf8, len);
    if (r == HG_OK) {
        UINT32 pos = 0;
        size_t k = 0, b = 0;
        for (; b < len && k + 1 < count; k++) {
            out_x[k] = x_at(&l, pos);
            b += step(utf8, b, &pos);
        }
        if (b < len || k + 1 != count)
            r = hg__fail(HG_ERR_INVALID, "hg_text_caret_positions: count != codepoints + 1");
        else
            out_x[k] = x_at(&l, l.n);
    }
    line_close(&l);
    return r;
}

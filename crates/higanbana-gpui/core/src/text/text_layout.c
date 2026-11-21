/* text_layout.c — layout/medição e gravação de texto na draw list. */
#include "text.h"
#include "context.h"
#include "draw_list.h"
#include "error.h"
#include <stdlib.h>

#define HG_NO_WRAP_WIDTH 1.0e6f

hg_result hg__text_layout(hg_text_system* ts, hg_font_id font, float size, const WCHAR* text, UINT32 len,
                          float max_width, IDWriteTextLayout** out) {
    IDWriteTextFormat* fmt;
    hg_result r = hg__text_format(ts, font, size, &fmt);
    if (r != HG_OK) return r;
    int wrap = max_width > 0;
    fmt->lpVtbl->SetWordWrapping(fmt, wrap ? DWRITE_WORD_WRAPPING_WRAP : DWRITE_WORD_WRAPPING_NO_WRAP);
    HRESULT hr = ts->factory->lpVtbl->CreateTextLayout(ts->factory, text, len, fmt, wrap ? max_width : HG_NO_WRAP_WIDTH,
                                                       HG_NO_WRAP_WIDTH, out);
    return SUCCEEDED(hr) ? HG_OK : hg__fail(HG_ERR_DEVICE, "texto: CreateTextLayout falhou");
}

static hg_result metrics_of(IDWriteTextLayout* layout, hg_text_metrics* out) {
    DWRITE_TEXT_METRICS tm;
    DWRITE_LINE_METRICS first;
    UINT32 lines = 0;
    if (FAILED(layout->lpVtbl->GetMetrics(layout, &tm))) return hg__fail(HG_ERR_DEVICE, "texto: GetMetrics falhou");
    HRESULT hr = layout->lpVtbl->GetLineMetrics(layout, &first, 1, &lines);
    out->width = tm.widthIncludingTrailingWhitespace;
    out->height = tm.height;
    out->baseline = (SUCCEEDED(hr) || hr == E_NOT_SUFFICIENT_BUFFER) && lines > 0 ? first.baseline : 0;
    return HG_OK;
}

/* Converte para UTF-16 numa pilha local se couber, senão no heap. */
#define HG_STACK_WCHARS 512

hg_result hg_text_measure(hg_context* ctx, hg_font_id font, float size_px, const char* utf8, size_t len,
                          float max_width, hg_text_metrics* out) {
    if (!ctx || !out || (!utf8 && len)) return hg__fail(HG_ERR_INVALID, "hg_text_measure: argumento nulo");
    hg_text_system* ts;
    hg_result r = hg__text_system_get(ctx, &ts);
    int n = 0;
    if (r == HG_OK) r = hg__utf8_to_utf16(utf8, len, NULL, 0, &n);
    if (r != HG_OK) return r;

    WCHAR stack[HG_STACK_WCHARS];
    WCHAR* wide = n <= HG_STACK_WCHARS ? stack : malloc(sizeof(WCHAR) * n);
    if (!wide) return hg__fail(HG_ERR_OOM, "hg_text_measure: sem memória");
    hg__utf8_to_utf16(utf8, len, wide, n, &n);

    IDWriteTextLayout* layout = NULL;
    r = hg__text_layout(ts, font, size_px, wide, (UINT32)n, max_width, &layout);
    if (r == HG_OK) r = metrics_of(layout, out);
    DW_RELEASE(layout);
    if (wide != stack) free(wide);
    return r;
}

hg_result hg_draw_text(hg_draw_list* dl, hg_font_id font, float size_px, float x, float y, float max_width,
                       hg_color color, const char* utf8, size_t len) {
    if (!dl || (!utf8 && len)) return hg__fail(HG_ERR_INVALID, "hg_draw_text: argumento nulo");
    int n = 0;
    hg_result r = hg__utf8_to_utf16(utf8, len, NULL, 0, &n);
    if (r != HG_OK || n == 0) return r;

    hg_batch* b = hg__draw_list_batch(dl);
    hg_text_run* run = b ? hg_arena_alloc(&dl->arena, sizeof *run, _Alignof(hg_text_run)) : NULL;
    WCHAR* wide = run ? hg_arena_alloc(&dl->arena, sizeof(WCHAR) * n, _Alignof(WCHAR)) : NULL;
    if (!wide) return hg__fail(HG_ERR_OOM, "hg_draw_text: sem memória");
    hg__utf8_to_utf16(utf8, len, wide, n, &n);

    *run = (hg_text_run){ NULL, font, size_px, x, y, max_width, color, wide, (UINT32)n };
    if (b->texts_last) b->texts_last->next = run; else b->texts_first = run;
    b->texts_last = run;
    b->text_count++;
    return HG_OK;
}

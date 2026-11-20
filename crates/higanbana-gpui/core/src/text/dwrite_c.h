/* dwrite_c.h — declarações C mínimas do DirectWrite (o dwrite.h do SDK só serve C++).
 * Vtables seguem a ordem exata do dwrite.h; slots não usados viram `void* _unused`. */
#ifndef HG_DWRITE_C_H
#define HG_DWRITE_C_H
#include <windows.h>
#include <unknwn.h>

typedef struct IDWriteFactory IDWriteFactory;
typedef struct IDWriteTextFormat IDWriteTextFormat;
typedef struct IDWriteTextLayout IDWriteTextLayout;
typedef struct IDWriteGlyphRunAnalysis IDWriteGlyphRunAnalysis;
typedef struct IDWriteTextRenderer IDWriteTextRenderer;
typedef struct IDWriteFontFace IDWriteFontFace;

typedef enum { DWRITE_FACTORY_TYPE_SHARED = 0 } DWRITE_FACTORY_TYPE;
typedef enum { DWRITE_FONT_STYLE_NORMAL = 0, DWRITE_FONT_STYLE_ITALIC = 2 } DWRITE_FONT_STYLE;
typedef enum { DWRITE_FONT_STRETCH_NORMAL = 5 } DWRITE_FONT_STRETCH;
typedef enum { DWRITE_WORD_WRAPPING_WRAP = 0, DWRITE_WORD_WRAPPING_NO_WRAP = 1 } DWRITE_WORD_WRAPPING;
typedef enum { DWRITE_MEASURING_MODE_NATURAL = 0 } DWRITE_MEASURING_MODE;
typedef enum { DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC = 5 } DWRITE_RENDERING_MODE;
typedef enum { DWRITE_TEXTURE_ALIASED_1x1 = 0, DWRITE_TEXTURE_CLEARTYPE_3x1 = 1 } DWRITE_TEXTURE_TYPE;

typedef struct DWRITE_GLYPH_OFFSET { FLOAT advanceOffset, ascenderOffset; } DWRITE_GLYPH_OFFSET;
typedef struct DWRITE_MATRIX { FLOAT m11, m12, m21, m22, dx, dy; } DWRITE_MATRIX;

typedef struct DWRITE_GLYPH_RUN {
    IDWriteFontFace*           fontFace;
    FLOAT                      fontEmSize;
    UINT32                     glyphCount;
    const UINT16*              glyphIndices;
    const FLOAT*               glyphAdvances;
    const DWRITE_GLYPH_OFFSET* glyphOffsets;
    BOOL                       isSideways;
    UINT32                     bidiLevel;
} DWRITE_GLYPH_RUN;

typedef struct DWRITE_TEXT_METRICS {
    FLOAT  left, top, width, widthIncludingTrailingWhitespace, height, layoutWidth, layoutHeight;
    UINT32 maxBidiReorderingDepth, lineCount;
} DWRITE_TEXT_METRICS;

typedef struct DWRITE_LINE_METRICS {
    UINT32 length, trailingWhitespaceLength, newlineLength;
    FLOAT  height, baseline;
    BOOL   isTrimmed;
} DWRITE_LINE_METRICS;

typedef struct DWRITE_HIT_TEST_METRICS {
    UINT32 textPosition, length;
    FLOAT  left, top, width, height;
    UINT32 bidiLevel;
    BOOL   isText, isTrimmed;
} DWRITE_HIT_TEST_METRICS;

/* ---- IDWriteFactory --------------------------------------------------- */
typedef struct IDWriteFactoryVtbl {
    HRESULT(STDMETHODCALLTYPE* QueryInterface)(IDWriteFactory*, REFIID, void**);
    ULONG(STDMETHODCALLTYPE* AddRef)(IDWriteFactory*);
    ULONG(STDMETHODCALLTYPE* Release)(IDWriteFactory*);
    void* _unused3_14[12]; /* GetSystemFontCollection .. UnregisterFontFileLoader */
    HRESULT(STDMETHODCALLTYPE* CreateTextFormat)(IDWriteFactory*, const WCHAR* family, void* collection,
                                                 UINT32 weight, DWRITE_FONT_STYLE, DWRITE_FONT_STRETCH,
                                                 FLOAT size, const WCHAR* locale, IDWriteTextFormat** out);
    void* _unused16_17[2]; /* CreateTypography, GetGdiInterop */
    HRESULT(STDMETHODCALLTYPE* CreateTextLayout)(IDWriteFactory*, const WCHAR* text, UINT32 len, IDWriteTextFormat*,
                                                 FLOAT max_w, FLOAT max_h, IDWriteTextLayout** out);
    void* _unused19_22[4]; /* CreateGdiCompatibleTextLayout .. CreateNumberSubstitution */
    HRESULT(STDMETHODCALLTYPE* CreateGlyphRunAnalysis)(IDWriteFactory*, const DWRITE_GLYPH_RUN*, FLOAT ppd,
                                                       const DWRITE_MATRIX*, DWRITE_RENDERING_MODE,
                                                       DWRITE_MEASURING_MODE, FLOAT x, FLOAT y,
                                                       IDWriteGlyphRunAnalysis** out);
} IDWriteFactoryVtbl;
struct IDWriteFactory { const IDWriteFactoryVtbl* lpVtbl; };

/* ---- IDWriteTextFormat ------------------------------------------------ */
typedef struct IDWriteTextFormatVtbl {
    HRESULT(STDMETHODCALLTYPE* QueryInterface)(IDWriteTextFormat*, REFIID, void**);
    ULONG(STDMETHODCALLTYPE* AddRef)(IDWriteTextFormat*);
    ULONG(STDMETHODCALLTYPE* Release)(IDWriteTextFormat*);
    void* _unused3_4[2]; /* SetTextAlignment, SetParagraphAlignment */
    HRESULT(STDMETHODCALLTYPE* SetWordWrapping)(IDWriteTextFormat*, DWRITE_WORD_WRAPPING);
} IDWriteTextFormatVtbl;
struct IDWriteTextFormat { const IDWriteTextFormatVtbl* lpVtbl; };

/* ---- IDWriteTextLayout (herda IDWriteTextFormat: 28 slots; Draw = 58, HitTestPoint = 64) ------------ */
typedef struct IDWriteTextLayoutVtbl {
    HRESULT(STDMETHODCALLTYPE* QueryInterface)(IDWriteTextLayout*, REFIID, void**);
    ULONG(STDMETHODCALLTYPE* AddRef)(IDWriteTextLayout*);
    ULONG(STDMETHODCALLTYPE* Release)(IDWriteTextLayout*);
    void* _unused3_57[55]; /* resto de IDWriteTextFormat + setters/getters do layout */
    HRESULT(STDMETHODCALLTYPE* Draw)(IDWriteTextLayout*, void* ctx, IDWriteTextRenderer*, FLOAT x, FLOAT y);
    HRESULT(STDMETHODCALLTYPE* GetLineMetrics)(IDWriteTextLayout*, DWRITE_LINE_METRICS*, UINT32 max, UINT32* count);
    HRESULT(STDMETHODCALLTYPE* GetMetrics)(IDWriteTextLayout*, DWRITE_TEXT_METRICS*);
    void* _unused61_63[3]; /* GetOverhangMetrics, GetClusterMetrics, DetermineMinWidth */
    HRESULT(STDMETHODCALLTYPE* HitTestPoint)(IDWriteTextLayout*, FLOAT x, FLOAT y, BOOL* trailing, BOOL* inside,
                                             DWRITE_HIT_TEST_METRICS*);
    HRESULT(STDMETHODCALLTYPE* HitTestTextPosition)(IDWriteTextLayout*, UINT32 pos, BOOL trailing, FLOAT* x, FLOAT* y,
                                                    DWRITE_HIT_TEST_METRICS*);
} IDWriteTextLayoutVtbl;
struct IDWriteTextLayout { const IDWriteTextLayoutVtbl* lpVtbl; };

/* ---- IDWriteGlyphRunAnalysis ------------------------------------------ */
typedef struct IDWriteGlyphRunAnalysisVtbl {
    HRESULT(STDMETHODCALLTYPE* QueryInterface)(IDWriteGlyphRunAnalysis*, REFIID, void**);
    ULONG(STDMETHODCALLTYPE* AddRef)(IDWriteGlyphRunAnalysis*);
    ULONG(STDMETHODCALLTYPE* Release)(IDWriteGlyphRunAnalysis*);
    HRESULT(STDMETHODCALLTYPE* GetAlphaTextureBounds)(IDWriteGlyphRunAnalysis*, DWRITE_TEXTURE_TYPE, RECT*);
    HRESULT(STDMETHODCALLTYPE* CreateAlphaTexture)(IDWriteGlyphRunAnalysis*, DWRITE_TEXTURE_TYPE, const RECT*,
                                                   BYTE* out, UINT32 size);
} IDWriteGlyphRunAnalysisVtbl;
struct IDWriteGlyphRunAnalysis { const IDWriteGlyphRunAnalysisVtbl* lpVtbl; };

/* ---- IDWriteTextRenderer (implementado por nós) ----------------------- */
typedef struct IDWriteTextRendererVtbl {
    HRESULT(STDMETHODCALLTYPE* QueryInterface)(IDWriteTextRenderer*, REFIID, void**);
    ULONG(STDMETHODCALLTYPE* AddRef)(IDWriteTextRenderer*);
    ULONG(STDMETHODCALLTYPE* Release)(IDWriteTextRenderer*);
    HRESULT(STDMETHODCALLTYPE* IsPixelSnappingDisabled)(IDWriteTextRenderer*, void*, BOOL*);
    HRESULT(STDMETHODCALLTYPE* GetCurrentTransform)(IDWriteTextRenderer*, void*, DWRITE_MATRIX*);
    HRESULT(STDMETHODCALLTYPE* GetPixelsPerDip)(IDWriteTextRenderer*, void*, FLOAT*);
    HRESULT(STDMETHODCALLTYPE* DrawGlyphRun)(IDWriteTextRenderer*, void* ctx, FLOAT x, FLOAT y, DWRITE_MEASURING_MODE,
                                             const DWRITE_GLYPH_RUN*, const void* desc, IUnknown* effect);
    HRESULT(STDMETHODCALLTYPE* DrawUnderline)(IDWriteTextRenderer*, void*, FLOAT, FLOAT, const void*, IUnknown*);
    HRESULT(STDMETHODCALLTYPE* DrawStrikethrough)(IDWriteTextRenderer*, void*, FLOAT, FLOAT, const void*, IUnknown*);
    HRESULT(STDMETHODCALLTYPE* DrawInlineObject)(IDWriteTextRenderer*, void*, FLOAT, FLOAT, IUnknown*, BOOL, BOOL,
                                                 IUnknown*);
} IDWriteTextRendererVtbl;
struct IDWriteTextRenderer { const IDWriteTextRendererVtbl* lpVtbl; };

#define DW_RELEASE(p) do { if (p) { (p)->lpVtbl->Release(p); (p) = NULL; } } while (0)

typedef HRESULT(WINAPI* hg_dwrite_create_factory_fn)(DWRITE_FACTORY_TYPE, REFIID, IUnknown**);
#endif

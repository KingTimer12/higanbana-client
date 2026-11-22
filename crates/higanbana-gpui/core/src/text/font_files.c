/* font_files.c — fontes carregadas de arquivo (sem instalar) via IDWriteFactory3/FontSetBuilder1.
 * Sem vtables declaradas: chama pelo índice do slot, conferido com o dwrite_3.h do SDK. */
#include "text.h"
#include "context.h"
#include "error.h"
#include <stdlib.h>

static const GUID k_iid_factory3 = { 0x9a1b41c3, 0xd3bb, 0x466a, { 0x87, 0xfc, 0xfe, 0x67, 0x55, 0x6a, 0x3b, 0x65 } };
static const GUID k_iid_builder1 = { 0x3ff7715f, 0x3cdc, 0x4dc6, { 0x9b, 0x72, 0xec, 0x56, 0x21, 0xdc, 0xca, 0xfd } };

/* Slots (IUnknown = 0..2). */
enum {
    SLOT_FACTORY_GET_SYSTEM_COLLECTION = 3, /* IDWriteFactory::GetSystemFontCollection */
    SLOT_FACTORY_CREATE_FILE_REF = 7,       /* IDWriteFactory::CreateFontFileReference */
    SLOT_FACTORY3_CREATE_SET_BUILDER = 36,  /* IDWriteFactory3::CreateFontSetBuilder */
    SLOT_FACTORY3_COLLECTION_FROM_SET = 37, /* IDWriteFactory3::CreateFontCollectionFromFontSet */
    SLOT_BUILDER_CREATE_SET = 6,            /* IDWriteFontSetBuilder::CreateFontSet */
    SLOT_BUILDER1_ADD_FILE = 7,             /* IDWriteFontSetBuilder1::AddFontFile */
    SLOT_COLLECTION_FIND = 5,               /* IDWriteFontCollection::FindFamilyName */
};

#define SLOT(obj, i) ((*(void***)(obj))[i])
#define RELEASE(p) do { if (p) { ((IUnknown*)(p))->lpVtbl->Release((IUnknown*)(p)); (p) = NULL; } } while (0)

typedef HRESULT(STDMETHODCALLTYPE* get_system_fn)(void*, IUnknown**, BOOL);
typedef HRESULT(STDMETHODCALLTYPE* file_ref_fn)(void*, const WCHAR*, const FILETIME*, IUnknown**);
typedef HRESULT(STDMETHODCALLTYPE* out_fn)(void*, IUnknown**);
typedef HRESULT(STDMETHODCALLTYPE* from_set_fn)(void*, IUnknown*, IUnknown**);
typedef HRESULT(STDMETHODCALLTYPE* add_file_fn)(void*, IUnknown*);
typedef HRESULT(STDMETHODCALLTYPE* find_fn)(void*, const WCHAR*, UINT32*, BOOL*);

static WCHAR* wide(const char* utf8) {
    int n = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, utf8, -1, NULL, 0);
    WCHAR* w = n > 0 ? malloc(sizeof(WCHAR) * n) : NULL;
    if (w) MultiByteToWideChar(CP_UTF8, 0, utf8, -1, w, n);
    return w;
}

static BOOL has_family(IUnknown* collection, const WCHAR* family) {
    UINT32 index = 0;
    BOOL exists = FALSE;
    return collection && SUCCEEDED(((find_fn)SLOT(collection, SLOT_COLLECTION_FIND))(collection, family, &index, &exists)) && exists;
}

IUnknown* hg__font_collection_for(hg_text_system* ts, const WCHAR* family) {
    return has_family(ts->file_collection, family) ? ts->file_collection : NULL;
}

void hg__font_files_destroy(hg_text_system* ts) {
    RELEASE(ts->file_collection);
    RELEASE(ts->file_builder);
}

static hg_result ensure_builder(hg_text_system* ts, IUnknown* f3) {
    if (ts->file_builder) return HG_OK;
    IUnknown* b0 = NULL;
    HRESULT hr = ((out_fn)SLOT(f3, SLOT_FACTORY3_CREATE_SET_BUILDER))(f3, &b0);
    if (SUCCEEDED(hr)) hr = b0->lpVtbl->QueryInterface(b0, &k_iid_builder1, (void**)&ts->file_builder);
    RELEASE(b0);
    return SUCCEEDED(hr) ? HG_OK : hg__fail(HG_ERR_UNAVAILABLE, "fonte de arquivo: precisa do Windows 10 1703");
}

/* Soma o arquivo ao builder e remonta a coleção (poucas fontes por app: custo só no início). */
static hg_result add_file(hg_text_system* ts, const WCHAR* path) {
    IUnknown *f3 = NULL, *file = NULL, *set = NULL, *coll = NULL;
    if (FAILED(ts->factory->lpVtbl->QueryInterface(ts->factory, &k_iid_factory3, (void**)&f3)))
        return hg__fail(HG_ERR_UNAVAILABLE, "fonte de arquivo: precisa do Windows 10");
    hg_result r = ensure_builder(ts, f3);
    if (r == HG_OK && (FAILED(((file_ref_fn)SLOT(f3, SLOT_FACTORY_CREATE_FILE_REF))(f3, path, NULL, &file)) ||
                       FAILED(((add_file_fn)SLOT(ts->file_builder, SLOT_BUILDER1_ADD_FILE))(ts->file_builder, file))))
        r = hg__fail(HG_ERR_INVALID, "fonte de arquivo: não abriu ou não é fonte");
    if (r == HG_OK && (FAILED(((out_fn)SLOT(ts->file_builder, SLOT_BUILDER_CREATE_SET))(ts->file_builder, &set)) ||
                       FAILED(((from_set_fn)SLOT(f3, SLOT_FACTORY3_COLLECTION_FROM_SET))(f3, set, &coll))))
        r = hg__fail(HG_ERR_DEVICE, "fonte de arquivo: coleção falhou");
    if (r == HG_OK) {
        RELEASE(ts->file_collection);
        ts->file_collection = coll;
        coll = NULL;
        /* Formatos já criados apontam para a coleção antiga. */
        for (uint32_t i = 0; i < ts->font_count; i++)
            for (int k = 0; k < HG_FORMAT_CACHE; k++) DW_RELEASE(ts->fonts[i].formats[k]);
    }
    RELEASE(coll);
    RELEASE(set);
    RELEASE(file);
    RELEASE(f3);
    return r;
}

hg_result hg_font_add_file(hg_context* ctx, const char* path_utf8) {
    if (!ctx || !path_utf8) return hg__fail(HG_ERR_INVALID, "hg_font_add_file: argumento nulo");
    hg_text_system* ts;
    hg_result r = hg__text_system_get(ctx, &ts);
    if (r != HG_OK) return r;
    WCHAR* path = wide(path_utf8);
    if (!path) return hg__fail(HG_ERR_INVALID, "hg_font_add_file: caminho inválido");
    r = add_file(ts, path);
    free(path);
    return r;
}

hg_result hg_font_family_exists(hg_context* ctx, const char* family_utf8, int* out) {
    if (!ctx || !family_utf8 || !out) return hg__fail(HG_ERR_INVALID, "hg_font_family_exists: argumento nulo");
    hg_text_system* ts;
    hg_result r = hg__text_system_get(ctx, &ts);
    if (r != HG_OK) return r;
    WCHAR* family = wide(family_utf8);
    if (!family) return hg__fail(HG_ERR_INVALID, "hg_font_family_exists: nome inválido");
    IUnknown* sys = NULL;
    BOOL found = has_family(ts->file_collection, family);
    if (!found && SUCCEEDED(((get_system_fn)SLOT(ts->factory, SLOT_FACTORY_GET_SYSTEM_COLLECTION))(ts->factory, &sys, FALSE)))
        found = has_family(sys, family);
    RELEASE(sys);
    free(family);
    *out = found ? 1 : 0;
    return HG_OK;
}

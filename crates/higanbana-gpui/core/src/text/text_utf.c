/* text_utf.c — conversão UTF-8 -> UTF-16 com validação estrita. */
#include "text.h"
#include "error.h"
#include <limits.h>

hg_result hg__utf8_to_utf16(const char* utf8, size_t len, WCHAR* out, int out_cap, int* out_len) {
    if (len == 0) { *out_len = 0; return HG_OK; }
    if (len > INT_MAX) return hg__fail(HG_ERR_INVALID, "texto grande demais");
    int n = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, utf8, (int)len, out, out ? out_cap : 0);
    if (n <= 0) return hg__fail(HG_ERR_INVALID, "texto não é UTF-8 válido");
    *out_len = n;
    return HG_OK;
}

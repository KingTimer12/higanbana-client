#include "error.h"
#include <string.h>

static __declspec(thread) char g_last_error[256];

hg_result hg__fail(hg_result code, const char* message) {
    strncpy_s(g_last_error, sizeof g_last_error, message ? message : "", _TRUNCATE);
    return code;
}

const char* hg_last_error(void) { return g_last_error; }

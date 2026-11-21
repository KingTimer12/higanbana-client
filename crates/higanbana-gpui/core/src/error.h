/* error.h — erro thread-local compartilhado pelo core. */
#ifndef HG_ERROR_H
#define HG_ERROR_H
#include "hgui.h"

/* Grava a mensagem e devolve o código, para `return hg__fail(...)`. */
hg_result hg__fail(hg_result code, const char* message);
#endif

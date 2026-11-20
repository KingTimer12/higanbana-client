/* opengl_backend.c — STUB: substituído pela implementação real do backend. */
#include "backend.h"

static int opengl_probe(hg_backend_info* info) { (void)info; return 0; }

const hg_backend_vtbl hg__opengl_backend = {
    .kind = HG_BACKEND_OPENGL,
    .name = "OpenGL",
    .probe = opengl_probe,
};

/* gl_loader.c — carrega opengl32.dll e resolve WGL/GL em runtime (sem opengl32.lib). */
#include "gl_api.h"
#include <stdint.h>
#include <string.h>

#define LOAD(lib, type, name) (type)(void*)GetProcAddress(lib, name)

int gl_wgl_open(gl_wgl* w) {
    memset(w, 0, sizeof *w);
    w->lib = LoadLibraryW(L"opengl32.dll");
    if (!w->lib) return 0;
    w->create_context = LOAD(w->lib, hg_wgl_create_context_fn, "wglCreateContext");
    w->delete_context = LOAD(w->lib, hg_wgl_delete_context_fn, "wglDeleteContext");
    w->make_current = LOAD(w->lib, hg_wgl_make_current_fn, "wglMakeCurrent");
    w->get_proc = LOAD(w->lib, hg_wgl_get_proc_fn, "wglGetProcAddress");
    w->get_current_context = LOAD(w->lib, hg_wgl_get_current_context_fn, "wglGetCurrentContext");
    w->get_current_dc = LOAD(w->lib, hg_wgl_get_current_dc_fn, "wglGetCurrentDC");
    if (w->create_context && w->delete_context && w->make_current && w->get_proc &&
        w->get_current_context && w->get_current_dc)
        return 1;
    gl_wgl_close(w);
    return 0;
}

void gl_wgl_close(gl_wgl* w) {
    if (w->lib) FreeLibrary(w->lib);
    memset(w, 0, sizeof *w);
}

/* GL 1.1 vem exportado pela DLL; o resto, de wglGetProcAddress (que devolve
 * 0, 1, 2, 3 ou -1 quando não conhece a função). */
static void* resolve(const gl_wgl* w, const char* name) {
    void* p = (void*)GetProcAddress(w->lib, name);
    if (p) return p;
    p = (void*)w->get_proc(name);
    intptr_t v = (intptr_t)p;
    return (v >= -1 && v <= 3) ? NULL : p;
}

int gl_api_load(gl_wgl* w, gl_api* api) {
    int ok = 1;
#define HG_GL_LOAD(name, ret, args) \
    api->name = (hg_gl_##name##_fn)resolve(w, "gl" #name); \
    ok &= api->name != NULL;
    HG_GL_FUNCS(HG_GL_LOAD)
#undef HG_GL_LOAD
    w->swap_interval = (hg_wgl_swap_interval_fn)resolve(w, "wglSwapIntervalEXT");
    return ok;
}

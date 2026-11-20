/* gl_context.c — janela oculta, pixel format e contexto WGL 3.3 core
 * (contexto legado temporário só para obter wglCreateContextAttribsARB). */
#include "gl_internal.h"
#include "error.h"
#include <string.h>

static const wchar_t k_hidden_class[] = L"hg_gl_hidden";

static HWND create_hidden_window(void) {
    HINSTANCE inst = GetModuleHandleW(NULL);
    WNDCLASSEXW wc = { 0 };
    wc.cbSize = sizeof wc;
    wc.style = CS_OWNDC;
    wc.lpfnWndProc = DefWindowProcW;
    wc.hInstance = inst;
    wc.lpszClassName = k_hidden_class;
    if (!RegisterClassExW(&wc) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return NULL;
    return CreateWindowExW(0, k_hidden_class, L"", WS_POPUP, 0, 0, 1, 1, NULL, NULL, inst, NULL);
}

static int choose_format(gl_wgl_context* c) {
    PIXELFORMATDESCRIPTOR* pfd = &c->pfd;
    memset(pfd, 0, sizeof *pfd);
    pfd->nSize = sizeof *pfd;
    pfd->nVersion = 1;
    pfd->dwFlags = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
    pfd->iPixelType = PFD_TYPE_RGBA;
    pfd->cColorBits = 32;
    pfd->cAlphaBits = 8;
    pfd->iLayerType = PFD_MAIN_PLANE;
    c->pixel_format = ChoosePixelFormat(c->hidden_dc, pfd);
    if (!c->pixel_format) return 0;
    DescribePixelFormat(c->hidden_dc, c->pixel_format, sizeof *pfd, pfd);
    return SetPixelFormat(c->hidden_dc, c->pixel_format, pfd);
}

/* Contexto legado → wglCreateContextAttribsARB → contexto 3.3 core. */
static int create_core_rc(gl_wgl_context* c) {
    HGLRC legacy = c->wgl.create_context(c->hidden_dc);
    if (!legacy || !c->wgl.make_current(c->hidden_dc, legacy)) {
        if (legacy) c->wgl.delete_context(legacy);
        return 0;
    }
    hg_wgl_create_attribs_fn create_attribs =
        (hg_wgl_create_attribs_fn)(void*)c->wgl.get_proc("wglCreateContextAttribsARB");
    static const int attribs[] = {
        WGL_CONTEXT_MAJOR_VERSION_ARB, 3, WGL_CONTEXT_MINOR_VERSION_ARB, 3,
        WGL_CONTEXT_PROFILE_MASK_ARB, WGL_CONTEXT_CORE_PROFILE_BIT_ARB, 0,
    };
    c->rc = create_attribs ? create_attribs(c->hidden_dc, NULL, attribs) : NULL;
    c->wgl.make_current(NULL, NULL);
    c->wgl.delete_context(legacy);
    return c->rc && c->wgl.make_current(c->hidden_dc, c->rc);
}

hg_result gl_context_create(gl_wgl_context* c, hg_backend_info* info) {
    memset(c, 0, sizeof *c);
    if (!gl_wgl_open(&c->wgl)) return hg__fail(HG_ERR_UNAVAILABLE, "opengl: opengl32.dll ausente");
    hg_result r = HG_OK;
    c->hidden = create_hidden_window();
    c->hidden_dc = c->hidden ? GetDC(c->hidden) : NULL;
    if (!c->hidden_dc || !choose_format(c))
        r = hg__fail(HG_ERR_DEVICE, "opengl: pixel format indisponível");
    else if (!create_core_rc(c))
        r = hg__fail(HG_ERR_UNAVAILABLE, "opengl: contexto 3.3 core indisponível");
    else if (!gl_api_load(&c->wgl, &c->gl))
        r = hg__fail(HG_ERR_UNAVAILABLE, "opengl: funções GL 3.3 ausentes");
    if (r != HG_OK) {
        gl_context_destroy(c);
        return r;
    }
    const char* renderer = (const char*)c->gl.GetString(GL_RENDERER);
    if (info && renderer) strncpy_s(info->adapter, sizeof info->adapter, renderer, _TRUNCATE);
    return HG_OK;
}

void gl_context_destroy(gl_wgl_context* c) {
    if (c->rc) {
        if (c->wgl.get_current_context() == c->rc) c->wgl.make_current(NULL, NULL);
        c->wgl.delete_context(c->rc);
    }
    if (c->hidden_dc) ReleaseDC(c->hidden, c->hidden_dc);
    if (c->hidden) DestroyWindow(c->hidden);
    gl_wgl_close(&c->wgl);
    memset(c, 0, sizeof *c);
}

hg_result gl_context_bind(gl_wgl_context* c, HDC dc) {
    if (c->wgl.get_current_context() == c->rc && c->wgl.get_current_dc() == dc) return HG_OK;
    if (!c->wgl.make_current(dc, c->rc)) return hg__fail(HG_ERR_DEVICE, "opengl: wglMakeCurrent falhou");
    return HG_OK;
}

/* video_gl.c — lado OpenGL do alvo de vídeo: contexto 3.3 escondido (o mesmo do backend
 * OpenGL), WGL_NV_DX_interop2 no device D3D11 da UI e um FBO para o mpv desenhar. */
#include "gl_internal.h"
#include "video_target.h"
#include "error.h"
#include <stdlib.h>

#define HG_GL_FRAMEBUFFER          0x8D40
#define HG_GL_COLOR_ATTACHMENT0    0x8CE0
#define HG_GL_FRAMEBUFFER_COMPLETE 0x8CD5
#define HG_GL_TEXTURE_2D           0x0DE1
#define HG_WGL_ACCESS_WRITE_DISCARD_NV 0x0002

typedef HANDLE(WINAPI* dx_open_fn)(void*);
typedef BOOL(WINAPI* dx_close_fn)(HANDLE);
typedef HANDLE(WINAPI* dx_register_fn)(HANDLE, void*, GLuint, GLenum, GLenum);
typedef BOOL(WINAPI* dx_unregister_fn)(HANDLE, HANDLE);
typedef BOOL(WINAPI* dx_lock_fn)(HANDLE, GLint, HANDLE*);
typedef void(APIENTRY* gen_fn)(GLsizei, GLuint*);
typedef void(APIENTRY* del_fn)(GLsizei, const GLuint*);
typedef void(APIENTRY* bind_fb_fn)(GLenum, GLuint);
typedef void(APIENTRY* fb_tex_fn)(GLenum, GLenum, GLenum, GLuint, GLint);
typedef GLenum(APIENTRY* check_fb_fn)(GLenum);

struct video_gl {
    gl_wgl_context   c;
    HANDLE           dev;
    dx_open_fn       open;
    dx_close_fn      close;
    dx_register_fn   reg;
    dx_unregister_fn unreg;
    dx_lock_fn       lock;
    dx_lock_fn       unlock;
    gen_fn           gen_fb;
    del_fn           del_fb;
    bind_fb_fn       bind_fb;
    fb_tex_fn        fb_tex;
    check_fb_fn      check_fb;
    GLuint           fbo;
};

void* video_gl_proc(video_gl* g, const char* name) {
    return gl_resolve(&g->c.wgl, name);
}

#define PROC(field, type, name) (g->field = (type)video_gl_proc(g, name))

hg_result video_gl_create(void* d3d_device, video_gl** out) {
    video_gl* g = calloc(1, sizeof *g);
    if (!g) return hg__fail(HG_ERR_OOM, "alvo de vídeo: sem memória");
    hg_result r = gl_context_create(&g->c, NULL);
    if (r != HG_OK) {
        free(g);
        return r;
    }
    int ok = PROC(open, dx_open_fn, "wglDXOpenDeviceNV") && PROC(close, dx_close_fn, "wglDXCloseDeviceNV") &&
             PROC(reg, dx_register_fn, "wglDXRegisterObjectNV") && PROC(unreg, dx_unregister_fn, "wglDXUnregisterObjectNV") &&
             PROC(lock, dx_lock_fn, "wglDXLockObjectsNV") && PROC(unlock, dx_lock_fn, "wglDXUnlockObjectsNV");
    if (!ok) {
        video_gl_destroy(g);
        return hg__fail(HG_ERR_UNAVAILABLE, "alvo de vídeo: sem WGL_NV_DX_interop");
    }
    ok = PROC(gen_fb, gen_fn, "glGenFramebuffers") && PROC(del_fb, del_fn, "glDeleteFramebuffers") &&
         PROC(bind_fb, bind_fb_fn, "glBindFramebuffer") && PROC(fb_tex, fb_tex_fn, "glFramebufferTexture2D") &&
         PROC(check_fb, check_fb_fn, "glCheckFramebufferStatus");
    g->dev = ok ? g->open(d3d_device) : NULL;
    if (!g->dev) {
        video_gl_destroy(g);
        return hg__fail(HG_ERR_UNAVAILABLE, "alvo de vídeo: wglDXOpenDeviceNV recusou o device D3D11");
    }
    g->gen_fb(1, &g->fbo);
    *out = g;
    return HG_OK;
}

hg_result video_gl_register(video_gl* g, void* d3d_tex, uint32_t* gl_tex, void** handle) {
    GLuint name = 0;
    g->c.gl.GenTextures(1, &name);
    HANDLE h = g->reg(g->dev, d3d_tex, name, HG_GL_TEXTURE_2D, HG_WGL_ACCESS_WRITE_DISCARD_NV);
    if (!h) {
        g->c.gl.DeleteTextures(1, &name);
        return hg__fail(HG_ERR_DEVICE, "alvo de vídeo: wglDXRegisterObjectNV falhou");
    }
    *gl_tex = name;
    *handle = h;
    return HG_OK;
}

void video_gl_unregister(video_gl* g, uint32_t gl_tex, void* handle) {
    if (handle) g->unreg(g->dev, handle);
    if (gl_tex) g->c.gl.DeleteTextures(1, &gl_tex);
}

hg_result video_gl_bind(video_gl* g, void* handle, uint32_t gl_tex, uint32_t* fbo) {
    HANDLE h = handle;
    if (!g->lock(g->dev, 1, &h)) return hg__fail(HG_ERR_DEVICE, "alvo de vídeo: wglDXLockObjectsNV falhou");
    g->bind_fb(HG_GL_FRAMEBUFFER, g->fbo);
    g->fb_tex(HG_GL_FRAMEBUFFER, HG_GL_COLOR_ATTACHMENT0, HG_GL_TEXTURE_2D, gl_tex, 0);
    if (g->check_fb(HG_GL_FRAMEBUFFER) != HG_GL_FRAMEBUFFER_COMPLETE) {
        g->bind_fb(HG_GL_FRAMEBUFFER, 0);
        g->unlock(g->dev, 1, &h);
        return hg__fail(HG_ERR_DEVICE, "alvo de vídeo: FBO incompleto");
    }
    *fbo = g->fbo;
    return HG_OK;
}

hg_result video_gl_unbind(video_gl* g, void* handle) {
    HANDLE h = handle;
    g->bind_fb(HG_GL_FRAMEBUFFER, 0);
    return g->unlock(g->dev, 1, &h) ? HG_OK : hg__fail(HG_ERR_DEVICE, "alvo de vídeo: wglDXUnlockObjectsNV falhou");
}

void video_gl_destroy(video_gl* g) {
    if (!g) return;
    if (g->fbo) g->del_fb(1, &g->fbo);
    if (g->dev) g->close(g->dev);
    gl_context_destroy(&g->c);
    free(g);
}

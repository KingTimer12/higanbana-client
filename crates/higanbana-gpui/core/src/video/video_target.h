/* video_target.h — alvo de vídeo: GL (thread de render) escreve em texturas D3D11 do
 * device da UI via WGL_NV_DX_interop2; a UI mostra a última publicada numa hg_image externa.
 * Tipos GL/D3D ficam como void* aqui: cada lado (video_gl.c / video_d3d.c) inclui só o seu. */
#ifndef HG_VIDEO_TARGET_H
#define HG_VIDEO_TARGET_H
#include "hgui.h"
#include <windows.h>

#define HG_VIDEO_SLOTS 2

typedef struct video_gl video_gl;

typedef struct video_slot {
    void*    tex;    /* ID3D11Texture2D* */
    void*    srv;    /* ID3D11ShaderResourceView* */
    uint32_t gl_tex; /* nome GL registrado na interop */
    void*    handle; /* HANDLE da interop */
    uint32_t w, h;
} video_slot;

struct hg_video_target {
    hg_context*      ctx;
    void*            device;  /* ID3D11Device* (AddRef), multithread protegido */
    hg_image*        image;   /* externa; só a thread da UI mexe */
    video_gl*        gl;      /* só a thread de render mexe */
    video_slot       slots[HG_VIDEO_SLOTS];
    int              writing; /* slot do quadro em curso; -1 fora de quadro */
    CRITICAL_SECTION lock;    /* protege pub_* e ui_slot entre as threads */
    void*            pub_tex; /* último publicado (AddRef) */
    void*            pub_srv;
    int              pub_slot; /* slot do publicado; -1 nenhum */
    int              ui_slot;  /* slot que a UI está mostrando; -1 nenhum */
    uint64_t         pub_seq, seen_seq;
};

/* video_d3d.c */
/* Device D3D11 do contexto (AddRef) com proteção multithread ligada. UNAVAILABLE se não é D3D11. */
hg_result video_d3d_device(hg_context* ctx, void** dev);
hg_result video_d3d_texture(void* dev, uint32_t w, uint32_t h, void** tex, void** srv);
void      video_d3d_addref(void* unk);
void      video_d3d_release(void* unk);
/* 1 se o contexto ainda está no D3D11 e com o mesmo device do alvo. */
int       video_d3d_same_device(hg_context* ctx, void* dev);
/* A imagem externa passa a mostrar tex/srv. */
hg_result video_d3d_wrap(hg_image* img, void* tex, void* srv);

/* video_gl.c */
hg_result video_gl_create(void* d3d_device, video_gl** out);
void*     video_gl_proc(video_gl* g, const char* name);
hg_result video_gl_register(video_gl* g, void* d3d_tex, uint32_t* gl_tex, void** handle);
void      video_gl_unregister(video_gl* g, uint32_t gl_tex, void* handle);
/* Trava a textura na interop e liga o FBO com ela como cor. */
hg_result video_gl_bind(video_gl* g, void* handle, uint32_t gl_tex, uint32_t* fbo);
hg_result video_gl_unbind(video_gl* g, void* handle);
void      video_gl_destroy(video_gl* g);
#endif

/* video_target.c — API do alvo de vídeo: slots alternados na thread de render, publicação
 * do último quadro e troca da textura da imagem externa na thread da UI. */
#include "video_target.h"
#include "error.h"
#include "image.h"
#include <stdlib.h>

hg_result hg_video_target_create(hg_context* ctx, hg_video_target** out) {
    if (!ctx || !out) return hg__fail(HG_ERR_INVALID, "hg_video_target_create: argumento nulo");
    *out = NULL;
    void* dev = NULL;
    hg_result r = video_d3d_device(ctx, &dev);
    if (r != HG_OK) return r;
    hg_video_target* t = calloc(1, sizeof *t);
    if (!t) {
        video_d3d_release(dev);
        return hg__fail(HG_ERR_OOM, "alvo de vídeo: sem memória");
    }
    t->ctx = ctx;
    t->device = dev;
    t->writing = -1;
    t->pub_slot = -1;
    t->ui_slot = -1;
    InitializeCriticalSection(&t->lock);
    *out = t;
    return HG_OK;
}


void* hg_video_target_gl_proc(hg_video_target* t, const char* name) {
    return t && t->gl && name ? video_gl_proc(t->gl, name) : NULL;
}

static hg_result slot_ready(hg_video_target* t, video_slot* s, uint32_t w, uint32_t h);
static void slot_free(hg_video_target* t, video_slot* s);

hg_result hg_video_target_gl_init(hg_video_target* t) {
    if (!t) return hg__fail(HG_ERR_INVALID, "hg_video_target_gl_init: alvo nulo");
    if (t->gl) return HG_OK;
    hg_result r = video_gl_create(t->device, &t->gl);
    if (r != HG_OK) return r;
    /* Sonda: registrar e travar uma textura agora, para a falha (ex.: GL e D3D11 em GPUs
     * diferentes) aparecer antes de o mpv abrir a saída de vídeo no GL. */
    video_slot probe = { 0 };
    uint32_t fbo = 0;
    r = slot_ready(t, &probe, 16, 16);
    if (r == HG_OK) r = video_gl_bind(t->gl, probe.handle, probe.gl_tex, &fbo);
    if (r == HG_OK) r = video_gl_unbind(t->gl, probe.handle);
    slot_free(t, &probe);
    if (r != HG_OK) {
        video_gl_destroy(t->gl);
        t->gl = NULL;
    }
    return r;
}

static void slot_free(hg_video_target* t, video_slot* s) {
    video_gl_unregister(t->gl, s->gl_tex, s->handle);
    video_d3d_release(s->srv);
    video_d3d_release(s->tex);
    *s = (video_slot){ 0 };
}

/* Garante a textura do slot no tamanho pedido (recria se mudou). */
static hg_result slot_ready(hg_video_target* t, video_slot* s, uint32_t w, uint32_t h) {
    if (s->tex && s->w == w && s->h == h) return HG_OK;
    if (s->tex) slot_free(t, s);
    hg_result r = video_d3d_texture(t->device, w, h, &s->tex, &s->srv);
    if (r == HG_OK) r = video_gl_register(t->gl, s->tex, &s->gl_tex, &s->handle);
    if (r != HG_OK) {
        slot_free(t, s);
        return r;
    }
    s->w = w;
    s->h = h;
    return HG_OK;
}

hg_result hg_video_target_begin(hg_video_target* t, uint32_t width, uint32_t height, uint32_t* fbo) {
    if (!t || !t->gl || !fbo || width == 0 || height == 0 || t->writing >= 0)
        return hg__fail(HG_ERR_INVALID, "hg_video_target_begin: argumento inválido");
    /* Três slots: nunca o que a UI mostra (a interop descarta o conteúdo ao travar) nem
     * o publicado ainda não visto. Retomar o publicado deixava a UI sem quadro: o mpv
     * passa quase todo o tempo dentro do quadro e ela quase nunca o achava publicado. */
    EnterCriticalSection(&t->lock);
    int slot = 0;
    while (slot == t->ui_slot || slot == t->pub_slot) slot++;
    LeaveCriticalSection(&t->lock);
    video_slot* s = &t->slots[slot];
    hg_result r = slot_ready(t, s, width, height);
    if (r == HG_OK) r = video_gl_bind(t->gl, s->handle, s->gl_tex, fbo);
    if (r == HG_OK) t->writing = slot;
    return r;
}

hg_result hg_video_target_end(hg_video_target* t) {
    if (!t || t->writing < 0) return hg__fail(HG_ERR_INVALID, "hg_video_target_end: sem quadro");
    video_slot* s = &t->slots[t->writing];
    hg_result r = video_gl_unbind(t->gl, s->handle);
    if (r == HG_OK) {
        video_d3d_addref(s->tex);
        video_d3d_addref(s->srv);
        EnterCriticalSection(&t->lock);
        void *old_tex = t->pub_tex, *old_srv = t->pub_srv;
        t->pub_tex = s->tex;
        t->pub_srv = s->srv;
        t->pub_slot = t->writing;
        t->pub_seq++;
        LeaveCriticalSection(&t->lock);
        video_d3d_release(old_srv);
        video_d3d_release(old_tex);
    }
    t->writing = -1;
    return r;
}

void hg_video_target_gl_release(hg_video_target* t) {
    if (!t || !t->gl) return;
    for (int i = 0; i < HG_VIDEO_SLOTS; i++) slot_free(t, &t->slots[i]);
    video_gl_destroy(t->gl);
    t->gl = NULL;
}

hg_result hg_video_target_image(hg_video_target* t, hg_image** out, int* fresh) {
    if (!t || !out) return hg__fail(HG_ERR_INVALID, "hg_video_target_image: argumento nulo");
    void *tex = NULL, *srv = NULL;
    EnterCriticalSection(&t->lock);
    if (t->pub_seq != t->seen_seq && t->pub_tex) {
        tex = t->pub_tex;
        srv = t->pub_srv;
        video_d3d_addref(tex);
        video_d3d_addref(srv);
        t->seen_seq = t->pub_seq;
        t->ui_slot = t->pub_slot;
    }
    LeaveCriticalSection(&t->lock);
    /* Backend trocado (mesmo sem quadro novo): a textura é de um device que não vale mais. */
    hg_result r = HG_OK;
    if (!video_d3d_same_device(t->ctx, t->device) || (t->image && !t->image->gpu && !tex))
        r = hg__fail(HG_ERR_UNAVAILABLE, "alvo de vídeo: backend trocado");
    if (r != HG_OK) {
        video_d3d_release(srv);
        video_d3d_release(tex);
        tex = srv = NULL;
    } else if (tex) {
        if (!t->image) r = hg__image_create_external(t->ctx, &t->image);
        if (r == HG_OK) r = video_d3d_wrap(t->image, tex, srv);
        video_d3d_release(srv);
        video_d3d_release(tex);
    }
    if (fresh) *fresh = tex && r == HG_OK;
    *out = t->image && t->image->gpu ? t->image : NULL;
    return r;
}

void hg_video_target_destroy(hg_video_target* t) {
    if (!t) return;
    hg_video_target_gl_release(t); /* no-op se a thread de render já soltou */
    if (t->image) hg_image_destroy(t->image);
    video_d3d_release(t->pub_srv);
    video_d3d_release(t->pub_tex);
    video_d3d_release(t->device);
    DeleteCriticalSection(&t->lock);
    free(t);
}

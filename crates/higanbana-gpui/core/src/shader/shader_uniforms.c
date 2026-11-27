/* shader_uniforms.c — monta o uniform de 320 bytes do shader do usuário (layout em prelude.wgsl). */
#include "shader.h"
#include <string.h>

void hg__shader_uniforms(const hg_shader_draw* d, float viewport_w, float viewport_h, float y_sign,
                         float out[HG_SHADER_UNIFORM_FLOATS]) {
    /* offset 0: rect */
    out[0] = d->rect.x;
    out[1] = d->rect.y;
    out[2] = d->rect.w;
    out[3] = d->rect.h;
    /* offset 16: cantos tl, tr, br, bl */
    memcpy(out + 4, d->radius, sizeof d->radius);
    /* offset 32: tempo, escala, opacidade, flags (como float) */
    out[8] = d->time;
    out[9] = d->scale;
    out[10] = d->opacity;
    out[11] = (float)d->flags;
    /* offset 48: viewport (largura, altura, 0, sinal do Y do backdrop) */
    out[12] = viewport_w;
    out[13] = viewport_h;
    out[14] = 0.0f;
    out[15] = y_sign;
    /* offset 64: params[16] */
    memcpy(out + 16, d->params, sizeof d->params);
}

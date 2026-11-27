/* gl_user_shader.c — shaders do usuário: GLSL 330 do naga (glsl_vs/glsl_fs) compilado no 1º uso,
 * desenhados como um quad (strip de 4 vértices via gl_VertexID, VAO vazio). Mapa fixo de
 * translate.rs: os blocos HgShaderUniforms_block_0Vertex/_0Fragment vão ao binding point 0
 * e o sampler _group_0_binding_1_fs (backdrop) à unidade 1, com filtro linear/clamp na textura. */
#include "gl_internal.h"
#include "draw_list.h"
#include "error.h"
#include "shader.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define UNIFORM_BINDING 0
#define BACKDROP_UNIT   1

typedef struct gl_user_shader {
    GLuint prog;
} gl_user_shader;

/* UBO, textura vazia e VAO: criados uma vez, no 1º shader do contexto. */
static hg_result ensure_common(gl_device* d) {
    gl_user_shaders* u = &d->user_shaders;
    if (u->vao) return HG_OK;
    const gl_api* gl = &d->ctx.gl;
    gl->GenBuffers(1, &u->ubo);
    gl->GenTextures(1, &u->empty);
    gl->GenVertexArrays(1, &u->vao);
    if (!u->ubo || !u->empty || !u->vao) {
        gl_user_shaders_release(gl, u);
        return hg__fail(HG_ERR_DEVICE, "opengl: falha ao criar estado dos shaders do usuário");
    }
    gl->BindBuffer(GL_UNIFORM_BUFFER, u->ubo);
    gl->BufferData(GL_UNIFORM_BUFFER, HG_SHADER_UNIFORM_BYTES, NULL, GL_STREAM_DRAW);
    gl->BindBuffer(GL_UNIFORM_BUFFER, 0);
    const uint32_t transparent = 0;
    gl->ActiveTexture(GL_TEXTURE0 + BACKDROP_UNIT); /* não mexe no atlas da unidade 0 */
    gl->BindTexture(GL_TEXTURE_2D, u->empty);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    gl->TexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, 1, 1, 0, GL_RGBA, GL_UNSIGNED_BYTE, &transparent);
    gl->ActiveTexture(GL_TEXTURE0);
    return HG_OK;
}

/* Grava "opengl: <estágio>: <log do driver>" no shader. */
static hg_result fail_with_log(hg_shader* sh, const char* stage, const char* log) {
    char msg[2048];
    snprintf(msg, sizeof msg, "opengl: %s: %s", stage, log && log[0] ? log : "sem mensagem do driver");
    return hg__shader_set_error(sh, msg);
}

static hg_result compile(const gl_api* gl, hg_shader* sh, GLenum kind, const char* src, GLuint* out) {
    const char* stage = kind == GL_VERTEX_SHADER ? "vertex" : "fragment";
    if (!src) return fail_with_log(sh, stage, "shader sem fonte GLSL");
    *out = gl->CreateShader(kind);
    if (!*out) return fail_with_log(sh, stage, "glCreateShader falhou");
    gl->ShaderSource(*out, 1, &src, NULL);
    gl->CompileShader(*out);
    GLint ok = 0;
    gl->GetShaderiv(*out, GL_COMPILE_STATUS, &ok);
    if (ok) return HG_OK;
    char log[1536] = { 0 };
    gl->GetShaderInfoLog(*out, (GLsizei)sizeof log, NULL, log);
    gl->DeleteShader(*out);
    *out = 0;
    return fail_with_log(sh, stage, log);
}

static hg_result link(const gl_api* gl, hg_shader* sh, GLuint vs, GLuint fs, GLuint* out) {
    GLuint prog = gl->CreateProgram();
    if (!prog) return fail_with_log(sh, "link", "glCreateProgram falhou");
    gl->AttachShader(prog, vs);
    gl->AttachShader(prog, fs);
    gl->LinkProgram(prog);
    GLint ok = 0;
    gl->GetProgramiv(prog, GL_LINK_STATUS, &ok);
    if (!ok) {
        char log[1536] = { 0 };
        gl->GetProgramInfoLog(prog, (GLsizei)sizeof log, NULL, log);
        gl->DeleteProgram(prog);
        return fail_with_log(sh, "link", log);
    }
    *out = prog;
    return HG_OK;
}

/* Liga os nomes do naga aos bindings fixos. Itens otimizados pelo driver (sem uso) são ignorados. */
static void bind_names(const gl_api* gl, GLuint prog) {
    static const char* const blocks[] = { "HgShaderUniforms_block_0Vertex", "HgShaderUniforms_block_0Fragment" };
    static const char* const samplers[] = { "_group_0_binding_1_vs", "_group_0_binding_1_fs" };
    for (size_t i = 0; i < ARRAYSIZE(blocks); i++) {
        GLuint idx = gl->GetUniformBlockIndex(prog, blocks[i]);
        if (idx != GL_INVALID_INDEX) gl->UniformBlockBinding(prog, idx, UNIFORM_BINDING);
    }
    gl->UseProgram(prog);
    for (size_t i = 0; i < ARRAYSIZE(samplers); i++) {
        GLint loc = gl->GetUniformLocation(prog, samplers[i]);
        if (loc >= 0) gl->Uniform1i(loc, BACKDROP_UNIT);
    }
}

hg_result gl_user_shader_prepare(gl_device* d, hg_shader* sh) {
    hg_result r = gl_context_ensure_current(&d->ctx);
    if (r == HG_OK) r = ensure_common(d);
    if (r != HG_OK) return r;
    const gl_api* gl = &d->ctx.gl;
    GLuint vs = 0, fs = 0, prog = 0;
    r = compile(gl, sh, GL_VERTEX_SHADER, sh->glsl_vs, &vs);
    if (r == HG_OK) r = compile(gl, sh, GL_FRAGMENT_SHADER, sh->glsl_fs, &fs);
    if (r == HG_OK) r = link(gl, sh, vs, fs, &prog);
    if (vs) gl->DeleteShader(vs);
    if (fs) gl->DeleteShader(fs);
    if (r != HG_OK) return r;
    gl_user_shader* gpu = calloc(1, sizeof *gpu);
    if (!gpu) {
        gl->DeleteProgram(prog);
        return hg__fail(HG_ERR_OOM, "opengl: sem memória");
    }
    bind_names(gl, prog);
    gpu->prog = prog;
    sh->gpu = gpu;
    return HG_OK;
}

void gl_user_shader_release(gl_device* d, hg_shader* sh) {
    gl_user_shader* gpu = sh->gpu;
    if (!gpu) return;
    /* Sem contexto corrente o programa morre junto com o GLRC. */
    if (gl_context_ensure_current(&d->ctx) == HG_OK) d->ctx.gl.DeleteProgram(gpu->prog);
    free(gpu);
    sh->gpu = NULL;
}

int gl_user_shader_draw(gl_device* d, gl_surface* s, const hg_batch* b) {
    hg_shader* sh = b->shader;
    const hg_shader_draw* sd = &b->shader_draw;
    if (!(sd->rect.w > 0.0f) || !(sd->rect.h > 0.0f)) return 0;
    /* Falha de compilação: sh->error guardado, o quadro segue sem este lote. */
    if (hg__shader_gpu(sh) != HG_OK || !sh->gpu) return 0;
    const gl_user_shader* gpu = sh->gpu;
    const gl_user_shaders* u = &d->user_shaders;
    const gl_api* gl = &d->ctx.gl;

    GLuint backdrop = u->empty;
    if (sd->flags & HG_SHADER_BACKDROP) {
        GLuint copy = gl_backdrop_capture(d, s, &sd->rect);
        if (copy) backdrop = copy;
    }
    /* y_sign = -1: a textura de backdrop tem a origem embaixo (cópia do FBO). */
    float uniforms[HG_SHADER_UNIFORM_FLOATS];
    hg__shader_uniforms(sd, (float)s->width, (float)s->height, -1.0f, uniforms);
    gl->BindBuffer(GL_UNIFORM_BUFFER, u->ubo);
    gl->BufferData(GL_UNIFORM_BUFFER, HG_SHADER_UNIFORM_BYTES, uniforms, GL_STREAM_DRAW); /* órfão por desenho */
    gl->BindBufferBase(GL_UNIFORM_BUFFER, UNIFORM_BINDING, u->ubo);

    gl->ActiveTexture(GL_TEXTURE0 + BACKDROP_UNIT);
    gl->BindTexture(GL_TEXTURE_2D, backdrop);
    gl->ActiveTexture(GL_TEXTURE0); /* o resto do quadro usa a unidade 0 */
    gl->UseProgram(gpu->prog);
    gl->BindVertexArray(u->vao);
    gl->DrawArrays(GL_TRIANGLE_STRIP, 0, 4);
    return 1;
}

void gl_user_shaders_release(const gl_api* gl, gl_user_shaders* u) {
    if (u->vao) gl->DeleteVertexArrays(1, &u->vao);
    if (u->empty) gl->DeleteTextures(1, &u->empty);
    if (u->ubo) gl->DeleteBuffers(1, &u->ubo);
    u->vao = u->empty = u->ubo = 0;
}

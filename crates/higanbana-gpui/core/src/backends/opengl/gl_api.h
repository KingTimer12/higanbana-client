/* gl_api.h — tipos, constantes e tabela de funções do OpenGL 3.3 core + WGL.
 * O SDK do Windows só traz GL 1.1; o resto é declarado aqui e carregado em runtime. */
#ifndef HG_GL_API_H
#define HG_GL_API_H
#include <windows.h>
#include <GL/gl.h>
#include <stddef.h>

typedef char      GLchar;
typedef ptrdiff_t GLsizeiptr;
typedef ptrdiff_t GLintptr;

#define GL_ARRAY_BUFFER    0x8892
#define GL_STREAM_DRAW     0x88E0
#define GL_FRAGMENT_SHADER 0x8B30
#define GL_VERTEX_SHADER   0x8B31
#define GL_COMPILE_STATUS  0x8B81
#define GL_LINK_STATUS     0x8B82
#define GL_TEXTURE0        0x84C0
#define GL_CLAMP_TO_EDGE   0x812F
#define GL_R8              0x8229
#ifndef GL_BGRA
#define GL_BGRA            0x80E1
#endif

#define WGL_CONTEXT_MAJOR_VERSION_ARB    0x2091
#define WGL_CONTEXT_MINOR_VERSION_ARB    0x2092
#define WGL_CONTEXT_PROFILE_MASK_ARB     0x9126
#define WGL_CONTEXT_CORE_PROFILE_BIT_ARB 0x0001

/* ---- WGL (opengl32.dll) --------------------------------------------------- */
typedef HGLRC(WINAPI* hg_wgl_create_context_fn)(HDC);
typedef BOOL(WINAPI* hg_wgl_delete_context_fn)(HGLRC);
typedef BOOL(WINAPI* hg_wgl_make_current_fn)(HDC, HGLRC);
typedef PROC(WINAPI* hg_wgl_get_proc_fn)(LPCSTR);
typedef HGLRC(WINAPI* hg_wgl_get_current_context_fn)(void);
typedef HDC(WINAPI* hg_wgl_get_current_dc_fn)(void);
typedef HGLRC(WINAPI* hg_wgl_create_attribs_fn)(HDC, HGLRC, const int*);
typedef BOOL(WINAPI* hg_wgl_swap_interval_fn)(int);

typedef struct gl_wgl {
    HMODULE                        lib;
    hg_wgl_create_context_fn       create_context;
    hg_wgl_delete_context_fn       delete_context;
    hg_wgl_make_current_fn         make_current;
    hg_wgl_get_proc_fn             get_proc;
    hg_wgl_get_current_context_fn  get_current_context;
    hg_wgl_get_current_dc_fn       get_current_dc;
    hg_wgl_swap_interval_fn        swap_interval; /* opcional (WGL_EXT_swap_control) */
} gl_wgl;

/* ---- GL: X-macro (nome sem prefixo "gl", retorno, parâmetros) ------------- */
#define HG_GL_FUNCS(X) \
    X(Clear, void, (GLbitfield)) \
    X(ClearColor, void, (GLfloat, GLfloat, GLfloat, GLfloat)) \
    X(Viewport, void, (GLint, GLint, GLsizei, GLsizei)) \
    X(Scissor, void, (GLint, GLint, GLsizei, GLsizei)) \
    X(Enable, void, (GLenum)) \
    X(Disable, void, (GLenum)) \
    X(BlendFunc, void, (GLenum, GLenum)) \
    X(GetString, const GLubyte*, (GLenum)) \
    X(ReadPixels, void, (GLint, GLint, GLsizei, GLsizei, GLenum, GLenum, void*)) \
    X(PixelStorei, void, (GLenum, GLint)) \
    X(GenTextures, void, (GLsizei, GLuint*)) \
    X(DeleteTextures, void, (GLsizei, const GLuint*)) \
    X(BindTexture, void, (GLenum, GLuint)) \
    X(TexImage2D, void, (GLenum, GLint, GLint, GLsizei, GLsizei, GLint, GLenum, GLenum, const void*)) \
    X(TexSubImage2D, void, (GLenum, GLint, GLint, GLint, GLsizei, GLsizei, GLenum, GLenum, const void*)) \
    X(TexParameteri, void, (GLenum, GLenum, GLint)) \
    X(ActiveTexture, void, (GLenum)) \
    X(GenBuffers, void, (GLsizei, GLuint*)) \
    X(DeleteBuffers, void, (GLsizei, const GLuint*)) \
    X(BindBuffer, void, (GLenum, GLuint)) \
    X(BufferData, void, (GLenum, GLsizeiptr, const void*, GLenum)) \
    X(BufferSubData, void, (GLenum, GLintptr, GLsizeiptr, const void*)) \
    X(GenVertexArrays, void, (GLsizei, GLuint*)) \
    X(DeleteVertexArrays, void, (GLsizei, const GLuint*)) \
    X(BindVertexArray, void, (GLuint)) \
    X(EnableVertexAttribArray, void, (GLuint)) \
    X(VertexAttribPointer, void, (GLuint, GLint, GLenum, GLboolean, GLsizei, const void*)) \
    X(VertexAttribDivisor, void, (GLuint, GLuint)) \
    X(DrawArraysInstanced, void, (GLenum, GLint, GLsizei, GLsizei)) \
    X(CreateShader, GLuint, (GLenum)) \
    X(ShaderSource, void, (GLuint, GLsizei, const GLchar* const*, const GLint*)) \
    X(CompileShader, void, (GLuint)) \
    X(GetShaderiv, void, (GLuint, GLenum, GLint*)) \
    X(DeleteShader, void, (GLuint)) \
    X(CreateProgram, GLuint, (void)) \
    X(AttachShader, void, (GLuint, GLuint)) \
    X(LinkProgram, void, (GLuint)) \
    X(GetProgramiv, void, (GLuint, GLenum, GLint*)) \
    X(DeleteProgram, void, (GLuint)) \
    X(UseProgram, void, (GLuint)) \
    X(GetUniformLocation, GLint, (GLuint, const GLchar*)) \
    X(Uniform1i, void, (GLint, GLint)) \
    X(Uniform2f, void, (GLint, GLfloat, GLfloat))

#define HG_GL_TYPEDEF(name, ret, args) typedef ret(APIENTRY* hg_gl_##name##_fn) args;
HG_GL_FUNCS(HG_GL_TYPEDEF)
#undef HG_GL_TYPEDEF

/* Uma tabela por device: nada global, vários contextos convivem. */
typedef struct gl_api {
#define HG_GL_FIELD(name, ret, args) hg_gl_##name##_fn name;
    HG_GL_FUNCS(HG_GL_FIELD)
#undef HG_GL_FIELD
} gl_api;

/* Abre opengl32.dll e resolve as funções WGL básicas. 0 se indisponível. */
int  gl_wgl_open(gl_wgl* w);
void gl_wgl_close(gl_wgl* w);
/* Resolve a tabela GL (contexto precisa estar corrente). 0 se faltar alguma. */
int  gl_api_load(gl_wgl* w, gl_api* api);
/* Função GL/WGL pelo nome: exportada pela DLL (GL 1.1) ou de wglGetProcAddress. NULL se não houver. */
void* gl_resolve(const gl_wgl* w, const char* name);
#endif

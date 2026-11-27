/* gl_internal.h — tipos compartilhados entre os módulos do backend OpenGL 3.3. */
#ifndef HG_GL_INTERNAL_H
#define HG_GL_INTERNAL_H
#include "gl_api.h"
#include "window.h"
#include "glyph_frame.h"

/* ---- contexto WGL: janela oculta + GLRC 3.3 core ---------------------- */
typedef struct gl_wgl_context {
    gl_wgl                wgl;
    gl_api                gl;
    HWND                  hidden;
    HDC                   hidden_dc;
    HGLRC                 rc;
    int                   pixel_format;
    PIXELFORMATDESCRIPTOR pfd;
} gl_wgl_context;

/* Cria o contexto 3.3 core e o deixa corrente na DC oculta; adapter = GL_RENDERER. */
hg_result gl_context_create(gl_wgl_context* c, hg_backend_info* info);
void      gl_context_destroy(gl_wgl_context* c);
/* Torna `rc` corrente em `dc` (no-op se já estiver). */
hg_result gl_context_bind(gl_wgl_context* c, HDC dc);
/* Garante o GLRC corrente (na DC oculta se não estiver em nenhuma): create/destroy fora de um frame. */
hg_result gl_context_ensure_current(gl_wgl_context* c);

/* ---- buffers: instance VBO com orphaning, cresce em potência de 2 ----- */
typedef struct gl_buffer {
    GLuint   name;
    uint32_t capacity; /* bytes */
} gl_buffer;

/* Garante capacidade e órfã o conteúdo antigo (glBufferData NULL). Deixa ligado em GL_ARRAY_BUFFER. */
void gl_buffer_orphan(const gl_api* gl, gl_buffer* b, uint32_t bytes);
void gl_buffer_release(const gl_api* gl, gl_buffer* b);

/* ---- pipeline: programas GLSL + VAOs ---------------------------------- */
typedef struct gl_pipeline {
    GLuint quad_prog, glyph_prog;
    GLint  quad_viewport, glyph_viewport, glyph_atlas_size;
    GLuint quad_vao, glyph_vao;
} gl_pipeline;

/* Compila e liga um programa; 0 em falha. */
GLuint    gl_program_build(const gl_api* gl, const char* vs, const char* fs);
extern const char* const hg__gl_quad_vs;
extern const char* const hg__gl_quad_fs;
extern const char* const hg__gl_glyph_vs;
extern const char* const hg__gl_glyph_fs;

hg_result gl_pipeline_create(const gl_api* gl, gl_pipeline* p);
void      gl_pipeline_release(const gl_api* gl, gl_pipeline* p);
/* Aponta os atributos de instância a partir da instância `first` (GL 3.3 não tem base instance). */
void      gl_pipeline_point_quads(const gl_api* gl, const gl_pipeline* p, GLuint vbo, uint32_t first);
void      gl_pipeline_point_glyphs(const gl_api* gl, const gl_pipeline* p, GLuint vbo, uint32_t first);

/* ---- glyphs: atlas R8 + instâncias do frame --------------------------- */
typedef struct gl_glyphs {
    GLuint         atlas;
    gl_buffer      instances;
    hg_glyph_frame frame;
} gl_glyphs;

/* ---- imagens: programa/VAO sob demanda + instâncias do frame ---------- */
typedef struct gl_images {
    GLuint    prog, vao;
    GLint     viewport;
    gl_buffer instances;
    uint64_t  texture_bytes; /* soma das texturas RGBA8 vivas */
} gl_images;

extern const char* const hg__gl_image_vs;
extern const char* const hg__gl_image_fs;

/* ---- shaders do usuário: estado comum criado no 1º prepare ------------ */
typedef struct gl_user_shaders {
    GLuint ubo;   /* uniform de HG_SHADER_UNIFORM_BYTES no binding point 0 */
    GLuint empty; /* unidade 1 quando não há backdrop: 1x1 transparente */
    GLuint vao;   /* vazio: o quad vem de gl_VertexID */
} gl_user_shaders;

/* ---- device: um por contexto ------------------------------------------ */
typedef struct gl_device {
    gl_wgl_context  ctx;
    gl_pipeline     pipeline;
    gl_buffer       quads;
    gl_glyphs       glyphs;
    gl_images       images;
    gl_user_shaders user_shaders;
} gl_device;

struct hg_image;
struct hg_batch;
/* Textura da imagem (img->gpu = GLuint*); tornam o contexto corrente se preciso. */
hg_result gl_texture_create(gl_device* d, struct hg_image* img);
hg_result gl_texture_update(gl_device* d, struct hg_image* img);
void      gl_texture_destroy(gl_device* d, struct hg_image* img);
/* Garante texturas e sobe as instâncias de imagem do frame. */
hg_result gl_images_prepare(gl_device* d, const hg_draw_list* dl);
void      gl_images_release(const gl_api* gl, gl_images* p);

hg_result gl_device_create(gl_device* d, hg_backend_info* info);
void      gl_device_release(gl_device* d);

hg_result gl_glyphs_prepare(gl_device* d, hg_context* ctx, const hg_draw_list* dl);
void      gl_glyphs_release(const gl_api* gl, gl_glyphs* g);
uint64_t  gl_gpu_bytes(const gl_device* d);

/* ---- surface: DC da janela com o pixel format do device --------------- */
/* Cópia do back buffer para shaders com HG_SHADER_BACKDROP (origem embaixo, como o FBO). */
typedef struct gl_backdrop {
    GLuint   tex;
    uint32_t width, height;
} gl_backdrop;

typedef struct gl_surface {
    HWND        hwnd;
    HDC         dc;
    uint32_t    width, height;
    int         interval; /* último swap interval aplicado; -1 = nenhum */
    gl_backdrop backdrop; /* criado no 1º shader com backdrop; morre com a surface/resize */
} gl_surface;

hg_result gl_surface_create(gl_device* d, HWND hwnd, uint32_t w, uint32_t h, gl_surface* s);
void      gl_surface_release(gl_device* d, gl_surface* s);
hg_result gl_surface_begin(gl_device* d, gl_surface* s, uint32_t w, uint32_t h, hg_color clear);
hg_result gl_surface_present(gl_device* d, gl_surface* s, int vsync);
hg_result gl_surface_read_pixel(gl_device* d, gl_surface* s, uint32_t x, uint32_t y, uint32_t* out);

/* Desenha as imagens do lote a partir da instância `start`; devolve nº de draws. */
uint32_t gl_images_draw(gl_device* d, const gl_surface* s, const struct hg_batch* b, uint32_t start);

/* ---- backdrop -------------------------------------------------------- */
/* Margem copiada em volta do retângulo (backdrop_blur do prelúdio limita o raio a 64 px). */
#define GL_BACKDROP_MARGIN 64.0f
/* Copia `r` + margem (recortado à superfície) do back buffer para a textura de backdrop.
 * 0 = nada a copiar (fora da superfície) ou falha. Contexto corrente na DC da surface. */
GLuint gl_backdrop_capture(gl_device* d, gl_surface* s, const hg_rect* r);
/* Contexto precisa estar corrente. */
void   gl_backdrop_release(const gl_api* gl, gl_backdrop* b);
/* Bytes da textura de backdrop (0 sem textura), para hg_stats.gpu_bytes. */
uint64_t gl_backdrop_bytes(const gl_backdrop* b);

/* ---- shaders do usuário ----------------------------------------------- */
struct hg_shader;
/* Compila glsl_vs/glsl_fs e cria sh->gpu; log do driver vai para sh->error. */
hg_result gl_user_shader_prepare(gl_device* d, struct hg_shader* sh);
void      gl_user_shader_release(gl_device* d, struct hg_shader* sh);
/* Desenha o lote de shader; 1 = desenhou (o estado comum precisa ser religado), 0 = pulado. */
int       gl_user_shader_draw(gl_device* d, gl_surface* s, const struct hg_batch* b);
void      gl_user_shaders_release(const gl_api* gl, gl_user_shaders* u);

/* ---- render: envia draw list ------------------------------------------ */
hg_result gl_render_draw_list(gl_device* d, hg_context* ctx, gl_surface* s, const hg_draw_list* dl);
#endif

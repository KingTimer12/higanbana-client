/* vk_internal.h — tipos compartilhados entre os módulos do backend Vulkan. */
#ifndef HG_VK_INTERNAL_H
#define HG_VK_INTERNAL_H
#include "vk_api.h"
#include "window.h"
#include "glyph_frame.h"

#define VK_FRAMES     2 /* frames em voo por janela */
#define VK_MAX_IMAGES 8

/* ---- buffers: host-visible coerente, mapeado, cresce em potência de 2 -- */
typedef struct vk_buffer {
    VkBuffer       buffer;
    VkDeviceMemory memory;
    void*          mapped;
    VkDeviceSize   capacity;
} vk_buffer;

/* ---- pipeline: render passes (limpa/carrega), layout e pipelines ------ */
typedef struct vk_pipeline {
    VkRenderPass          pass_clear, pass_load; /* compatíveis entre si */
    VkDescriptorSetLayout set_layout;
    VkPipelineLayout      layout;
    VkPipeline            quad, glyph, image;
} vk_pipeline;

/* ---- glyphs: atlas R8 (device local), descritor e uploads pendentes --- */
typedef struct vk_glyphs {
    VkImage            atlas;
    VkDeviceMemory     atlas_memory;
    VkImageView        atlas_view;
    VkSampler          sampler;
    VkDescriptorPool   pool;
    VkDescriptorSet    set;
    uint32_t           atlas_layout;  /* layout atual da imagem */
    hg_glyph_frame     frame;
    uint8_t*           pending;       /* R8 coletado no frame, copiado para o staging */
    size_t             pending_bytes, pending_capacity;
    VkBufferImageCopy* regions;
    uint32_t           region_count, region_capacity;
} vk_glyphs;

/* ---- imagens: sampler linear, texturas vivas e lixo à espera da GPU --- */
struct vk_texture; /* vk_texture.c */
typedef struct vk_images {
    VkSampler          sampler;       /* linear clamp, criado na primeira textura */
    struct vk_texture* garbage;       /* destruídas, liberadas quando a fence sinaliza */
    uint64_t           texture_bytes; /* RGBA8 residente */
    int                recording;     /* command buffers de frame gravando (não submetidos) */
} vk_images;

/* ---- shaders do usuário: estado comum, criado no 1º shader (vk_user_shader_common.c) */
struct vk_retired_pipeline; /* vk_user_shader.c */
typedef struct vk_user_shaders {
    VkDescriptorSetLayout set_layout;  /* 0: UBO dinâmico, 1: imagem, 2: sampler */
    VkPipelineLayout      layout;      /* set acima + o mesmo push constant dos pipelines comuns */
    VkSampler             sampler;     /* linear clamp */
    VkImage               empty;       /* 1x1 transparente (binding 1 sem backdrop) */
    VkDeviceMemory        empty_memory;
    VkImageView           empty_view;
    struct vk_retired_pipeline* garbage; /* pipelines soltos, à espera da GPU */
} vk_user_shaders;

/* ---- device: um por contexto ----------------------------------------- */
typedef struct vk_device {
    vk_api                           api;
    VkInstance                       instance;
    VkPhysicalDevice                 phys;
    VkDevice                         dev;
    VkQueue                          queue;
    uint32_t                         family;
    VkPhysicalDeviceMemoryProperties memory;
    VkCommandPool                    pool;
    vk_pipeline                      pipeline;
    vk_glyphs                        glyphs;
    vk_images                        images;
    vk_user_shaders                  user_shaders;
    vk_buffer                        readback; /* 1 pixel, para testes */
} vk_device;

/* ---- janela: swapchain + recursos de cada frame em voo ---------------- */
/* Uniforms e descritores dos shaders do usuário de um frame em voo (criados no 1º shader). */
#define VK_SHADER_UNIFORM_STRIDE 512u /* HG_SHADER_UNIFORM_BYTES alinhado a 256 (máximo do minUniformBufferOffsetAlignment) */
typedef struct vk_shader_frame {
    vk_buffer        uniforms;  /* VK_SHADER_UNIFORM_STRIDE por desenho */
    VkDescriptorPool pool;
    VkDescriptorSet  sets[2];   /* 0: textura vazia, 1: backdrop da superfície */
    /* O que está escrito nos sets (handles podem ser reusados: compara capacidade/geração). */
    VkDeviceSize     set_capacity; /* capacidade do buffer de uniforms; 0 = sets não escritos */
    uint32_t         set_backdrop; /* geração do backdrop no set 1; 0 = textura vazia */
    uint32_t         capacity, used; /* desenhos reservados / gravados neste frame */
} vk_shader_frame;

typedef struct vk_frame {
    VkCommandBuffer cmd;
    VkFence         fence;
    VkSemaphore     acquired;
    vk_buffer       quads, glyphs, staging, images;
    vk_shader_frame shaders;
} vk_frame;

/* Cópia do que já foi desenhado atrás de um shader com backdrop (uma por superfície). */
typedef struct vk_backdrop {
    VkImage        image;
    VkDeviceMemory memory;
    VkImageView    view;
    uint32_t       width, height;
    uint32_t       generation; /* muda a cada imagem criada (nunca 0 com imagem) */
    int            ready; /* já passou por uma cópia: layout SHADER_READ_ONLY */
} vk_backdrop;

typedef struct vk_surface {
    VkSurfaceKHR   surface;
    VkSwapchainKHR swapchain;
    uint32_t       width, height, image_count;
    uint32_t       req_w, req_h;  /* último tamanho pedido pela janela */
    VkImage        images[VK_MAX_IMAGES];
    VkImageView    views[VK_MAX_IMAGES];
    VkFramebuffer  framebuffers[VK_MAX_IMAGES];
    VkSemaphore    rendered[VK_MAX_IMAGES];
    int            vsync, dirty;  /* dirty: recriar swapchain no próximo frame */
    int            transfer_src;  /* imagens aceitam cópia (readback, backdrop) */
    vk_backdrop    backdrop;
    vk_frame       frames[VK_FRAMES];
    uint32_t       slot;          /* frame em voo atual */
    /* Estado do frame corrente. */
    int            active, recording, acquire_pending, cleared, rendered_signaled;
    uint32_t       image;
    VkClearValue   clear;
} vk_surface;

/* vk_instance.c — instância única do processo (nunca destruída; ver arquivo). 0 se indisponível. */
int       vk_shared_instance(vk_api* api, VkInstance* out);

/* vk_device.c — instância, escolha de GPU e device lógico. */
hg_result vk_instance_create(vk_api* api, VkInstance* out);
/* Prefere GPU discreta com fila gráfica que apresenta em Win32. 0 se nenhuma. */
int       vk_pick_physical(vk_api* api, VkInstance inst, VkPhysicalDevice* out, uint32_t* family, char* name, size_t name_size);
hg_result vk_device_create(vk_device* d, hg_backend_info* info);
void      vk_device_release(vk_device* d);

/* vk_memory.c */
int       vk_memory_type(const vk_device* d, uint32_t bits, VkFlags props, uint32_t* out);
hg_result vk_buffer_reserve(vk_device* d, vk_buffer* b, VkDeviceSize bytes, VkFlags usage);
void      vk_buffer_release(vk_device* d, vk_buffer* b);
uint64_t  vk_gpu_bytes(const vk_device* d, const vk_surface* s);

/* vk_pipeline.c */
typedef struct vk_pipeline_desc {
    VkShaderModule vs; const char* vs_entry;
    VkShaderModule fs; const char* fs_entry;
    uint32_t stride; /* 0 = sem vertex buffer */
    const VkVertexInputAttributeDescription* attrs;
    uint32_t attr_count;
    VkPipelineLayout layout;
} vk_pipeline_desc;
hg_result vk_pipeline_create(vk_device* d);
VkResult  vk_pipeline_build(vk_device* d, const vk_pipeline_desc* desc, VkPipeline* out);
void      vk_pipeline_release(vk_device* d);

/* vk_glyphs.c — grava cópias do atlas (fora do render pass) e sobe instâncias. */
hg_result vk_glyphs_prepare(vk_device* d, hg_context* ctx, vk_frame* f, const hg_draw_list* dl);
void      vk_glyphs_bind(vk_device* d, VkCommandBuffer cmd, vk_frame* f);
void      vk_glyphs_release(vk_device* d);

/* vk_texture.c — textura RGBA8 por hg_image (upload síncrono via staging) e
 * destruição adiada até a GPU terminar os frames que podem usá-la. */
hg_result vk_texture_create(vk_device* d, struct hg_image* img);
void      vk_texture_destroy(vk_device* d, struct hg_image* img);
VkDescriptorSet vk_texture_set(const struct hg_image* img);
void      vk_textures_collect(vk_device* d); /* libera o lixo já concluído pela GPU */
/* Fence de uma submissão vazia: sinaliza depois de todo o trabalho já submetido. 0 se falhar. */
VkFence   vk_submit_marker(vk_device* d);
void      vk_images_release(vk_device* d);  /* device ocioso: libera lixo e sampler */

struct hg_batch;

/* vk_images.c — instâncias de imagem do frame e draws agrupados por textura. */
hg_result vk_images_prepare(vk_device* d, vk_frame* f, const hg_draw_list* dl);
uint32_t  vk_images_draw(vk_device* d, VkCommandBuffer cmd, vk_frame* f, const struct hg_batch* b, uint32_t start);

struct hg_shader;

/* vk_user_shader_common.c — estado comum (layout, sampler, textura vazia) e uniforms/sets por frame. */
hg_result vk_user_shaders_common(vk_device* d);
/* Antes do render pass: reserva uniforms/sets para os lotes de shader. Nada se não houver nenhum. */
void      vk_shader_frame_prepare(vk_device* d, vk_surface* s, vk_frame* f, const hg_draw_list* dl);
void      vk_shader_frame_release(vk_device* d, vk_frame* f);
void      vk_user_shaders_release(vk_device* d); /* device ocioso */

/* vk_user_shader.c — pipeline por shader (SPIR-V do naga) e o desenho de um lote. */
hg_result vk_user_shader_prepare(vk_device* d, struct hg_shader* sh);
void      vk_user_shader_release(vk_device* d, struct hg_shader* sh);
/* Dentro do render pass, com o scissor do lote em sc. 1 se desenhou. */
int       vk_user_shader_draw(vk_device* d, vk_surface* s, vk_frame* f, const struct hg_batch* b, const VkRect2D* sc);
void      vk_user_shaders_collect(vk_device* d); /* libera pipelines que a GPU já terminou */
void      vk_retired_pipelines_release(vk_device* d); /* device ocioso */

/* vk_backdrop.c — imagem por superfície; a cópia encerra o render pass e o retoma com LOAD. */
int       vk_backdrop_ensure(vk_device* d, vk_surface* s);
int       vk_backdrop_capture(vk_device* d, vk_surface* s, VkCommandBuffer cmd, const hg_rect* r);
void      vk_backdrop_release(vk_device* d, vk_surface* s);

/* vk_swapchain.c */
hg_result vk_surface_create(vk_device* d, HWND hwnd, uint32_t w, uint32_t h, vk_surface* s);
/* Recria swapchain/framebuffers no tamanho pedido; w/h 0 = só marca sujo. */
hg_result vk_swapchain_build(vk_device* d, vk_surface* s, uint32_t w, uint32_t h);
void      vk_surface_release(vk_device* d, vk_surface* s);

/* vk_frame.c — aquisição, render pass, submissão e present. */
hg_result vk_frame_begin(vk_device* d, vk_surface* s, uint32_t w, uint32_t h, hg_color clear);
hg_result vk_frame_record(vk_device* d, vk_surface* s); /* garante command buffer gravando */
void      vk_frame_pass_begin(vk_device* d, vk_surface* s);
hg_result vk_frame_flush(vk_device* d, vk_surface* s, int signal_rendered);
hg_result vk_frame_end(vk_device* d, vk_surface* s, int vsync);
hg_result vk_frames_create(vk_device* d, vk_surface* s);
void      vk_frames_release(vk_device* d, vk_surface* s);

/* vk_render.c / vk_readback.c */
hg_result vk_render_draw_list(vk_device* d, hg_context* ctx, vk_surface* s, const hg_draw_list* dl);
hg_result vk_surface_read_pixel(vk_device* d, vk_surface* s, uint32_t x, uint32_t y, uint32_t* out);

#define VK_DESTROY(d, fn, h) do { if (h) { (d)->api.fn((d)->dev, (h), NULL); (h) = 0; } } while (0)
#endif

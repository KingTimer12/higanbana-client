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
    VkPipeline            quad, glyph;
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
    vk_buffer                        readback; /* 1 pixel, para testes */
} vk_device;

/* ---- janela: swapchain + recursos de cada frame em voo ---------------- */
typedef struct vk_frame {
    VkCommandBuffer cmd;
    VkFence         fence;
    VkSemaphore     acquired;
    vk_buffer       quads, glyphs, staging;
} vk_frame;

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
    vk_frame       frames[VK_FRAMES];
    uint32_t       slot;          /* frame em voo atual */
    /* Estado do frame corrente. */
    int            active, recording, acquire_pending, cleared, rendered_signaled;
    uint32_t       image;
    VkClearValue   clear;
} vk_surface;

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
hg_result vk_pipeline_create(vk_device* d);
void      vk_pipeline_release(vk_device* d);

/* vk_glyphs.c — grava cópias do atlas (fora do render pass) e sobe instâncias. */
hg_result vk_glyphs_prepare(vk_device* d, hg_context* ctx, vk_frame* f, const hg_draw_list* dl);
void      vk_glyphs_bind(vk_device* d, VkCommandBuffer cmd, vk_frame* f);
void      vk_glyphs_release(vk_device* d);

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

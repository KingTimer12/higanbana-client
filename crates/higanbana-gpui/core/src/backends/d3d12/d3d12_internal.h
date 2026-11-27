/* d3d12_internal.h — tipos compartilhados entre os módulos do backend D3D12. */
#ifndef HG_D3D12_INTERNAL_H
#define HG_D3D12_INTERNAL_H
#define COBJMACROS
#include "window.h"
#include "glyph_frame.h"
#include <d3d12.h>
#include <d3dcompiler.h>
#include <dxgi1_4.h>

#define D3D12_FRAMES 2 /* frames em voo */
#define D3D12_BUFFERS 2 /* backbuffers por swapchain */

/* ---- loader: d3d12.dll e dxgi.dll carregadas em runtime --------------- */
typedef HRESULT(WINAPI* hg_d3d12_create_device_fn)(IUnknown*, D3D_FEATURE_LEVEL, REFIID, void**);
typedef HRESULT(WINAPI* hg_d3d12_serialize_rs_fn)(const D3D12_ROOT_SIGNATURE_DESC*, D3D_ROOT_SIGNATURE_VERSION,
                                                   ID3DBlob**, ID3DBlob**);
typedef HRESULT(WINAPI* hg_dxgi_create_factory2_fn)(UINT, REFIID, void**);
typedef struct d3d12_loader {
    HMODULE                    d3d12, dxgi;
    hg_d3d12_create_device_fn  create_device;
    hg_d3d12_serialize_rs_fn   serialize_root_signature;
    hg_dxgi_create_factory2_fn create_factory2;
} d3d12_loader;

int  d3d12_loader_open(d3d12_loader* l);
void d3d12_loader_close(d3d12_loader* l);

/* d3dcompiler_47.dll: só carregada no 1º shader do usuário. */
typedef struct d3d12_compiler {
    HMODULE     lib;
    pD3DCompile compile;
} d3d12_compiler;

int  d3d12_compiler_open(d3d12_compiler* c);
void d3d12_compiler_close(d3d12_compiler* c);

/* ---- upload: anel por frame num buffer UPLOAD mapeado ----------------- */
#define D3D12_RETIRED_MAX 16
typedef struct d3d12_upload {
    ID3D12Resource* buffer;
    uint8_t*        cpu;
    UINT64          capacity, offset;
    ID3D12Resource* retired[D3D12_RETIRED_MAX]; /* trocados no meio do frame; soltos ao reusar o slot */
    UINT            retired_count;
} d3d12_upload;

typedef struct d3d12_alloc {
    uint8_t*                  cpu;
    D3D12_GPU_VIRTUAL_ADDRESS gpu;
    ID3D12Resource*           resource;
    UINT64                    offset; /* dentro de resource */
} d3d12_alloc;

hg_result d3d12_upload_alloc(ID3D12Device* dev, d3d12_upload* u, UINT64 bytes, UINT64 align, d3d12_alloc* out);
void      d3d12_upload_reset(d3d12_upload* u);
void      d3d12_upload_release(d3d12_upload* u);

/* ---- pipeline: root signature + PSOs de quad e glyph ------------------ */
typedef struct d3d12_pipeline {
    ID3D12RootSignature* root;
    ID3D12PipelineState* quad;
    ID3D12PipelineState* glyph;
    ID3D12RootSignature* image_root; /* igual a root, mas com sampler linear */
    ID3D12PipelineState* image;
} d3d12_pipeline;

hg_result d3d12_pipeline_create(ID3D12Device* dev, const d3d12_loader* l, d3d12_pipeline* p);
void      d3d12_pipeline_release(d3d12_pipeline* p);
/* PSO com o blend pré-multiplicado e o formato do swapchain (layout NULL = sem vertex buffer). */
HRESULT   d3d12_pso_create(ID3D12Device* dev, ID3D12RootSignature* root, D3D12_SHADER_BYTECODE vs,
                           D3D12_SHADER_BYTECODE ps, const D3D12_INPUT_ELEMENT_DESC* layout, UINT n,
                           ID3D12PipelineState** out);

/* ---- descritores: heap SRV visível ao shader, slot 0 = atlas -------- */
#define D3D12_SRV_MAX 1024 /* limite de texturas vivas (atlas + imagens, incl. aposentadas) */
typedef struct d3d12_srv_heap {
    ID3D12DescriptorHeap* heap;
    UINT                  stride;
    UINT                  free_list[D3D12_SRV_MAX];
    UINT                  free_count;
} d3d12_srv_heap;

hg_result d3d12_srv_create(ID3D12Device* dev, d3d12_srv_heap* h);
void      d3d12_srv_release(d3d12_srv_heap* h);
hg_result d3d12_srv_alloc(d3d12_srv_heap* h, UINT* index);
void      d3d12_srv_free(d3d12_srv_heap* h, UINT index);
D3D12_CPU_DESCRIPTOR_HANDLE d3d12_srv_cpu(const d3d12_srv_heap* h, UINT index);
D3D12_GPU_DESCRIPTOR_HANDLE d3d12_srv_gpu(const d3d12_srv_heap* h, UINT index);
/* SRV 2D BGRA8 de `tex` no slot `index`; tex NULL = descritor nulo (leituras dão 0, transparente). */
void      d3d12_srv_write_bgra(ID3D12Device* dev, const d3d12_srv_heap* h, UINT index, ID3D12Resource* tex);

/* ---- glyphs: atlas R8 (heap DEFAULT) + SRV no slot 0 ------------------ */
typedef struct d3d12_glyphs {
    ID3D12Resource*       atlas;
    int                   copying; /* atlas em COPY_DEST neste frame */
    hg_glyph_frame        frame;
    D3D12_GPU_VIRTUAL_ADDRESS instances; /* instâncias do frame no anel */
} d3d12_glyphs;

/* ---- imagens: texturas em hg_image.gpu; solturas adiadas até a fence -- */
typedef struct d3d12_retired_tex {
    IUnknown*       obj; /* textura ou PSO */
    UINT            srv; /* 0 = nenhum (o slot 0 é do atlas e nunca volta à lista) */
    UINT64          fence; /* livre quando a fence passar deste valor */
} d3d12_retired_tex;

typedef struct d3d12_texture { /* hg_image.gpu */
    ID3D12Resource* tex;
    UINT            srv; /* índice no heap de descritores */
} d3d12_texture;

typedef struct d3d12_images {
    d3d12_retired_tex* retired;
    UINT               retired_count, retired_cap;
    uint64_t           texture_bytes;
    D3D12_GPU_VIRTUAL_ADDRESS instances; /* instâncias do frame no anel */
    UINT               instance_bytes;
} d3d12_images;

/* ---- shaders do usuário: estado comum criado no 1º prepare -------------- */
typedef struct d3d12_user_shaders {
    d3d12_compiler       compiler;
    ID3D12RootSignature* root;      /* CBV raiz b0, tabela SRV t1, sampler estático s2 (linear, clamp) */
    UINT                 empty_srv; /* t1 sem backdrop: descritor nulo (0 = ainda não criado) */
} d3d12_user_shaders;

struct hg_image;
struct hg_batch;
struct hg_shader;

/* ---- device: um por contexto ------------------------------------------ */
typedef struct d3d12_device {
    d3d12_loader               loader;
    IDXGIFactory4*             factory;
    ID3D12Device*              dev;
    ID3D12CommandQueue*        queue;
    ID3D12CommandAllocator*    allocators[D3D12_FRAMES];
    ID3D12GraphicsCommandList* list;
    ID3D12Fence*               fence;
    HANDLE                     fence_event;
    UINT64                     fence_value, slot_values[D3D12_FRAMES];
    UINT                       slot;
    int                        recording; /* lista aberta (entre begin e end) */
    d3d12_upload               uploads[D3D12_FRAMES];
    d3d12_pipeline             pipeline;
    d3d12_glyphs               glyphs;
    d3d12_srv_heap             srvs;
    d3d12_images               images;
    float                      globals[4]; /* constantes b0 do submit atual */
    d3d12_user_shaders         user_shaders;
} d3d12_device;

hg_result d3d12_device_create(d3d12_device* d, hg_backend_info* info);
void      d3d12_device_release(d3d12_device* d);
/* Escreve a descrição (UTF-8) de um adaptador. */
void      d3d12_describe_adapter(IDXGIAdapter1* a, hg_backend_info* info);

/* ---- sync: fence, slots de frame e lista de comandos ------------------ */
void      d3d12_wait_idle(d3d12_device* d);
hg_result d3d12_list_open(d3d12_device* d);  /* espera o slot, reseta alocador/anel */
hg_result d3d12_list_execute(d3d12_device* d); /* fecha e envia; marca o slot com fence */
void      d3d12_barrier(ID3D12GraphicsCommandList* l, ID3D12Resource* r, D3D12_RESOURCE_STATES from,
                        D3D12_RESOURCE_STATES to);
static inline d3d12_upload* d3d12_ring(d3d12_device* d) { return &d->uploads[d->slot]; }

/* ---- backdrop: cópia do alvo para os shaders com HG_SHADER_BACKDROP ----- */
typedef struct d3d12_backdrop {
    ID3D12Resource* tex; /* tamanho da superfície; criada no 1º uso, fica em PIXEL_SHADER_RESOURCE */
    UINT            srv; /* 0 = sem SRV */
    UINT            width, height; /* da textura (0 sem textura) */
} d3d12_backdrop;

/* ---- swapchain: uma por janela ---------------------------------------- */
typedef struct d3d12_surface {
    IDXGISwapChain3*      swapchain;
    ID3D12DescriptorHeap* rtv_heap;
    ID3D12Resource*       buffers[D3D12_BUFFERS];
    UINT                  rtv_size;
    UINT                  width, height;
    d3d12_backdrop        backdrop;
} d3d12_surface;

hg_result d3d12_surface_create(d3d12_device* d, HWND hwnd, UINT w, UINT h, d3d12_surface* s);
hg_result d3d12_surface_resize(d3d12_device* d, d3d12_surface* s, UINT w, UINT h);
void      d3d12_surface_release(d3d12_device* d, d3d12_surface* s);
ID3D12Resource* d3d12_surface_current(d3d12_surface* s);
D3D12_CPU_DESCRIPTOR_HANDLE d3d12_surface_rtv(d3d12_surface* s);
hg_result d3d12_surface_read_pixel(d3d12_device* d, d3d12_surface* s, UINT x, UINT y, uint32_t* out);

/* ---- frame: começo/fim (barreiras, clear, present) -------------------- */
hg_result d3d12_frame_begin(d3d12_device* d, d3d12_surface* s, hg_color clear);
/* *occluded = 1 se o Present foi descartado (DXGI_STATUS_OCCLUDED). */
hg_result d3d12_frame_end(d3d12_device* d, d3d12_surface* s, int vsync, int* occluded);
void      d3d12_frame_bind_target(d3d12_device* d, d3d12_surface* s);

/* ---- glyphs e render -------------------------------------------------- */
hg_result d3d12_glyphs_prepare(d3d12_device* d, hg_context* ctx, const hg_draw_list* dl);
void      d3d12_glyphs_bind(d3d12_device* d);
void      d3d12_glyphs_release(d3d12_glyphs* g);
/* Volta à root signature principal (reaplica constantes e heap). */
void      d3d12_use_root(d3d12_device* d, ID3D12RootSignature* root);

/* ---- imagens ---------------------------------------------------------- */
hg_result d3d12_texture_create(d3d12_device* d, struct hg_image* img);
void      d3d12_texture_destroy(d3d12_device* d, struct hg_image* img);
/* Adia a soltura de `obj` (e do SRV, se != 0) até a GPU terminar o último uso possível. */
void      d3d12_retire(d3d12_device* d, IUnknown* obj, UINT srv);
/* Solta texturas aposentadas cuja fence já passou (all = todas; GPU ociosa). */
void      d3d12_textures_collect(d3d12_device* d, int all);
hg_result d3d12_images_prepare(d3d12_device* d, const hg_draw_list* dl);
UINT      d3d12_images_draw(d3d12_device* d, const struct hg_batch* b, UINT start);
void      d3d12_images_release(d3d12_device* d);
uint64_t  d3d12_gpu_bytes(const d3d12_device* d);
hg_result d3d12_render_draw_list(d3d12_device* d, hg_context* ctx, d3d12_surface* s, const hg_draw_list* dl);

/* ---- shaders do usuário e backdrop ------------------------------------ */
/* Margem copiada em volta do retângulo (backdrop_blur do prelúdio limita o raio a 64 px). */
#define D3D12_BACKDROP_MARGIN 64.0f
/* Grava a cópia de `r` + margem (recortada) do backbuffer para o backdrop; devolve o índice do SRV,
 * ou 0 = nada a copiar / falha (o chamador usa o descritor nulo). */
UINT      d3d12_backdrop_capture(d3d12_device* d, d3d12_surface* s, const hg_rect* r);
/* Só com a GPU ociosa (resize/release da superfície já esperam). */
void      d3d12_backdrop_release(d3d12_device* d, d3d12_backdrop* b);
/* Bytes da textura de backdrop (0 sem textura), para hg_stats.gpu_bytes. */
uint64_t  d3d12_backdrop_bytes(const d3d12_backdrop* b);

/* Compila o HLSL (vs_main/ps_main) e cria o PSO em sh->gpu; erro do compilador vai para sh->error. */
hg_result d3d12_user_shader_prepare(d3d12_device* d, struct hg_shader* sh);
void      d3d12_user_shader_release(d3d12_device* d, struct hg_shader* sh);
/* Grava o lote de shader; 1 = desenhou (o estado comum precisa ser religado), 0 = pulado. */
int       d3d12_user_shader_draw(d3d12_device* d, d3d12_surface* s, const struct hg_batch* b);
void      d3d12_user_shaders_release(d3d12_device* d);

#define D3D12_RELEASE(p) do { if (p) { IUnknown_Release((IUnknown*)(p)); (p) = NULL; } } while (0)
#endif

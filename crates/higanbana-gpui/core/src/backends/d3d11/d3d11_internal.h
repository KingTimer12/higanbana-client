/* d3d11_internal.h — tipos compartilhados entre os módulos do backend D3D11. */
#ifndef HG_D3D11_INTERNAL_H
#define HG_D3D11_INTERNAL_H
#define COBJMACROS
#include "window.h"
#include "glyph_frame.h"
#include <d3d11.h>
#include <dxgi1_2.h>

/* ---- loader: d3d11.dll carregada em runtime --------------------------- */
typedef HRESULT(WINAPI* hg_d3d11_create_device_fn)(IDXGIAdapter*, D3D_DRIVER_TYPE, HMODULE, UINT,
                                                   const D3D_FEATURE_LEVEL*, UINT, UINT, ID3D11Device**,
                                                   D3D_FEATURE_LEVEL*, ID3D11DeviceContext**);
typedef struct d3d11_loader {
    HMODULE                    lib;
    hg_d3d11_create_device_fn  create_device;
} d3d11_loader;

int  d3d11_loader_open(d3d11_loader* l);
void d3d11_loader_close(d3d11_loader* l);

/* ---- buffers: instance buffer dinâmico que só cresce ------------------ */
typedef struct d3d11_dynamic_buffer {
    ID3D11Buffer* buffer;
    UINT          capacity; /* bytes */
    UINT          bind;
} d3d11_dynamic_buffer;

hg_result d3d11_buffer_reserve(ID3D11Device* dev, d3d11_dynamic_buffer* b, UINT bytes);
void      d3d11_buffer_release(d3d11_dynamic_buffer* b);

/* ---- pipeline: shaders, input layout e estados fixos ------------------ */
typedef struct d3d11_pipeline {
    ID3D11VertexShader*      quad_vs;
    ID3D11PixelShader*       quad_ps;
    ID3D11InputLayout*       quad_layout;
    ID3D11BlendState*        blend;
    ID3D11RasterizerState*   raster;
    ID3D11Buffer*            globals; /* cbuffer: viewport */
} d3d11_pipeline;

hg_result d3d11_pipeline_create(ID3D11Device* dev, d3d11_pipeline* p);
void      d3d11_pipeline_release(d3d11_pipeline* p);

/* ---- glyphs: atlas R8 + pipeline + instâncias do frame ---------------- */
typedef struct d3d11_glyphs {
    ID3D11VertexShader*       vs;
    ID3D11PixelShader*        ps;
    ID3D11InputLayout*        layout;
    ID3D11SamplerState*       sampler;
    ID3D11Texture2D*          atlas;
    ID3D11ShaderResourceView* atlas_srv;
    d3d11_dynamic_buffer      instances_gpu;
    hg_glyph_frame            frame;
} d3d11_glyphs;

/* ---- device: um por contexto ------------------------------------------ */
typedef struct d3d11_device {
    d3d11_loader         loader;
    ID3D11Device*        dev;
    ID3D11DeviceContext* imm;
    IDXGIFactory2*       factory;
    d3d11_pipeline       pipeline;
    d3d11_dynamic_buffer quads;
    d3d11_glyphs         glyphs;
} d3d11_device;

/* Rasteriza/coleta os glyphs do frame e sobe as instâncias (pipeline criado sob demanda). */
hg_result d3d11_glyphs_prepare(d3d11_device* d, hg_context* ctx, const hg_draw_list* dl);
void      d3d11_glyphs_bind(d3d11_device* d);
void      d3d11_glyphs_release(d3d11_glyphs* g);
/* Memória de GPU estimada: buffers dinâmicos + atlas (swapchains não contam). */
uint64_t  d3d11_gpu_bytes(const d3d11_device* d);

hg_result d3d11_device_create(d3d11_device* d, hg_backend_info* info);
void      d3d11_device_release(d3d11_device* d);

/* ---- swapchain: uma por janela ---------------------------------------- */
typedef struct d3d11_surface {
    IDXGISwapChain1*        swapchain;
    ID3D11RenderTargetView* rtv;
    UINT                    width, height;
} d3d11_surface;

hg_result d3d11_surface_create(d3d11_device* d, HWND hwnd, UINT w, UINT h, d3d11_surface* s);
hg_result d3d11_surface_resize(d3d11_device* d, d3d11_surface* s, UINT w, UINT h);
hg_result d3d11_surface_present(d3d11_surface* s, int vsync);
void      d3d11_surface_release(d3d11_surface* s);
hg_result d3d11_surface_read_pixel(d3d11_device* d, d3d11_surface* s, UINT x, UINT y, uint32_t* out);

/* ---- render: envia draw list ------------------------------------------ */
hg_result d3d11_render_draw_list(d3d11_device* d, hg_context* ctx, d3d11_surface* s, const hg_draw_list* dl);

#define D3D11_RELEASE(p) do { if (p) { IUnknown_Release((IUnknown*)(p)); (p) = NULL; } } while (0)
#endif

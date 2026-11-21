/* d3d11_buffers.c — buffer dinâmico reaproveitado entre frames; cresce em potência de 2. */
#include "d3d11_internal.h"
#include "error.h"
#include "text.h"

hg_result d3d11_buffer_reserve(ID3D11Device* dev, d3d11_dynamic_buffer* b, UINT bytes) {
    if (b->buffer && b->capacity >= bytes) return HG_OK;
    UINT cap = b->capacity ? b->capacity : 64 * 1024;
    while (cap < bytes) cap *= 2;
    D3D11_BUFFER_DESC desc = { 0 };
    desc.ByteWidth = cap;
    desc.Usage = D3D11_USAGE_DYNAMIC;
    desc.BindFlags = b->bind;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_WRITE;
    ID3D11Buffer* fresh = NULL;
    if (FAILED(ID3D11Device_CreateBuffer(dev, &desc, NULL, &fresh)))
        return hg__fail(HG_ERR_OOM, "d3d11: CreateBuffer falhou");
    D3D11_RELEASE(b->buffer);
    b->buffer = fresh;
    b->capacity = cap;
    return HG_OK;
}

void d3d11_buffer_release(d3d11_dynamic_buffer* b) {
    D3D11_RELEASE(b->buffer);
    b->capacity = 0;
}

uint64_t d3d11_gpu_bytes(const d3d11_device* d) {
    uint64_t bytes = (uint64_t)d->quads.capacity + d->glyphs.instances_gpu.capacity +
                     d->images.instances.capacity + d->images.texture_bytes;
    if (d->glyphs.atlas) bytes += (uint64_t)HG_GLYPH_ATLAS_SIZE * HG_GLYPH_ATLAS_SIZE; /* R8 */
    return bytes;
}

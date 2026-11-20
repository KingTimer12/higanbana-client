/* vk_memory.c — tipos de memória e buffers host-visible mapeados que só crescem. */
#include "vk_internal.h"
#include "error.h"
#include "text.h"

int vk_memory_type(const vk_device* d, uint32_t bits, VkFlags props, uint32_t* out) {
    for (uint32_t i = 0; i < d->memory.memoryTypeCount; i++)
        if ((bits & (1u << i)) && (d->memory.memoryTypes[i].propertyFlags & props) == props) {
            *out = i;
            return 1;
        }
    return 0;
}

hg_result vk_buffer_reserve(vk_device* d, vk_buffer* b, VkDeviceSize bytes, VkFlags usage) {
    if (b->buffer && b->capacity >= bytes) return HG_OK;
    VkDeviceSize cap = b->capacity ? b->capacity : 256;
    while (cap < bytes) cap *= 2;
    vk_buffer_release(d, b); /* chamador garante que a GPU não usa mais (fence do slot) */

    vk_api* a = &d->api;
    VkBufferCreateInfo ci = { VK_STYPE_BUFFER_CREATE_INFO, NULL, 0, cap, usage, 0, 0, NULL };
    VkMemoryRequirements req;
    uint32_t type;
    if (a->vkCreateBuffer(d->dev, &ci, NULL, &b->buffer) != VK_SUCCESS) return hg__fail(HG_ERR_OOM, "vulkan: vkCreateBuffer falhou");
    a->vkGetBufferMemoryRequirements(d->dev, b->buffer, &req);
    if (!vk_memory_type(d, req.memoryTypeBits, VK_MEMORY_HOST_VISIBLE | VK_MEMORY_HOST_COHERENT, &type)) {
        vk_buffer_release(d, b);
        return hg__fail(HG_ERR_DEVICE, "vulkan: sem memória host-visible coerente");
    }
    VkMemoryAllocateInfo ai = { VK_STYPE_MEMORY_ALLOCATE_INFO, NULL, req.size, type };
    if (a->vkAllocateMemory(d->dev, &ai, NULL, &b->memory) != VK_SUCCESS ||
        a->vkBindBufferMemory(d->dev, b->buffer, b->memory, 0) != VK_SUCCESS ||
        a->vkMapMemory(d->dev, b->memory, 0, VK_WHOLE_SIZE, 0, &b->mapped) != VK_SUCCESS) {
        vk_buffer_release(d, b);
        return hg__fail(HG_ERR_OOM, "vulkan: alocação de buffer falhou");
    }
    b->capacity = cap;
    return HG_OK;
}

void vk_buffer_release(vk_device* d, vk_buffer* b) {
    VK_DESTROY(d, vkDestroyBuffer, b->buffer);
    VK_DESTROY(d, vkFreeMemory, b->memory); /* libera também o mapeamento */
    b->mapped = NULL;
    b->capacity = 0;
}

uint64_t vk_gpu_bytes(const vk_device* d, const vk_surface* s) {
    uint64_t bytes = d->readback.capacity;
    if (d->glyphs.atlas) bytes += (uint64_t)HG_GLYPH_ATLAS_SIZE * HG_GLYPH_ATLAS_SIZE; /* R8 */
    for (int i = 0; i < VK_FRAMES; i++) {
        const vk_frame* f = &s->frames[i];
        bytes += f->quads.capacity + f->glyphs.capacity + f->staging.capacity;
    }
    return bytes;
}

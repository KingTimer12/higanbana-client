/* vk_readback.c — lê um pixel da imagem atual da swapchain (testes): copia 1x1
 * para um buffer host-visible, submete o que foi gravado e espera a fence. */
#include "vk_internal.h"
#include "error.h"

typedef struct VkMemoryBarrier { VK_HEADER; VkFlags srcAccessMask, dstAccessMask; } VkMemoryBarrier;
#define VK_STYPE_MEMORY_BARRIER 46

static void image_barrier(vk_device* d, VkCommandBuffer cmd, VkImage img, uint32_t from, uint32_t to,
                          VkFlags src_stage, VkFlags src_access, VkFlags dst_stage, VkFlags dst_access) {
    VkImageMemoryBarrier b = { VK_STYPE_IMAGE_MEMORY_BARRIER, NULL, src_access, dst_access, from, to,
                               VK_QUEUE_FAMILY_IGNORED, VK_QUEUE_FAMILY_IGNORED, img, { VK_ASPECT_COLOR, 0, 1, 0, 1 } };
    d->api.vkCmdPipelineBarrier(cmd, src_stage, dst_stage, 0, 0, NULL, 0, NULL, 1, &b);
}

hg_result vk_surface_read_pixel(vk_device* d, vk_surface* s, uint32_t x, uint32_t y, uint32_t* out) {
    if (!s->active) return hg__fail(HG_ERR_INVALID, "vulkan: read_pixel fora de um frame");
    if (x >= s->width || y >= s->height) return hg__fail(HG_ERR_INVALID, "vulkan: read_pixel fora da imagem");
    hg_result r = vk_frame_record(d, s);
    if (r != HG_OK) return r;
    vk_frame* f = &s->frames[s->slot];
    if (!s->cleared) { /* nada desenhado ainda: aplica o clear */
        vk_frame_pass_begin(d, s);
        d->api.vkCmdEndRenderPass(f->cmd);
    }
    VkImage img = s->images[s->image];
    image_barrier(d, f->cmd, img, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
                  VK_STAGE_COLOR_OUTPUT, VK_ACCESS_COLOR_WRITE, VK_STAGE_TRANSFER, VK_ACCESS_TRANSFER_READ);
    VkBufferImageCopy c = { 0, 0, 0, { VK_ASPECT_COLOR, 0, 0, 1 }, { (int32_t)x, (int32_t)y, 0 }, { 1, 1, 1 } };
    d->api.vkCmdCopyImageToBuffer(f->cmd, img, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, d->readback.buffer, 1, &c);
    image_barrier(d, f->cmd, img, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR,
                  VK_STAGE_TRANSFER, 0, VK_STAGE_BOTTOM_OF_PIPE, 0);
    VkMemoryBarrier host = { VK_STYPE_MEMORY_BARRIER, NULL, VK_ACCESS_TRANSFER_WRITE, VK_ACCESS_HOST_READ };
    d->api.vkCmdPipelineBarrier(f->cmd, VK_STAGE_TRANSFER, VK_STAGE_HOST, 0, 1, &host, 0, NULL, 0, NULL);

    /* Sem sinalizar "renderizado": o present espera a fence, não o semáforo. */
    r = vk_frame_flush(d, s, 0);
    if (r != HG_OK) return r;
    if (d->api.vkWaitForFences(d->dev, 1, &f->fence, VK_TRUE, ~0ull) != VK_SUCCESS)
        return hg__fail(HG_ERR_DEVICE, "vulkan: espera do readback falhou");
    *out = *(const uint32_t*)d->readback.mapped; /* B8G8R8A8: B no byte baixo */
    return HG_OK;
}

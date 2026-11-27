/* vk_backdrop.c — imagem por superfície com o que já foi desenhado atrás de um shader.
 * Só existe depois do 1º shader com HG_SHADER_BACKDROP; cada desenho copia apenas o
 * retângulo dele + margem (dois vidros sobrepostos veem um ao outro). A cópia acontece fora
 * do render pass: encerra o pass, barreiras, vkCmdCopyImage, barreiras e retoma com LOAD.
 * Uma imagem serve aos dois frames em voo: a barreira antes da cópia espera as leituras do
 * fragment shader já submetidas (escopo de ordem de submissão da fila). */
#include "vk_internal.h"
#include <math.h>

#define VK_BACKDROP_MARGIN 64.0f

static VkResult create_image(vk_device* d, vk_backdrop* b, uint32_t w, uint32_t h) {
    vk_api* a = &d->api;
    VkImageCreateInfo ci = { VK_STYPE_IMAGE_CREATE_INFO, NULL, 0, VK_IMAGE_TYPE_2D, VK_FORMAT_B8G8R8A8_UNORM, { w, h, 1 },
                             1, 1, VK_SAMPLE_COUNT_1, 0, VK_IMAGE_USAGE_TRANSFER_DST | VK_IMAGE_USAGE_SAMPLED, 0, 0, NULL,
                             VK_IMAGE_LAYOUT_UNDEFINED };
    VkResult r = a->vkCreateImage(d->dev, &ci, NULL, &b->image);
    if (r != VK_SUCCESS) return r;
    VkMemoryRequirements req;
    uint32_t type;
    a->vkGetImageMemoryRequirements(d->dev, b->image, &req);
    if (!vk_memory_type(d, req.memoryTypeBits, VK_MEMORY_DEVICE_LOCAL, &type) &&
        !vk_memory_type(d, req.memoryTypeBits, 0, &type))
        return -1;
    VkMemoryAllocateInfo mi = { VK_STYPE_MEMORY_ALLOCATE_INFO, NULL, req.size, type };
    r = a->vkAllocateMemory(d->dev, &mi, NULL, &b->memory);
    if (r == VK_SUCCESS) r = a->vkBindImageMemory(d->dev, b->image, b->memory, 0);
    if (r != VK_SUCCESS) return r;
    VkImageViewCreateInfo vi = { VK_STYPE_IMAGE_VIEW_CREATE_INFO, NULL, 0, b->image, VK_IMAGE_VIEW_TYPE_2D,
                                 VK_FORMAT_B8G8R8A8_UNORM, { 0, 0, 0, 0 }, { VK_ASPECT_COLOR, 0, 1, 0, 1 } };
    return a->vkCreateImageView(d->dev, &vi, NULL, &b->view);
}

/* Fora do render pass, antes de gravar o frame. 0 se a superfície não permite cópia ou faltou memória. */
int vk_backdrop_ensure(vk_device* d, vk_surface* s) {
    vk_backdrop* b = &s->backdrop;
    if (!s->transfer_src) return 0;
    if (b->view && b->width == s->width && b->height == s->height) return 1;
    if (b->view) { /* o build da swapchain já solta no resize; aqui é só defesa */
        d->api.vkDeviceWaitIdle(d->dev);
        vk_backdrop_release(d, s);
    }
    if (create_image(d, b, s->width, s->height) != VK_SUCCESS) {
        vk_backdrop_release(d, s);
        return 0;
    }
    b->width = s->width;
    b->height = s->height;
    b->generation = b->generation + 1 ? b->generation + 1 : 1;
    return 1;
}

static int32_t clamp_px(float v, uint32_t max) {
    if (!(v > 0.0f)) return 0; /* também pega NaN */
    if (v > (float)max) return (int32_t)max;
    return (int32_t)v;
}

static void barrier2(vk_device* d, VkCommandBuffer cmd, const VkImageMemoryBarrier b[2], VkFlags src, VkFlags dst) {
    d->api.vkCmdPipelineBarrier(cmd, src, dst, 0, 0, NULL, 0, NULL, 2, b);
}

int vk_backdrop_capture(vk_device* d, vk_surface* s, VkCommandBuffer cmd, const hg_rect* r) {
    vk_backdrop* b = &s->backdrop;
    if (!b->view || b->width != s->width || b->height != s->height) return 0;
    const float m = VK_BACKDROP_MARGIN;
    int32_t left = clamp_px(floorf(r->x - m), s->width);
    int32_t top = clamp_px(floorf(r->y - m), s->height);
    int32_t right = clamp_px(ceilf(r->x + r->w + m), s->width);
    int32_t bottom = clamp_px(ceilf(r->y + r->h + m), s->height);
    if (left >= right || top >= bottom) return 0; /* fora da superfície: nada a copiar */

    VkImage back = s->images[s->image];
    const VkImageSubresourceRange range = { VK_ASPECT_COLOR, 0, 1, 0, 1 };
    d->api.vkCmdEndRenderPass(cmd); /* finalLayout do pass: PRESENT_SRC */
    VkImageMemoryBarrier pre[2] = {
        { VK_STYPE_IMAGE_MEMORY_BARRIER, NULL, VK_ACCESS_COLOR_WRITE, VK_ACCESS_TRANSFER_READ,
          VK_IMAGE_LAYOUT_PRESENT_SRC_KHR, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, VK_QUEUE_FAMILY_IGNORED,
          VK_QUEUE_FAMILY_IGNORED, back, range },
        /* Conteúdo fora do retângulo não importa: a 1ª cópia descarta (UNDEFINED). */
        { VK_STYPE_IMAGE_MEMORY_BARRIER, NULL, 0, VK_ACCESS_TRANSFER_WRITE,
          b->ready ? VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL : VK_IMAGE_LAYOUT_UNDEFINED,
          VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_QUEUE_FAMILY_IGNORED, VK_QUEUE_FAMILY_IGNORED, b->image, range },
    };
    barrier2(d, cmd, pre, VK_STAGE_COLOR_OUTPUT | VK_STAGE_FRAGMENT_SHADER, VK_STAGE_TRANSFER);

    const VkImageSubresourceLayers layers = { VK_ASPECT_COLOR, 0, 0, 1 };
    VkImageCopy c = { layers, { left, top, 0 }, layers, { left, top, 0 },
                      { (uint32_t)(right - left), (uint32_t)(bottom - top), 1 } };
    d->api.vkCmdCopyImage(cmd, back, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, b->image, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                          1, &c);

    VkImageMemoryBarrier post[2] = {
        { VK_STYPE_IMAGE_MEMORY_BARRIER, NULL, 0, VK_ACCESS_COLOR_READ | VK_ACCESS_COLOR_WRITE,
          VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR, VK_QUEUE_FAMILY_IGNORED,
          VK_QUEUE_FAMILY_IGNORED, back, range },
        { VK_STYPE_IMAGE_MEMORY_BARRIER, NULL, VK_ACCESS_TRANSFER_WRITE, VK_ACCESS_SHADER_READ,
          VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL, VK_QUEUE_FAMILY_IGNORED,
          VK_QUEUE_FAMILY_IGNORED, b->image, range },
    };
    barrier2(d, cmd, post, VK_STAGE_TRANSFER, VK_STAGE_COLOR_OUTPUT | VK_STAGE_FRAGMENT_SHADER);
    b->ready = 1;
    vk_frame_pass_begin(d, s); /* s->cleared = 1: pass_load (LOAD_OP_LOAD a partir de PRESENT_SRC) */
    return 1;
}

/* Só com a GPU ociosa para esta superfície (build da swapchain / release). */
void vk_backdrop_release(vk_device* d, vk_surface* s) {
    vk_backdrop* b = &s->backdrop;
    VK_DESTROY(d, vkDestroyImageView, b->view);
    VK_DESTROY(d, vkDestroyImage, b->image);
    VK_DESTROY(d, vkFreeMemory, b->memory);
    b->width = b->height = 0;
    b->ready = 0; /* generation fica: a próxima imagem ganha outro número */
}

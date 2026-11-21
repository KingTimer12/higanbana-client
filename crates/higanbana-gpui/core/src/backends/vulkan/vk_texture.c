/* vk_texture.c — textura RGBA8 device local por hg_image, com descriptor pool
 * próprio (1 set por pool: sem limite global de texturas, só memória). Upload
 * síncrono (staging + fence própria, uma vez por textura). Destruição adiada: a
 * textura vai para o lixo e só é liberada quando uma fence submetida depois dos
 * frames que podiam usá-la sinaliza. */
#include "vk_internal.h"
#include "error.h"
#include "image.h"
#include <stdlib.h>
#include <string.h>

typedef struct vk_texture {
    VkImage            image;
    VkDeviceMemory     memory;
    VkImageView        view;
    VkDescriptorPool   pool;
    VkDescriptorSet    set;
    VkFence            retire;  /* lixo: sinaliza quando a GPU não usa mais */
    uint64_t           bytes;
    struct vk_texture* next;
} vk_texture;

static void texture_free(vk_device* d, vk_texture* t) {
    VK_DESTROY(d, vkDestroyFence, t->retire);
    VK_DESTROY(d, vkDestroyDescriptorPool, t->pool); /* libera o set */
    VK_DESTROY(d, vkDestroyImageView, t->view);
    VK_DESTROY(d, vkDestroyImage, t->image);
    VK_DESTROY(d, vkFreeMemory, t->memory);
    free(t);
}

static VkResult ensure_sampler(vk_device* d) {
    if (d->images.sampler) return VK_SUCCESS;
    VkSamplerCreateInfo si = { VK_STYPE_SAMPLER_CREATE_INFO, NULL, 0, VK_FILTER_LINEAR, VK_FILTER_LINEAR, 0,
                               VK_SAMPLER_ADDRESS_CLAMP_TO_EDGE, VK_SAMPLER_ADDRESS_CLAMP_TO_EDGE,
                               VK_SAMPLER_ADDRESS_CLAMP_TO_EDGE, 0, 0, 1.0f, 0, 0, 0, 0, 0, 0 };
    return d->api.vkCreateSampler(d->dev, &si, NULL, &d->images.sampler);
}

static VkResult create_image(vk_device* d, vk_texture* t, uint32_t w, uint32_t h) {
    vk_api* a = &d->api;
    VkImageCreateInfo ci = { VK_STYPE_IMAGE_CREATE_INFO, NULL, 0, VK_IMAGE_TYPE_2D, VK_FORMAT_R8G8B8A8_UNORM,
                             { w, h, 1 }, 1, 1, VK_SAMPLE_COUNT_1, 0,
                             VK_IMAGE_USAGE_TRANSFER_DST | VK_IMAGE_USAGE_SAMPLED, 0, 0, NULL, VK_IMAGE_LAYOUT_UNDEFINED };
    VkResult r = a->vkCreateImage(d->dev, &ci, NULL, &t->image);
    if (r != VK_SUCCESS) return r;
    VkMemoryRequirements req;
    uint32_t type;
    a->vkGetImageMemoryRequirements(d->dev, t->image, &req);
    if (!vk_memory_type(d, req.memoryTypeBits, VK_MEMORY_DEVICE_LOCAL, &type) &&
        !vk_memory_type(d, req.memoryTypeBits, 0, &type))
        return -1;
    VkMemoryAllocateInfo ai = { VK_STYPE_MEMORY_ALLOCATE_INFO, NULL, req.size, type };
    r = a->vkAllocateMemory(d->dev, &ai, NULL, &t->memory);
    if (r == VK_SUCCESS) r = a->vkBindImageMemory(d->dev, t->image, t->memory, 0);
    if (r != VK_SUCCESS) return r;
    VkImageViewCreateInfo vi = { VK_STYPE_IMAGE_VIEW_CREATE_INFO, NULL, 0, t->image, VK_IMAGE_VIEW_TYPE_2D,
                                 VK_FORMAT_R8G8B8A8_UNORM, { 0, 0, 0, 0 }, { VK_ASPECT_COLOR, 0, 1, 0, 1 } };
    return a->vkCreateImageView(d->dev, &vi, NULL, &t->view);
}

static VkResult create_set(vk_device* d, vk_texture* t) {
    vk_api* a = &d->api;
    VkDescriptorPoolSize sizes[2] = { { VK_DESCRIPTOR_SAMPLED_IMAGE, 1 }, { VK_DESCRIPTOR_SAMPLER, 1 } };
    VkDescriptorPoolCreateInfo pi = { VK_STYPE_DESCRIPTOR_POOL_CREATE_INFO, NULL, 0, 1, 2, sizes };
    VkResult r = a->vkCreateDescriptorPool(d->dev, &pi, NULL, &t->pool);
    if (r != VK_SUCCESS) return r;
    VkDescriptorSetAllocateInfo ai = { VK_STYPE_DESCRIPTOR_SET_ALLOCATE, NULL, t->pool, 1, &d->pipeline.set_layout };
    r = a->vkAllocateDescriptorSets(d->dev, &ai, &t->set);
    if (r != VK_SUCCESS) return r;
    VkDescriptorImageInfo img = { 0, t->view, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL };
    VkDescriptorImageInfo smp = { d->images.sampler, 0, 0 };
    VkWriteDescriptorSet w[2] = {
        { VK_STYPE_WRITE_DESCRIPTOR_SET, NULL, t->set, 0, 0, 1, VK_DESCRIPTOR_SAMPLED_IMAGE, &img, NULL, NULL },
        { VK_STYPE_WRITE_DESCRIPTOR_SET, NULL, t->set, 1, 0, 1, VK_DESCRIPTOR_SAMPLER, &smp, NULL, NULL },
    };
    a->vkUpdateDescriptorSets(d->dev, 2, w, 0, NULL);
    return VK_SUCCESS;
}

static void barrier(vk_device* d, VkCommandBuffer cmd, VkImage image, uint32_t from, uint32_t to) {
    int to_dst = to == VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;
    VkImageMemoryBarrier b = { VK_STYPE_IMAGE_MEMORY_BARRIER, NULL, to_dst ? 0 : VK_ACCESS_TRANSFER_WRITE,
                               to_dst ? VK_ACCESS_TRANSFER_WRITE : VK_ACCESS_SHADER_READ, from, to,
                               VK_QUEUE_FAMILY_IGNORED, VK_QUEUE_FAMILY_IGNORED, image, { VK_ASPECT_COLOR, 0, 1, 0, 1 } };
    d->api.vkCmdPipelineBarrier(cmd, to_dst ? VK_STAGE_TOP_OF_PIPE : VK_STAGE_TRANSFER,
                                to_dst ? VK_STAGE_TRANSFER : VK_STAGE_FRAGMENT_SHADER, 0, 0, NULL, 0, NULL, 1, &b);
}

/* Staging → imagem num command buffer avulso; espera só a própria fence. */
static VkResult upload(vk_device* d, vk_texture* t, const hg_image* img) {
    vk_api* a = &d->api;
    vk_buffer staging = { 0 };
    VkDeviceSize bytes = (VkDeviceSize)img->width * img->height * 4;
    if (vk_buffer_reserve(d, &staging, bytes, VK_BUFFER_USAGE_TRANSFER_SRC) != HG_OK) return -1;
    memcpy(staging.mapped, img->pixels, (size_t)bytes);
    VkCommandBuffer cmd = NULL;
    VkFence fence = 0;
    VkCommandBufferAllocateInfo ai = { VK_STYPE_COMMAND_BUFFER_ALLOCATE, NULL, d->pool, 0, 1 };
    VkFenceCreateInfo fci = { VK_STYPE_FENCE_CREATE_INFO, NULL, 0 };
    VkCommandBufferBeginInfo bi = { VK_STYPE_COMMAND_BUFFER_BEGIN_INFO, NULL, VK_CMD_BUFFER_ONE_TIME_SUBMIT, NULL };
    VkResult r = a->vkAllocateCommandBuffers(d->dev, &ai, &cmd);
    if (r == VK_SUCCESS) r = a->vkCreateFence(d->dev, &fci, NULL, &fence);
    if (r == VK_SUCCESS) r = a->vkBeginCommandBuffer(cmd, &bi);
    if (r == VK_SUCCESS) {
        VkBufferImageCopy c = { 0, 0, 0, { VK_ASPECT_COLOR, 0, 0, 1 }, { 0, 0, 0 }, { img->width, img->height, 1 } };
        barrier(d, cmd, t->image, VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL);
        a->vkCmdCopyBufferToImage(cmd, staging.buffer, t->image, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 1, &c);
        barrier(d, cmd, t->image, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL);
        r = a->vkEndCommandBuffer(cmd);
    }
    VkSubmitInfo si = { VK_STYPE_SUBMIT_INFO, NULL, 0, NULL, NULL, 1, &cmd, 0, NULL };
    if (r == VK_SUCCESS) r = a->vkQueueSubmit(d->queue, 1, &si, fence);
    if (r == VK_SUCCESS) r = a->vkWaitForFences(d->dev, 1, &fence, VK_TRUE, ~0ull);
    if (cmd) a->vkFreeCommandBuffers(d->dev, d->pool, 1, &cmd);
    VK_DESTROY(d, vkDestroyFence, fence);
    vk_buffer_release(d, &staging);
    return r;
}

hg_result vk_texture_create(vk_device* d, hg_image* img) {
    vk_textures_collect(d);
    vk_texture* t = calloc(1, sizeof *t);
    if (!t) return hg__fail(HG_ERR_OOM, "vulkan: sem memória para textura");
    VkResult r = ensure_sampler(d);
    if (r == VK_SUCCESS) r = create_image(d, t, img->width, img->height);
    if (r == VK_SUCCESS) r = create_set(d, t);
    if (r == VK_SUCCESS) r = upload(d, t, img);
    if (r != VK_SUCCESS) {
        texture_free(d, t);
        return hg__fail(HG_ERR_OOM, "vulkan: textura de imagem falhou");
    }
    t->bytes = (uint64_t)img->width * img->height * 4;
    d->images.texture_bytes += t->bytes;
    img->gpu = t;
    return HG_OK;
}

VkDescriptorSet vk_texture_set(const hg_image* img) { return ((const vk_texture*)img->gpu)->set; }

void vk_texture_destroy(vk_device* d, hg_image* img) {
    vk_texture* t = img->gpu;
    if (!t) return;
    img->gpu = NULL;
    d->images.texture_bytes -= t->bytes;
    t->next = d->images.garbage;
    d->images.garbage = t;
    vk_textures_collect(d);
}

/* Submissão vazia com fence: sinaliza depois de todo o trabalho já submetido. */
static VkFence submit_marker(vk_device* d) {
    VkFence f = 0;
    VkFenceCreateInfo fci = { VK_STYPE_FENCE_CREATE_INFO, NULL, 0 };
    if (d->api.vkCreateFence(d->dev, &fci, NULL, &f) != VK_SUCCESS) return 0;
    if (d->api.vkQueueSubmit(d->queue, 0, NULL, f) != VK_SUCCESS) VK_DESTROY(d, vkDestroyFence, f);
    return f;
}

void vk_textures_collect(vk_device* d) {
    for (vk_texture** p = &d->images.garbage; *p;) {
        vk_texture* t = *p;
        /* Com frame gravando (pode usar t), a marca só vale depois da submissão. */
        if (!t->retire && !d->images.recording) t->retire = submit_marker(d);
        if (t->retire && d->api.vkWaitForFences(d->dev, 1, &t->retire, VK_TRUE, 0) == VK_SUCCESS) {
            *p = t->next;
            texture_free(d, t);
        } else {
            p = &t->next;
        }
    }
}

/* Device ocioso (vkDeviceWaitIdle antes): libera todo o lixo e o sampler. */
void vk_images_release(vk_device* d) {
    while (d->images.garbage) {
        vk_texture* t = d->images.garbage;
        d->images.garbage = t->next;
        texture_free(d, t);
    }
    VK_DESTROY(d, vkDestroySampler, d->images.sampler);
    d->images.texture_bytes = 0;
}

/* vk_atlas.c — imagem R8 do atlas de glyphs (device local), sampler nearest e
 * descriptor set; transições de layout do atlas. */
#include "vk_atlas.h"
#include "error.h"
#include "text.h"

static VkResult create_image(vk_device* d, vk_glyphs* g) {
    vk_api* a = &d->api;
    VkImageCreateInfo ci = { VK_STYPE_IMAGE_CREATE_INFO, NULL, 0, VK_IMAGE_TYPE_2D, VK_FORMAT_R8_UNORM,
                             { HG_GLYPH_ATLAS_SIZE, HG_GLYPH_ATLAS_SIZE, 1 }, 1, 1, VK_SAMPLE_COUNT_1, 0,
                             VK_IMAGE_USAGE_TRANSFER_DST | VK_IMAGE_USAGE_SAMPLED, 0, 0, NULL, VK_IMAGE_LAYOUT_UNDEFINED };
    VkResult r = a->vkCreateImage(d->dev, &ci, NULL, &g->atlas);
    if (r != VK_SUCCESS) return r;
    VkMemoryRequirements req;
    uint32_t type;
    a->vkGetImageMemoryRequirements(d->dev, g->atlas, &req);
    if (!vk_memory_type(d, req.memoryTypeBits, VK_MEMORY_DEVICE_LOCAL, &type) &&
        !vk_memory_type(d, req.memoryTypeBits, 0, &type))
        return -1;
    VkMemoryAllocateInfo ai = { VK_STYPE_MEMORY_ALLOCATE_INFO, NULL, req.size, type };
    r = a->vkAllocateMemory(d->dev, &ai, NULL, &g->atlas_memory);
    if (r == VK_SUCCESS) r = a->vkBindImageMemory(d->dev, g->atlas, g->atlas_memory, 0);
    if (r != VK_SUCCESS) return r;
    VkImageViewCreateInfo vi = { VK_STYPE_IMAGE_VIEW_CREATE_INFO, NULL, 0, g->atlas, VK_IMAGE_VIEW_TYPE_2D,
                                 VK_FORMAT_R8_UNORM, { 0, 0, 0, 0 }, { VK_ASPECT_COLOR, 0, 1, 0, 1 } };
    return a->vkCreateImageView(d->dev, &vi, NULL, &g->atlas_view);
}

static VkResult create_descriptor(vk_device* d, vk_glyphs* g) {
    vk_api* a = &d->api;
    VkSamplerCreateInfo si = { VK_STYPE_SAMPLER_CREATE_INFO, NULL, 0, VK_FILTER_NEAREST, VK_FILTER_NEAREST, 0,
                               VK_SAMPLER_ADDRESS_CLAMP_TO_EDGE, VK_SAMPLER_ADDRESS_CLAMP_TO_EDGE,
                               VK_SAMPLER_ADDRESS_CLAMP_TO_EDGE, 0, 0, 1.0f, 0, 0, 0, 0, 0, 0 };
    VkResult r = a->vkCreateSampler(d->dev, &si, NULL, &g->sampler);
    if (r != VK_SUCCESS) return r;
    VkDescriptorPoolSize sizes[2] = { { VK_DESCRIPTOR_SAMPLED_IMAGE, 1 }, { VK_DESCRIPTOR_SAMPLER, 1 } };
    VkDescriptorPoolCreateInfo pi = { VK_STYPE_DESCRIPTOR_POOL_CREATE_INFO, NULL, 0, 1, 2, sizes };
    r = a->vkCreateDescriptorPool(d->dev, &pi, NULL, &g->pool);
    if (r != VK_SUCCESS) return r;
    VkDescriptorSetAllocateInfo ai = { VK_STYPE_DESCRIPTOR_SET_ALLOCATE, NULL, g->pool, 1, &d->pipeline.set_layout };
    r = a->vkAllocateDescriptorSets(d->dev, &ai, &g->set);
    if (r != VK_SUCCESS) return r;
    VkDescriptorImageInfo img = { 0, g->atlas_view, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL };
    VkDescriptorImageInfo smp = { g->sampler, 0, 0 };
    VkWriteDescriptorSet w[2] = {
        { VK_STYPE_WRITE_DESCRIPTOR_SET, NULL, g->set, 0, 0, 1, VK_DESCRIPTOR_SAMPLED_IMAGE, &img, NULL, NULL },
        { VK_STYPE_WRITE_DESCRIPTOR_SET, NULL, g->set, 1, 0, 1, VK_DESCRIPTOR_SAMPLER, &smp, NULL, NULL },
    };
    a->vkUpdateDescriptorSets(d->dev, 2, w, 0, NULL);
    return VK_SUCCESS;
}

hg_result vk_atlas_ensure(vk_device* d) {
    vk_glyphs* g = &d->glyphs;
    if (g->set) return HG_OK; /* último objeto criado: atlas completo */
    vk_atlas_release(d);
    VkResult r = create_image(d, g);
    if (r == VK_SUCCESS) r = create_descriptor(d, g);
    if (r != VK_SUCCESS) {
        vk_atlas_release(d);
        return hg__fail(HG_ERR_OOM, "vulkan: atlas de glyphs falhou");
    }
    g->atlas_layout = VK_IMAGE_LAYOUT_UNDEFINED;
    return HG_OK;
}

void vk_atlas_transition(vk_device* d, VkCommandBuffer cmd, uint32_t layout) {
    vk_glyphs* g = &d->glyphs;
    int to_dst = layout == VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;
    int from_transfer = g->atlas_layout == VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;
    VkImageMemoryBarrier b = { VK_STYPE_IMAGE_MEMORY_BARRIER, NULL, from_transfer ? VK_ACCESS_TRANSFER_WRITE : 0,
                               to_dst ? VK_ACCESS_TRANSFER_WRITE : VK_ACCESS_SHADER_READ, g->atlas_layout, layout,
                               VK_QUEUE_FAMILY_IGNORED, VK_QUEUE_FAMILY_IGNORED, g->atlas, { VK_ASPECT_COLOR, 0, 1, 0, 1 } };
    /* Antes de escrever: espera leituras de frames anteriores (WAR). */
    VkFlags src = from_transfer ? VK_STAGE_TRANSFER : g->atlas_layout ? VK_STAGE_FRAGMENT_SHADER : VK_STAGE_TOP_OF_PIPE;
    VkFlags dst = to_dst ? VK_STAGE_TRANSFER : VK_STAGE_FRAGMENT_SHADER;
    d->api.vkCmdPipelineBarrier(cmd, src, dst, 0, 0, NULL, 0, NULL, 1, &b);
    g->atlas_layout = layout;
}

void vk_atlas_release(vk_device* d) {
    vk_glyphs* g = &d->glyphs;
    VK_DESTROY(d, vkDestroyDescriptorPool, g->pool); /* libera o set */
    g->set = 0;
    VK_DESTROY(d, vkDestroySampler, g->sampler);
    VK_DESTROY(d, vkDestroyImageView, g->atlas_view);
    VK_DESTROY(d, vkDestroyImage, g->atlas);
    VK_DESTROY(d, vkFreeMemory, g->atlas_memory);
}

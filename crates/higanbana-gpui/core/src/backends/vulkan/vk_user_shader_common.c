/* vk_user_shader_common.c — estado dos shaders do usuário compartilhado no device (layout do
 * set 0 com UBO dinâmico em 0, imagem em 1 e sampler em 2; sampler linear/clamp; textura 1x1
 * transparente) e, por frame em voo, o buffer de uniforms e os dois descriptor sets. Nada disso
 * existe antes do 1º shader: um frame sem lote de shader não toca em nada aqui. */
#include "vk_internal.h"
#include "draw_list.h"
#include "error.h"
#include "shader.h"

static VkResult create_layouts(vk_device* d, vk_user_shaders* u) {
    const VkFlags stages = VK_SHADER_STAGE_VERTEX | VK_SHADER_STAGE_FRAGMENT;
    VkDescriptorSetLayoutBinding b[3] = {
        { 0, VK_DESCRIPTOR_UNIFORM_BUFFER_DYNAMIC, 1, stages, NULL },
        { 1, VK_DESCRIPTOR_SAMPLED_IMAGE, 1, stages, NULL },
        { 2, VK_DESCRIPTOR_SAMPLER, 1, stages, NULL },
    };
    VkDescriptorSetLayoutCreateInfo sci = { VK_STYPE_DESCRIPTOR_SET_LAYOUT_CREATE, NULL, 0, 3, b };
    VkResult r = d->api.vkCreateDescriptorSetLayout(d->dev, &sci, NULL, &u->set_layout);
    if (r != VK_SUCCESS) return r;
    /* Mesmo push constant dos pipelines comuns: o valor empurrado por bind_common segue compatível. */
    VkPushConstantRange push = { stages, 0, 16 };
    VkPipelineLayoutCreateInfo ci = { VK_STYPE_PIPELINE_LAYOUT_CREATE_INFO, NULL, 0, 1, &u->set_layout, 1, &push };
    return d->api.vkCreatePipelineLayout(d->dev, &ci, NULL, &u->layout);
}

static VkResult create_sampler(vk_device* d, vk_user_shaders* u) {
    VkSamplerCreateInfo si = { VK_STYPE_SAMPLER_CREATE_INFO, NULL, 0, VK_FILTER_LINEAR, VK_FILTER_LINEAR, 0,
                               VK_SAMPLER_ADDRESS_CLAMP_TO_EDGE, VK_SAMPLER_ADDRESS_CLAMP_TO_EDGE,
                               VK_SAMPLER_ADDRESS_CLAMP_TO_EDGE, 0, 0, 1.0f, 0, 0, 0, 0, 0, 0 };
    return d->api.vkCreateSampler(d->dev, &si, NULL, &u->sampler);
}

static void layout_barrier(vk_device* d, VkCommandBuffer cmd, VkImage img, uint32_t from, uint32_t to, VkFlags src_stage,
                           VkFlags src_access, VkFlags dst_stage, VkFlags dst_access) {
    VkImageMemoryBarrier b = { VK_STYPE_IMAGE_MEMORY_BARRIER, NULL, src_access, dst_access, from, to,
                               VK_QUEUE_FAMILY_IGNORED, VK_QUEUE_FAMILY_IGNORED, img, { VK_ASPECT_COLOR, 0, 1, 0, 1 } };
    d->api.vkCmdPipelineBarrier(cmd, src_stage, dst_stage, 0, 0, NULL, 0, NULL, 1, &b);
}

/* Limpa a textura vazia com zero numa submissão própria (uma vez por device). */
static VkResult clear_empty(vk_device* d, VkImage img) {
    vk_api* a = &d->api;
    VkCommandBuffer cmd = NULL;
    VkFence fence = 0;
    VkCommandBufferAllocateInfo ai = { VK_STYPE_COMMAND_BUFFER_ALLOCATE, NULL, d->pool, 0, 1 };
    VkFenceCreateInfo fci = { VK_STYPE_FENCE_CREATE_INFO, NULL, 0 };
    VkCommandBufferBeginInfo bi = { VK_STYPE_COMMAND_BUFFER_BEGIN_INFO, NULL, VK_CMD_BUFFER_ONE_TIME_SUBMIT, NULL };
    VkResult r = a->vkAllocateCommandBuffers(d->dev, &ai, &cmd);
    if (r == VK_SUCCESS) r = a->vkCreateFence(d->dev, &fci, NULL, &fence);
    if (r == VK_SUCCESS) r = a->vkBeginCommandBuffer(cmd, &bi);
    if (r == VK_SUCCESS) {
        const VkClearValue zero = { { 0, 0, 0, 0 } };
        const VkImageSubresourceRange range = { VK_ASPECT_COLOR, 0, 1, 0, 1 };
        layout_barrier(d, cmd, img, VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_STAGE_TOP_OF_PIPE, 0,
                       VK_STAGE_TRANSFER, VK_ACCESS_TRANSFER_WRITE);
        a->vkCmdClearColorImage(cmd, img, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, &zero, 1, &range);
        layout_barrier(d, cmd, img, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL,
                       VK_STAGE_TRANSFER, VK_ACCESS_TRANSFER_WRITE, VK_STAGE_FRAGMENT_SHADER, VK_ACCESS_SHADER_READ);
        r = a->vkEndCommandBuffer(cmd);
    }
    VkSubmitInfo si = { VK_STYPE_SUBMIT_INFO, NULL, 0, NULL, NULL, 1, &cmd, 0, NULL };
    if (r == VK_SUCCESS) r = a->vkQueueSubmit(d->queue, 1, &si, fence);
    if (r == VK_SUCCESS) r = a->vkWaitForFences(d->dev, 1, &fence, VK_TRUE, ~0ull);
    if (cmd) a->vkFreeCommandBuffers(d->dev, d->pool, 1, &cmd);
    VK_DESTROY(d, vkDestroyFence, fence);
    return r;
}

static VkResult create_empty(vk_device* d, vk_user_shaders* u) {
    vk_api* a = &d->api;
    VkImageCreateInfo ci = { VK_STYPE_IMAGE_CREATE_INFO, NULL, 0, VK_IMAGE_TYPE_2D, VK_FORMAT_B8G8R8A8_UNORM, { 1, 1, 1 },
                             1, 1, VK_SAMPLE_COUNT_1, 0, VK_IMAGE_USAGE_TRANSFER_DST | VK_IMAGE_USAGE_SAMPLED, 0, 0, NULL,
                             VK_IMAGE_LAYOUT_UNDEFINED };
    VkResult r = a->vkCreateImage(d->dev, &ci, NULL, &u->empty);
    if (r != VK_SUCCESS) return r;
    VkMemoryRequirements req;
    uint32_t type;
    a->vkGetImageMemoryRequirements(d->dev, u->empty, &req);
    if (!vk_memory_type(d, req.memoryTypeBits, VK_MEMORY_DEVICE_LOCAL, &type) &&
        !vk_memory_type(d, req.memoryTypeBits, 0, &type))
        return -1;
    VkMemoryAllocateInfo mi = { VK_STYPE_MEMORY_ALLOCATE_INFO, NULL, req.size, type };
    r = a->vkAllocateMemory(d->dev, &mi, NULL, &u->empty_memory);
    if (r == VK_SUCCESS) r = a->vkBindImageMemory(d->dev, u->empty, u->empty_memory, 0);
    if (r != VK_SUCCESS) return r;
    VkImageViewCreateInfo vi = { VK_STYPE_IMAGE_VIEW_CREATE_INFO, NULL, 0, u->empty, VK_IMAGE_VIEW_TYPE_2D,
                                 VK_FORMAT_B8G8R8A8_UNORM, { 0, 0, 0, 0 }, { VK_ASPECT_COLOR, 0, 1, 0, 1 } };
    r = a->vkCreateImageView(d->dev, &vi, NULL, &u->empty_view);
    return r == VK_SUCCESS ? clear_empty(d, u->empty) : r;
}

static void common_free(vk_device* d, vk_user_shaders* u) {
    VK_DESTROY(d, vkDestroyImageView, u->empty_view);
    VK_DESTROY(d, vkDestroyImage, u->empty);
    VK_DESTROY(d, vkFreeMemory, u->empty_memory);
    VK_DESTROY(d, vkDestroySampler, u->sampler);
    VK_DESTROY(d, vkDestroyPipelineLayout, u->layout);
    VK_DESTROY(d, vkDestroyDescriptorSetLayout, u->set_layout);
}

hg_result vk_user_shaders_common(vk_device* d) {
    vk_user_shaders* u = &d->user_shaders;
    if (u->empty_view) return HG_OK;
    VkResult r = create_layouts(d, u);
    if (r == VK_SUCCESS) r = create_sampler(d, u);
    if (r == VK_SUCCESS) r = create_empty(d, u);
    if (r != VK_SUCCESS) {
        common_free(d, u);
        return hg__fail(HG_ERR_DEVICE, "vulkan: falha ao criar estado dos shaders do usuário");
    }
    return HG_OK;
}

static VkResult ensure_sets(vk_device* d, vk_shader_frame* sf) {
    if (sf->pool) return VK_SUCCESS;
    vk_api* a = &d->api;
    VkDescriptorPoolSize sizes[3] = {
        { VK_DESCRIPTOR_UNIFORM_BUFFER_DYNAMIC, 2 }, { VK_DESCRIPTOR_SAMPLED_IMAGE, 2 }, { VK_DESCRIPTOR_SAMPLER, 2 } };
    VkDescriptorPoolCreateInfo pi = { VK_STYPE_DESCRIPTOR_POOL_CREATE_INFO, NULL, 0, 2, 3, sizes };
    VkResult r = a->vkCreateDescriptorPool(d->dev, &pi, NULL, &sf->pool);
    if (r != VK_SUCCESS) return r;
    VkDescriptorSetLayout layouts[2] = { d->user_shaders.set_layout, d->user_shaders.set_layout };
    VkDescriptorSetAllocateInfo ai = { VK_STYPE_DESCRIPTOR_SET_ALLOCATE, NULL, sf->pool, 2, layouts };
    r = a->vkAllocateDescriptorSets(d->dev, &ai, sf->sets);
    if (r != VK_SUCCESS) VK_DESTROY(d, vkDestroyDescriptorPool, sf->pool);
    sf->set_capacity = 0;
    return r;
}

/* Reescreve os sets só quando o buffer ou a view do backdrop mudaram. Seguro aqui: a fence do
 * slot já foi esperada e nenhum comando deste frame ligou os sets ainda. */
static void write_sets(vk_device* d, vk_shader_frame* sf, const vk_backdrop* bd) {
    const vk_user_shaders* u = &d->user_shaders;
    uint32_t gen = bd->view ? bd->generation : 0;
    if (sf->set_capacity == sf->uniforms.capacity && sf->set_backdrop == gen) return;
    VkImageView backdrop = bd->view ? bd->view : u->empty_view;
    VkDescriptorBufferInfo buf = { sf->uniforms.buffer, 0, HG_SHADER_UNIFORM_BYTES };
    VkDescriptorImageInfo empty = { 0, u->empty_view, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL };
    VkDescriptorImageInfo back = { 0, backdrop, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL };
    VkDescriptorImageInfo smp = { u->sampler, 0, 0 };
    VkWriteDescriptorSet w[6];
    for (int i = 0; i < 2; i++) {
        VkWriteDescriptorSet* p = &w[i * 3];
        VkWriteDescriptorSet ub = { VK_STYPE_WRITE_DESCRIPTOR_SET, NULL, sf->sets[i], 0, 0, 1,
                                    VK_DESCRIPTOR_UNIFORM_BUFFER_DYNAMIC, NULL, &buf, NULL };
        VkWriteDescriptorSet im = { VK_STYPE_WRITE_DESCRIPTOR_SET, NULL, sf->sets[i], 1, 0, 1, VK_DESCRIPTOR_SAMPLED_IMAGE,
                                    i ? &back : &empty, NULL, NULL };
        VkWriteDescriptorSet sm = { VK_STYPE_WRITE_DESCRIPTOR_SET, NULL, sf->sets[i], 2, 0, 1, VK_DESCRIPTOR_SAMPLER, &smp,
                                    NULL, NULL };
        p[0] = ub;
        p[1] = im;
        p[2] = sm;
    }
    d->api.vkUpdateDescriptorSets(d->dev, 6, w, 0, NULL);
    sf->set_capacity = sf->uniforms.capacity;
    sf->set_backdrop = gen;
}

void vk_shader_frame_prepare(vk_device* d, vk_surface* s, vk_frame* f, const hg_draw_list* dl) {
    vk_shader_frame* sf = &f->shaders;
    sf->capacity = sf->used = 0;
    uint32_t count = 0;
    int backdrop = 0;
    for (const hg_batch* b = dl->first; b; b = b->next)
        if (b->shader) {
            count++;
            backdrop |= (b->shader_draw.flags & HG_SHADER_BACKDROP) != 0;
        }
    if (count == 0) return; /* sem shader: custo zero */
    /* Falhas aqui (OOM) não derrubam o frame: os lotes de shader só não desenham. */
    if (vk_user_shaders_common(d) != HG_OK) return;
    if (vk_buffer_reserve(d, &sf->uniforms, (VkDeviceSize)count * VK_SHADER_UNIFORM_STRIDE,
                          VK_BUFFER_USAGE_UNIFORM_BUFFER) != HG_OK)
        return;
    if (ensure_sets(d, sf) != VK_SUCCESS) return;
    if (backdrop) vk_backdrop_ensure(d, s);
    write_sets(d, sf, &s->backdrop);
    sf->capacity = count;
}

void vk_shader_frame_release(vk_device* d, vk_frame* f) {
    vk_shader_frame* sf = &f->shaders;
    VK_DESTROY(d, vkDestroyDescriptorPool, sf->pool); /* libera os sets */
    vk_buffer_release(d, &sf->uniforms);
    sf->sets[0] = sf->sets[1] = 0;
    sf->set_capacity = 0;
    sf->set_backdrop = 0;
    sf->capacity = sf->used = 0;
}

void vk_user_shaders_release(vk_device* d) {
    vk_retired_pipelines_release(d);
    common_free(d, &d->user_shaders);
}

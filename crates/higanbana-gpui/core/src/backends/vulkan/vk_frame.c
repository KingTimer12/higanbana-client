/* vk_frame.c — ciclo do frame: espera o slot (2 em voo), adquire imagem, grava,
 * submete e apresenta. OUT_OF_DATE/SUBOPTIMAL recriam a swapchain. */
#include "vk_internal.h"
#include "error.h"

hg_result vk_frames_create(vk_device* d, vk_surface* s) {
    vk_api* a = &d->api;
    VkCommandBuffer cmds[VK_FRAMES];
    VkCommandBufferAllocateInfo ai = { VK_STYPE_COMMAND_BUFFER_ALLOCATE, NULL, d->pool, 0, VK_FRAMES };
    if (a->vkAllocateCommandBuffers(d->dev, &ai, cmds) != VK_SUCCESS)
        return hg__fail(HG_ERR_DEVICE, "vulkan: vkAllocateCommandBuffers falhou");
    VkFenceCreateInfo fci = { VK_STYPE_FENCE_CREATE_INFO, NULL, VK_FENCE_CREATE_SIGNALED };
    VkSemaphoreCreateInfo sci = { VK_STYPE_SEMAPHORE_CREATE_INFO, NULL, 0 };
    for (int i = 0; i < VK_FRAMES; i++) {
        vk_frame* f = &s->frames[i];
        f->cmd = cmds[i];
        if (a->vkCreateFence(d->dev, &fci, NULL, &f->fence) != VK_SUCCESS ||
            a->vkCreateSemaphore(d->dev, &sci, NULL, &f->acquired) != VK_SUCCESS)
            return hg__fail(HG_ERR_DEVICE, "vulkan: sincronização do frame falhou");
    }
    return HG_OK;
}

void vk_frames_release(vk_device* d, vk_surface* s) {
    for (int i = 0; i < VK_FRAMES; i++) {
        vk_frame* f = &s->frames[i];
        if (f->cmd) d->api.vkFreeCommandBuffers(d->dev, d->pool, 1, &f->cmd);
        f->cmd = NULL;
        VK_DESTROY(d, vkDestroyFence, f->fence);
        VK_DESTROY(d, vkDestroySemaphore, f->acquired);
        vk_buffer_release(d, &f->quads);
        vk_buffer_release(d, &f->glyphs);
        vk_buffer_release(d, &f->staging);
        vk_buffer_release(d, &f->images);
    }
}

hg_result vk_frame_record(vk_device* d, vk_surface* s) {
    if (s->recording) return HG_OK;
    VkCommandBufferBeginInfo bi = { VK_STYPE_COMMAND_BUFFER_BEGIN_INFO, NULL, VK_CMD_BUFFER_ONE_TIME_SUBMIT, NULL };
    if (d->api.vkBeginCommandBuffer(s->frames[s->slot].cmd, &bi) != VK_SUCCESS)
        return hg__fail(HG_ERR_DEVICE, "vulkan: vkBeginCommandBuffer falhou");
    s->recording = 1;
    d->images.recording++;
    return HG_OK;
}

static VkResult acquire(vk_device* d, vk_surface* s) {
    return d->api.vkAcquireNextImageKHR(d->dev, s->swapchain, ~0ull, s->frames[s->slot].acquired, 0, &s->image);
}

hg_result vk_frame_begin(vk_device* d, vk_surface* s, uint32_t w, uint32_t h, hg_color clear) {
    s->active = 0;
    hg_result r = HG_OK;
    if (s->dirty || !s->swapchain || w != s->req_w || h != s->req_h) r = vk_swapchain_build(d, s, w, h);
    if (r != HG_OK || s->dirty) return r; /* sem extensão válida: frame vira no-op */

    vk_frame* f = &s->frames[s->slot];
    d->api.vkWaitForFences(d->dev, 1, &f->fence, VK_TRUE, ~0ull);
    VkResult vr = acquire(d, s);
    if (vr == VK_ERROR_OUT_OF_DATE_KHR) {
        r = vk_swapchain_build(d, s, w, h);
        if (r != HG_OK || s->dirty) return r;
        vr = acquire(d, s);
    }
    if (vr == VK_SUBOPTIMAL_KHR) s->dirty = 1; /* usa esta imagem, recria no próximo */
    else if (vr == VK_ERROR_OUT_OF_DATE_KHR) { s->dirty = 1; return HG_OK; }
    else if (vr != VK_SUCCESS) return hg__fail(HG_ERR_DEVICE, "vulkan: vkAcquireNextImageKHR falhou (device perdido?)");

    s->active = 1;
    s->acquire_pending = 1;
    s->cleared = 0;
    s->rendered_signaled = 0;
    s->clear.color[0] = clear.r;
    s->clear.color[1] = clear.g;
    s->clear.color[2] = clear.b;
    s->clear.color[3] = clear.a;
    return vk_frame_record(d, s);
}

/* Primeiro pass do frame limpa; os seguintes carregam o conteúdo. */
void vk_frame_pass_begin(vk_device* d, vk_surface* s) {
    VkRenderPassBeginInfo bi = { VK_STYPE_RENDER_PASS_BEGIN_INFO, NULL,
                                 s->cleared ? d->pipeline.pass_load : d->pipeline.pass_clear,
                                 s->framebuffers[s->image], { { 0, 0 }, { s->width, s->height } }, 1, &s->clear };
    d->api.vkCmdBeginRenderPass(s->frames[s->slot].cmd, &bi, 0 /* INLINE */);
    s->cleared = 1;
}

hg_result vk_frame_flush(vk_device* d, vk_surface* s, int signal_rendered) {
    vk_frame* f = &s->frames[s->slot];
    if (d->api.vkEndCommandBuffer(f->cmd) != VK_SUCCESS) return hg__fail(HG_ERR_DEVICE, "vulkan: vkEndCommandBuffer falhou");
    s->recording = 0;
    d->images.recording--;
    const VkFlags wait_stage = VK_STAGE_COLOR_OUTPUT | VK_STAGE_TRANSFER;
    VkSubmitInfo si = { VK_STYPE_SUBMIT_INFO, NULL, s->acquire_pending ? 1u : 0u, &f->acquired, &wait_stage,
                        1, &f->cmd, signal_rendered ? 1u : 0u, &s->rendered[s->image] };
    d->api.vkResetFences(d->dev, 1, &f->fence);
    if (d->api.vkQueueSubmit(d->queue, 1, &si, f->fence) != VK_SUCCESS)
        return hg__fail(HG_ERR_DEVICE, "vulkan: vkQueueSubmit falhou (device perdido?)");
    s->acquire_pending = 0;
    s->rendered_signaled = signal_rendered;
    return HG_OK;
}

hg_result vk_frame_end(vk_device* d, vk_surface* s, int vsync) {
    if (!s->active) return HG_OK;
    s->active = 0;
    hg_result r = HG_OK;
    if (!s->cleared) { /* frame sem draw: só limpa */
        r = vk_frame_record(d, s);
        if (r == HG_OK) {
            vk_frame_pass_begin(d, s);
            d->api.vkCmdEndRenderPass(s->frames[s->slot].cmd);
        }
    }
    if (r == HG_OK && s->recording) r = vk_frame_flush(d, s, 1);
    if (r != HG_OK) return r;

    VkPresentInfoKHR pi = { VK_STYPE_PRESENT_INFO_KHR, NULL, s->rendered_signaled ? 1u : 0u, &s->rendered[s->image],
                            1, &s->swapchain, &s->image, NULL };
    VkResult vr = d->api.vkQueuePresentKHR(d->queue, &pi);
    s->slot = (s->slot + 1) % VK_FRAMES;
    if (vsync != s->vsync) { s->vsync = vsync; s->dirty = 1; }
    if (vr == VK_ERROR_OUT_OF_DATE_KHR || vr == VK_SUBOPTIMAL_KHR) s->dirty = 1;
    else if (vr != VK_SUCCESS) return hg__fail(HG_ERR_DEVICE, "vulkan: vkQueuePresentKHR falhou");
    return HG_OK;
}

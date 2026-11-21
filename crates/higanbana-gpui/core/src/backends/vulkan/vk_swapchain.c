/* vk_swapchain.c — VkSurfaceKHR Win32 e swapchain B8G8R8A8 por janela, com image
 * views, framebuffers e semáforo "renderizado" por imagem. */
#include "vk_internal.h"
#include "error.h"
#include <string.h>

static void release_images(vk_device* d, vk_surface* s) {
    for (uint32_t i = 0; i < s->image_count; i++) {
        VK_DESTROY(d, vkDestroyFramebuffer, s->framebuffers[i]);
        VK_DESTROY(d, vkDestroyImageView, s->views[i]);
        VK_DESTROY(d, vkDestroySemaphore, s->rendered[i]);
    }
    s->image_count = 0;
}

static uint32_t pick_present_mode(vk_device* d, vk_surface* s) {
    if (s->vsync) return VK_PRESENT_MODE_FIFO_KHR;
    uint32_t modes[8], n = 8, best = VK_PRESENT_MODE_FIFO_KHR;
    d->api.vkGetPhysicalDeviceSurfacePresentModesKHR(d->phys, s->surface, &n, modes);
    for (uint32_t i = 0; i < n; i++) {
        if (modes[i] == VK_PRESENT_MODE_MAILBOX_KHR) return modes[i];
        if (modes[i] == VK_PRESENT_MODE_IMMEDIATE_KHR) best = modes[i];
    }
    return best;
}

static int has_bgra(vk_device* d, vk_surface* s) {
    VkSurfaceFormatKHR fmts[32];
    uint32_t n = 32;
    d->api.vkGetPhysicalDeviceSurfaceFormatsKHR(d->phys, s->surface, &n, fmts);
    for (uint32_t i = 0; i < n; i++)
        if (fmts[i].format == VK_FORMAT_B8G8R8A8_UNORM) return 1;
    return 0;
}

static hg_result create_views(vk_device* d, vk_surface* s) {
    vk_api* a = &d->api;
    uint32_t n = 0;
    a->vkGetSwapchainImagesKHR(d->dev, s->swapchain, &n, NULL);
    if (n == 0 || n > VK_MAX_IMAGES) return hg__fail(HG_ERR_DEVICE, "vulkan: número de imagens da swapchain inválido");
    a->vkGetSwapchainImagesKHR(d->dev, s->swapchain, &n, s->images);
    s->image_count = n;
    VkSemaphoreCreateInfo sci = { VK_STYPE_SEMAPHORE_CREATE_INFO, NULL, 0 };
    for (uint32_t i = 0; i < n; i++) {
        VkImageViewCreateInfo vi = { VK_STYPE_IMAGE_VIEW_CREATE_INFO, NULL, 0, s->images[i], VK_IMAGE_VIEW_TYPE_2D,
                                     VK_FORMAT_B8G8R8A8_UNORM, { 0, 0, 0, 0 }, { VK_ASPECT_COLOR, 0, 1, 0, 1 } };
        if (a->vkCreateImageView(d->dev, &vi, NULL, &s->views[i]) != VK_SUCCESS)
            return hg__fail(HG_ERR_DEVICE, "vulkan: vkCreateImageView falhou");
        VkFramebufferCreateInfo fi = { VK_STYPE_FRAMEBUFFER_CREATE_INFO, NULL, 0, d->pipeline.pass_clear, 1, &s->views[i],
                                       s->width, s->height, 1 };
        if (a->vkCreateFramebuffer(d->dev, &fi, NULL, &s->framebuffers[i]) != VK_SUCCESS ||
            a->vkCreateSemaphore(d->dev, &sci, NULL, &s->rendered[i]) != VK_SUCCESS)
            return hg__fail(HG_ERR_DEVICE, "vulkan: framebuffer da swapchain falhou");
    }
    return HG_OK;
}

hg_result vk_swapchain_build(vk_device* d, vk_surface* s, uint32_t w, uint32_t h) {
    vk_api* a = &d->api;
    VkSurfaceCapabilitiesKHR caps;
    if (a->vkGetPhysicalDeviceSurfaceCapabilitiesKHR(d->phys, s->surface, &caps) != VK_SUCCESS)
        return hg__fail(HG_ERR_DEVICE, "vulkan: superfície perdida");
    s->req_w = w; /* tamanho pedido; a extensão real vem da superfície */
    s->req_h = h;
    if (caps.currentExtent.width != ~0u) { w = caps.currentExtent.width; h = caps.currentExtent.height; }
    if (w == 0 || h == 0) { s->dirty = 1; return HG_OK; } /* minimizada: tenta de novo depois */

    a->vkDeviceWaitIdle(d->dev);
    release_images(d, s);
    uint32_t count = caps.minImageCount + 1;
    if (caps.maxImageCount && count > caps.maxImageCount) count = caps.maxImageCount;
    VkFlags usage = VK_IMAGE_USAGE_COLOR_ATTACHMENT | (caps.supportedUsageFlags & VK_IMAGE_USAGE_TRANSFER_SRC);
    uint32_t alpha = (caps.supportedCompositeAlpha & VK_COMPOSITE_ALPHA_OPAQUE_KHR)
                         ? VK_COMPOSITE_ALPHA_OPAQUE_KHR : (caps.supportedCompositeAlpha & (0u - caps.supportedCompositeAlpha));
    VkSwapchainKHR old = s->swapchain;
    VkSwapchainCreateInfoKHR ci = { VK_STYPE_SWAPCHAIN_CREATE_INFO_KHR, NULL, 0, s->surface, count, VK_FORMAT_B8G8R8A8_UNORM,
                                    VK_COLOR_SPACE_SRGB_NONLINEAR_KHR, { w, h }, 1, usage, 0, 0, NULL,
                                    caps.currentTransform, alpha, pick_present_mode(d, s), VK_TRUE, old };
    VkResult r = a->vkCreateSwapchainKHR(d->dev, &ci, NULL, &s->swapchain);
    if (old) a->vkDestroySwapchainKHR(d->dev, old, NULL);
    if (r != VK_SUCCESS) {
        s->swapchain = 0;
        return hg__fail(HG_ERR_DEVICE, "vulkan: vkCreateSwapchainKHR falhou");
    }
    s->width = w;
    s->height = h;
    s->dirty = 0;
    return create_views(d, s);
}

hg_result vk_surface_create(vk_device* d, HWND hwnd, uint32_t w, uint32_t h, vk_surface* s) {
    memset(s, 0, sizeof *s);
    s->vsync = 1;
    VkWin32SurfaceCreateInfoKHR ci = { VK_STYPE_WIN32_SURFACE_CREATE_KHR, NULL, 0, GetModuleHandleW(NULL), hwnd };
    if (d->api.vkCreateWin32SurfaceKHR(d->instance, &ci, NULL, &s->surface) != VK_SUCCESS)
        return hg__fail(HG_ERR_DEVICE, "vulkan: vkCreateWin32SurfaceKHR falhou");
    VkBool32 ok = 0;
    d->api.vkGetPhysicalDeviceSurfaceSupportKHR(d->phys, d->family, s->surface, &ok);
    if (!ok || !has_bgra(d, s)) return hg__fail(HG_ERR_DEVICE, "vulkan: superfície sem suporte a B8G8R8A8");
    hg_result r = vk_frames_create(d, s);
    return r == HG_OK ? vk_swapchain_build(d, s, w, h) : r;
}

void vk_surface_release(vk_device* d, vk_surface* s) {
    if (d->dev) d->api.vkDeviceWaitIdle(d->dev);
    vk_frames_release(d, s);
    release_images(d, s);
    VK_DESTROY(d, vkDestroySwapchainKHR, s->swapchain);
    if (s->surface) d->api.vkDestroySurfaceKHR(d->instance, s->surface, NULL);
    s->surface = 0;
}

/* vk_device.c — instância (surface Win32), escolha de GPU (discreta primeiro) e device. */
#include "vk_internal.h"
#include "error.h"
#include <string.h>

static const char* const k_instance_exts[] = { "VK_KHR_surface", "VK_KHR_win32_surface" };
static const char* const k_device_exts[] = { "VK_KHR_swapchain" };

hg_result vk_instance_create(vk_api* api, VkInstance* out) {
    VkApplicationInfo app = { VK_STYPE_APPLICATION_INFO, NULL, "higanbana", 1, "higanbana-gpui", 1, VK_API_VERSION_1_0 };
    VkInstanceCreateInfo ci = { VK_STYPE_INSTANCE_CREATE_INFO, NULL, 0, &app, 0, NULL, 2, k_instance_exts };
    *out = NULL;
    if (api->vkCreateInstance(&ci, NULL, out) != VK_SUCCESS) return hg__fail(HG_ERR_UNAVAILABLE, "vulkan: vkCreateInstance falhou");
    if (!vk_api_load_instance(api, *out)) {
        PFN_vkDestroyInstance destroy = (PFN_vkDestroyInstance)(void*)api->vkGetInstanceProcAddr(*out, "vkDestroyInstance");
        if (destroy) destroy(*out, NULL);
        *out = NULL;
        return hg__fail(HG_ERR_UNAVAILABLE, "vulkan: funções de instância ausentes");
    }
    return HG_OK;
}

/* Família com fila gráfica que apresenta em Win32; ~0u se não houver. */
static uint32_t graphics_family(vk_api* api, VkPhysicalDevice p) {
    VkQueueFamilyProperties props[16];
    uint32_t n = 16;
    api->vkGetPhysicalDeviceQueueFamilyProperties(p, &n, props);
    for (uint32_t i = 0; i < n; i++)
        if ((props[i].queueFlags & VK_QUEUE_GRAPHICS_BIT) && api->vkGetPhysicalDeviceWin32PresentationSupportKHR(p, i))
            return i;
    return ~0u;
}

int vk_pick_physical(vk_api* api, VkInstance inst, VkPhysicalDevice* out, uint32_t* family, char* name, size_t name_size) {
    VkPhysicalDevice devs[16];
    uint32_t n = 16;
    VkResult r = api->vkEnumeratePhysicalDevices(inst, &n, devs);
    if (r != VK_SUCCESS && r != 5 /* VK_INCOMPLETE */) return 0;
    int best = -1, best_score = -1;
    uint32_t best_family = 0;
    VkPhysicalDeviceProperties props;
    for (uint32_t i = 0; i < n; i++) {
        uint32_t fam = graphics_family(api, devs[i]);
        if (fam == ~0u) continue;
        api->vkGetPhysicalDeviceProperties(devs[i], &props);
        int score = props.deviceType == VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU ? 2 : props.deviceType == 1 /* integrada */ ? 1 : 0;
        if (score > best_score) { best = (int)i; best_score = score; best_family = fam; }
    }
    if (best < 0) return 0;
    *out = devs[best];
    *family = best_family;
    if (name && name_size) {
        api->vkGetPhysicalDeviceProperties(devs[best], &props);
        strncpy_s(name, name_size, props.deviceName, _TRUNCATE);
    }
    return 1;
}

static hg_result create_logical(vk_device* d) {
    const float priority = 1.0f;
    VkDeviceQueueCreateInfo q = { VK_STYPE_DEVICE_QUEUE_CREATE_INFO, NULL, 0, d->family, 1, &priority };
    VkDeviceCreateInfo ci = { VK_STYPE_DEVICE_CREATE_INFO, NULL, 0, 1, &q, 0, NULL, 1, k_device_exts, NULL };
    if (d->api.vkCreateDevice(d->phys, &ci, NULL, &d->dev) != VK_SUCCESS)
        return hg__fail(HG_ERR_DEVICE, "vulkan: vkCreateDevice falhou");
    if (!vk_api_load_device(&d->api, d->dev)) return hg__fail(HG_ERR_DEVICE, "vulkan: funções de device ausentes");
    d->api.vkGetDeviceQueue(d->dev, d->family, 0, &d->queue);
    VkCommandPoolCreateInfo pool = { VK_STYPE_COMMAND_POOL_CREATE_INFO, NULL, VK_CMD_POOL_RESET_COMMAND_BUFFER, d->family };
    if (d->api.vkCreateCommandPool(d->dev, &pool, NULL, &d->pool) != VK_SUCCESS)
        return hg__fail(HG_ERR_DEVICE, "vulkan: vkCreateCommandPool falhou");
    return HG_OK;
}

hg_result vk_device_create(vk_device* d, hg_backend_info* info) {
    memset(d, 0, sizeof *d);
    if (!vk_shared_instance(&d->api, &d->instance)) return hg__fail(HG_ERR_UNAVAILABLE, "vulkan: instância indisponível");
    hg_result r = HG_OK;
    if (!vk_pick_physical(&d->api, d->instance, &d->phys, &d->family,
                                        info ? info->adapter : NULL, info ? sizeof info->adapter : 0))
        r = hg__fail(HG_ERR_UNAVAILABLE, "vulkan: nenhuma GPU com apresentação Win32");
    if (r == HG_OK) {
        d->api.vkGetPhysicalDeviceMemoryProperties(d->phys, &d->memory);
        r = create_logical(d);
    }
    if (r == HG_OK) r = vk_pipeline_create(d);
    if (r == HG_OK) r = vk_buffer_reserve(d, &d->readback, 256, VK_BUFFER_USAGE_TRANSFER_DST);
    if (r != HG_OK) vk_device_release(d);
    return r;
}

void vk_device_release(vk_device* d) {
    if (d->dev) {
        d->api.vkDeviceWaitIdle(d->dev);
        vk_buffer_release(d, &d->readback);
        vk_glyphs_release(d);
        vk_images_release(d);
        vk_pipeline_release(d);
        VK_DESTROY(d, vkDestroyCommandPool, d->pool);
        d->api.vkDestroyDevice(d->dev, NULL);
        d->dev = NULL;
    }
    /* Instância e DLL são do processo (vk_instance.c): não destruir aqui. */
    d->instance = NULL;
}

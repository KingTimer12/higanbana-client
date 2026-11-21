/* vk_loader.c — carrega vulkan-1.dll em runtime e preenche a tabela de funções. */
#include "vk_api.h"
#include <string.h>

int vk_api_open(vk_api* api) {
    memset(api, 0, sizeof *api);
    api->lib = LoadLibraryW(L"vulkan-1.dll");
    if (!api->lib) return 0;
    api->vkGetInstanceProcAddr = (PFN_vkGetInstanceProcAddr)(void*)GetProcAddress(api->lib, "vkGetInstanceProcAddr");
    if (api->vkGetInstanceProcAddr)
        api->vkCreateInstance = (PFN_vkCreateInstance)(void*)api->vkGetInstanceProcAddr(NULL, "vkCreateInstance");
    if (!api->vkCreateInstance) { vk_api_close(api); return 0; }
    return 1;
}

int vk_api_load_instance(vk_api* api, VkInstance inst) {
    int ok = 1;
#define HG_VK_LOAD_INSTANCE(name, ret, params) \
    ok &= (api->name = (PFN_##name)(void*)api->vkGetInstanceProcAddr(inst, #name)) != NULL;
    HG_VK_INSTANCE_FNS(HG_VK_LOAD_INSTANCE)
#undef HG_VK_LOAD_INSTANCE
    return ok;
}

int vk_api_load_device(vk_api* api, VkDevice dev) {
    int ok = 1;
#define HG_VK_LOAD_DEVICE(name, ret, params) \
    ok &= (api->name = (PFN_##name)(void*)api->vkGetDeviceProcAddr(dev, #name)) != NULL;
    HG_VK_DEVICE_FNS(HG_VK_LOAD_DEVICE)
#undef HG_VK_LOAD_DEVICE
    return ok;
}

void vk_api_close(vk_api* api) {
    if (api->lib) FreeLibrary(api->lib);
    memset(api, 0, sizeof *api);
}

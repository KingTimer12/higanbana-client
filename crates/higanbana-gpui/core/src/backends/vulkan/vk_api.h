/* vk_api.h — assinaturas das funções Vulkan usadas (X-macros) e a tabela carregada
 * em runtime de vulkan-1.dll. Alocadores (VkAllocationCallbacks*) são sempre NULL. */
#ifndef HG_VK_API_H
#define HG_VK_API_H
#include "vk_pipeline_structs.h"

typedef const void* VkAlloc;

/* Nível instância (via vkGetInstanceProcAddr(instance, ...)). */
#define HG_VK_INSTANCE_FNS(X) \
    X(vkDestroyInstance, void, (VkInstance, VkAlloc)) \
    X(vkEnumeratePhysicalDevices, VkResult, (VkInstance, uint32_t*, VkPhysicalDevice*)) \
    X(vkGetPhysicalDeviceProperties, void, (VkPhysicalDevice, VkPhysicalDeviceProperties*)) \
    X(vkGetPhysicalDeviceQueueFamilyProperties, void, (VkPhysicalDevice, uint32_t*, VkQueueFamilyProperties*)) \
    X(vkGetPhysicalDeviceMemoryProperties, void, (VkPhysicalDevice, VkPhysicalDeviceMemoryProperties*)) \
    X(vkGetPhysicalDeviceWin32PresentationSupportKHR, VkBool32, (VkPhysicalDevice, uint32_t)) \
    X(vkCreateDevice, VkResult, (VkPhysicalDevice, const VkDeviceCreateInfo*, VkAlloc, VkDevice*)) \
    X(vkGetDeviceProcAddr, PFN_vkVoidFunction, (VkDevice, const char*)) \
    X(vkCreateWin32SurfaceKHR, VkResult, (VkInstance, const VkWin32SurfaceCreateInfoKHR*, VkAlloc, VkSurfaceKHR*)) \
    X(vkDestroySurfaceKHR, void, (VkInstance, VkSurfaceKHR, VkAlloc)) \
    X(vkGetPhysicalDeviceSurfaceSupportKHR, VkResult, (VkPhysicalDevice, uint32_t, VkSurfaceKHR, VkBool32*)) \
    X(vkGetPhysicalDeviceSurfaceCapabilitiesKHR, VkResult, (VkPhysicalDevice, VkSurfaceKHR, VkSurfaceCapabilitiesKHR*)) \
    X(vkGetPhysicalDeviceSurfaceFormatsKHR, VkResult, (VkPhysicalDevice, VkSurfaceKHR, uint32_t*, VkSurfaceFormatKHR*)) \
    X(vkGetPhysicalDeviceSurfacePresentModesKHR, VkResult, (VkPhysicalDevice, VkSurfaceKHR, uint32_t*, uint32_t*))

/* Nível device (via vkGetDeviceProcAddr). */
#define HG_VK_DEVICE_FNS(X) \
    X(vkDestroyDevice, void, (VkDevice, VkAlloc)) \
    X(vkGetDeviceQueue, void, (VkDevice, uint32_t, uint32_t, VkQueue*)) \
    X(vkDeviceWaitIdle, VkResult, (VkDevice)) \
    X(vkCreateSwapchainKHR, VkResult, (VkDevice, const VkSwapchainCreateInfoKHR*, VkAlloc, VkSwapchainKHR*)) \
    X(vkDestroySwapchainKHR, void, (VkDevice, VkSwapchainKHR, VkAlloc)) \
    X(vkGetSwapchainImagesKHR, VkResult, (VkDevice, VkSwapchainKHR, uint32_t*, VkImage*)) \
    X(vkAcquireNextImageKHR, VkResult, (VkDevice, VkSwapchainKHR, uint64_t, VkSemaphore, VkFence, uint32_t*)) \
    X(vkQueuePresentKHR, VkResult, (VkQueue, const VkPresentInfoKHR*)) \
    X(vkQueueSubmit, VkResult, (VkQueue, uint32_t, const VkSubmitInfo*, VkFence)) \
    X(vkCreateImageView, VkResult, (VkDevice, const VkImageViewCreateInfo*, VkAlloc, VkImageView*)) \
    X(vkDestroyImageView, void, (VkDevice, VkImageView, VkAlloc)) \
    X(vkCreateRenderPass, VkResult, (VkDevice, const VkRenderPassCreateInfo*, VkAlloc, VkRenderPass*)) \
    X(vkDestroyRenderPass, void, (VkDevice, VkRenderPass, VkAlloc)) \
    X(vkCreateFramebuffer, VkResult, (VkDevice, const VkFramebufferCreateInfo*, VkAlloc, VkFramebuffer*)) \
    X(vkDestroyFramebuffer, void, (VkDevice, VkFramebuffer, VkAlloc)) \
    X(vkCreateShaderModule, VkResult, (VkDevice, const VkShaderModuleCreateInfo*, VkAlloc, VkShaderModule*)) \
    X(vkDestroyShaderModule, void, (VkDevice, VkShaderModule, VkAlloc)) \
    X(vkCreatePipelineLayout, VkResult, (VkDevice, const VkPipelineLayoutCreateInfo*, VkAlloc, VkPipelineLayout*)) \
    X(vkDestroyPipelineLayout, void, (VkDevice, VkPipelineLayout, VkAlloc)) \
    X(vkCreateGraphicsPipelines, VkResult, (VkDevice, VkPipelineCache, uint32_t, const VkGraphicsPipelineCreateInfo*, VkAlloc, VkPipeline*)) \
    X(vkDestroyPipeline, void, (VkDevice, VkPipeline, VkAlloc)) \
    X(vkCreateDescriptorSetLayout, VkResult, (VkDevice, const VkDescriptorSetLayoutCreateInfo*, VkAlloc, VkDescriptorSetLayout*)) \
    X(vkDestroyDescriptorSetLayout, void, (VkDevice, VkDescriptorSetLayout, VkAlloc)) \
    X(vkCreateDescriptorPool, VkResult, (VkDevice, const VkDescriptorPoolCreateInfo*, VkAlloc, VkDescriptorPool*)) \
    X(vkDestroyDescriptorPool, void, (VkDevice, VkDescriptorPool, VkAlloc)) \
    X(vkAllocateDescriptorSets, VkResult, (VkDevice, const VkDescriptorSetAllocateInfo*, VkDescriptorSet*)) \
    X(vkUpdateDescriptorSets, void, (VkDevice, uint32_t, const VkWriteDescriptorSet*, uint32_t, const void*)) \
    X(vkCreateSampler, VkResult, (VkDevice, const VkSamplerCreateInfo*, VkAlloc, VkSampler*)) \
    X(vkDestroySampler, void, (VkDevice, VkSampler, VkAlloc)) \
    X(vkCreateBuffer, VkResult, (VkDevice, const VkBufferCreateInfo*, VkAlloc, VkBuffer*)) \
    X(vkDestroyBuffer, void, (VkDevice, VkBuffer, VkAlloc)) \
    X(vkGetBufferMemoryRequirements, void, (VkDevice, VkBuffer, VkMemoryRequirements*)) \
    X(vkBindBufferMemory, VkResult, (VkDevice, VkBuffer, VkDeviceMemory, VkDeviceSize)) \
    X(vkCreateImage, VkResult, (VkDevice, const VkImageCreateInfo*, VkAlloc, VkImage*)) \
    X(vkDestroyImage, void, (VkDevice, VkImage, VkAlloc)) \
    X(vkGetImageMemoryRequirements, void, (VkDevice, VkImage, VkMemoryRequirements*)) \
    X(vkBindImageMemory, VkResult, (VkDevice, VkImage, VkDeviceMemory, VkDeviceSize)) \
    X(vkAllocateMemory, VkResult, (VkDevice, const VkMemoryAllocateInfo*, VkAlloc, VkDeviceMemory*)) \
    X(vkFreeMemory, void, (VkDevice, VkDeviceMemory, VkAlloc)) \
    X(vkMapMemory, VkResult, (VkDevice, VkDeviceMemory, VkDeviceSize, VkDeviceSize, VkFlags, void**)) \
    X(vkCreateCommandPool, VkResult, (VkDevice, const VkCommandPoolCreateInfo*, VkAlloc, VkCommandPool*)) \
    X(vkDestroyCommandPool, void, (VkDevice, VkCommandPool, VkAlloc)) \
    X(vkAllocateCommandBuffers, VkResult, (VkDevice, const VkCommandBufferAllocateInfo*, VkCommandBuffer*)) \
    X(vkFreeCommandBuffers, void, (VkDevice, VkCommandPool, uint32_t, const VkCommandBuffer*)) \
    X(vkBeginCommandBuffer, VkResult, (VkCommandBuffer, const VkCommandBufferBeginInfo*)) \
    X(vkEndCommandBuffer, VkResult, (VkCommandBuffer)) \
    X(vkCreateFence, VkResult, (VkDevice, const VkFenceCreateInfo*, VkAlloc, VkFence*)) \
    X(vkDestroyFence, void, (VkDevice, VkFence, VkAlloc)) \
    X(vkWaitForFences, VkResult, (VkDevice, uint32_t, const VkFence*, VkBool32, uint64_t)) \
    X(vkResetFences, VkResult, (VkDevice, uint32_t, const VkFence*)) \
    X(vkCreateSemaphore, VkResult, (VkDevice, const VkSemaphoreCreateInfo*, VkAlloc, VkSemaphore*)) \
    X(vkDestroySemaphore, void, (VkDevice, VkSemaphore, VkAlloc)) \
    X(vkCmdBeginRenderPass, void, (VkCommandBuffer, const VkRenderPassBeginInfo*, uint32_t)) \
    X(vkCmdEndRenderPass, void, (VkCommandBuffer)) \
    X(vkCmdBindPipeline, void, (VkCommandBuffer, uint32_t, VkPipeline)) \
    X(vkCmdBindVertexBuffers, void, (VkCommandBuffer, uint32_t, uint32_t, const VkBuffer*, const VkDeviceSize*)) \
    X(vkCmdBindDescriptorSets, void, (VkCommandBuffer, uint32_t, VkPipelineLayout, uint32_t, uint32_t, const VkDescriptorSet*, uint32_t, const uint32_t*)) \
    X(vkCmdPushConstants, void, (VkCommandBuffer, VkPipelineLayout, VkFlags, uint32_t, uint32_t, const void*)) \
    X(vkCmdSetViewport, void, (VkCommandBuffer, uint32_t, uint32_t, const VkViewport*)) \
    X(vkCmdSetScissor, void, (VkCommandBuffer, uint32_t, uint32_t, const VkRect2D*)) \
    X(vkCmdDraw, void, (VkCommandBuffer, uint32_t, uint32_t, uint32_t, uint32_t)) \
    X(vkCmdPipelineBarrier, void, (VkCommandBuffer, VkFlags, VkFlags, VkFlags, uint32_t, const void*, uint32_t, const void*, uint32_t, const VkImageMemoryBarrier*)) \
    X(vkCmdCopyBufferToImage, void, (VkCommandBuffer, VkBuffer, VkImage, uint32_t, uint32_t, const VkBufferImageCopy*)) \
    X(vkCmdCopyImageToBuffer, void, (VkCommandBuffer, VkImage, uint32_t, VkBuffer, uint32_t, const VkBufferImageCopy*)) \
    X(vkCmdCopyImage, void, (VkCommandBuffer, VkImage, uint32_t, VkImage, uint32_t, uint32_t, const VkImageCopy*)) \
    X(vkCmdClearColorImage, void, (VkCommandBuffer, VkImage, uint32_t, const VkClearValue*, uint32_t, const VkImageSubresourceRange*))

#define HG_VK_TYPEDEF(name, ret, params) typedef ret (VKAPI_PTR* PFN_##name) params;
typedef PFN_vkVoidFunction (VKAPI_PTR* PFN_vkGetInstanceProcAddr)(VkInstance, const char*);
typedef VkResult (VKAPI_PTR* PFN_vkCreateInstance)(const VkInstanceCreateInfo*, VkAlloc, VkInstance*);
HG_VK_INSTANCE_FNS(HG_VK_TYPEDEF)
HG_VK_DEVICE_FNS(HG_VK_TYPEDEF)

#define HG_VK_MEMBER(name, ret, params) PFN_##name name;
typedef struct vk_api {
    HMODULE                  lib;
    PFN_vkGetInstanceProcAddr vkGetInstanceProcAddr;
    PFN_vkCreateInstance     vkCreateInstance;
    HG_VK_INSTANCE_FNS(HG_VK_MEMBER)
    HG_VK_DEVICE_FNS(HG_VK_MEMBER)
} vk_api;

/* Abre vulkan-1.dll e resolve vkCreateInstance. 0 se indisponível. */
int  vk_api_open(vk_api* api);
/* Resolve as funções de instância/device. 0 se alguma faltar. */
int  vk_api_load_instance(vk_api* api, VkInstance inst);
int  vk_api_load_device(vk_api* api, VkDevice dev);
void vk_api_close(vk_api* api);
#endif

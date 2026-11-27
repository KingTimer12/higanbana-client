/* vk_structs.h — structs Vulkan usadas pelo backend, na ordem exata de membros da spec.
 * Enums viram uint32_t (mesmo tamanho/alinhamento). Não reordenar campos. */
#ifndef HG_VK_STRUCTS_H
#define HG_VK_STRUCTS_H
#include "vk_types.h"
#include <windows.h>

#define VK_HEADER VkStructureType sType; const void* pNext

typedef struct VkExtent2D { uint32_t width, height; } VkExtent2D;
typedef struct VkExtent3D { uint32_t width, height, depth; } VkExtent3D;
typedef struct VkOffset2D { int32_t x, y; } VkOffset2D;
typedef struct VkOffset3D { int32_t x, y, z; } VkOffset3D;
typedef struct VkRect2D { VkOffset2D offset; VkExtent2D extent; } VkRect2D;
typedef struct VkViewport { float x, y, width, height, minDepth, maxDepth; } VkViewport;
typedef union VkClearValue { float color[4]; uint32_t u[4]; } VkClearValue;

/* ---- instância / device ---------------------------------------------- */
typedef struct VkApplicationInfo {
    VK_HEADER; const char* pApplicationName; uint32_t applicationVersion;
    const char* pEngineName; uint32_t engineVersion; uint32_t apiVersion;
} VkApplicationInfo;
typedef struct VkInstanceCreateInfo {
    VK_HEADER; VkFlags flags; const VkApplicationInfo* pApplicationInfo;
    uint32_t enabledLayerCount; const char* const* ppEnabledLayerNames;
    uint32_t enabledExtensionCount; const char* const* ppEnabledExtensionNames;
} VkInstanceCreateInfo;
typedef struct VkDeviceQueueCreateInfo {
    VK_HEADER; VkFlags flags; uint32_t queueFamilyIndex; uint32_t queueCount; const float* pQueuePriorities;
} VkDeviceQueueCreateInfo;
typedef struct VkDeviceCreateInfo {
    VK_HEADER; VkFlags flags; uint32_t queueCreateInfoCount; const VkDeviceQueueCreateInfo* pQueueCreateInfos;
    uint32_t enabledLayerCount; const char* const* ppEnabledLayerNames;
    uint32_t enabledExtensionCount; const char* const* ppEnabledExtensionNames; const void* pEnabledFeatures;
} VkDeviceCreateInfo;
/* Só lemos o começo; `rest` cobre limits + sparseProperties (824 bytes no total). */
typedef struct VkPhysicalDeviceProperties {
    uint32_t apiVersion, driverVersion, vendorID, deviceID, deviceType;
    char deviceName[256]; uint8_t pipelineCacheUUID[16];
    uint64_t rest[80];
} VkPhysicalDeviceProperties;
typedef struct VkQueueFamilyProperties {
    VkFlags queueFlags; uint32_t queueCount; uint32_t timestampValidBits; VkExtent3D minImageTransferGranularity;
} VkQueueFamilyProperties;
typedef struct VkMemoryType { VkFlags propertyFlags; uint32_t heapIndex; } VkMemoryType;
typedef struct VkMemoryHeap { VkDeviceSize size; VkFlags flags; } VkMemoryHeap;
typedef struct VkPhysicalDeviceMemoryProperties {
    uint32_t memoryTypeCount; VkMemoryType memoryTypes[32];
    uint32_t memoryHeapCount; VkMemoryHeap memoryHeaps[16];
} VkPhysicalDeviceMemoryProperties;

/* ---- superfície / swapchain ------------------------------------------ */
typedef struct VkWin32SurfaceCreateInfoKHR { VK_HEADER; VkFlags flags; HINSTANCE hinstance; HWND hwnd; } VkWin32SurfaceCreateInfoKHR;
typedef struct VkSurfaceCapabilitiesKHR {
    uint32_t minImageCount, maxImageCount; VkExtent2D currentExtent, minImageExtent, maxImageExtent;
    uint32_t maxImageArrayLayers; VkFlags supportedTransforms; uint32_t currentTransform;
    VkFlags supportedCompositeAlpha; VkFlags supportedUsageFlags;
} VkSurfaceCapabilitiesKHR;
typedef struct VkSurfaceFormatKHR { uint32_t format, colorSpace; } VkSurfaceFormatKHR;
typedef struct VkSwapchainCreateInfoKHR {
    VK_HEADER; VkFlags flags; VkSurfaceKHR surface; uint32_t minImageCount; uint32_t imageFormat;
    uint32_t imageColorSpace; VkExtent2D imageExtent; uint32_t imageArrayLayers; VkFlags imageUsage;
    uint32_t imageSharingMode; uint32_t queueFamilyIndexCount; const uint32_t* pQueueFamilyIndices;
    uint32_t preTransform; uint32_t compositeAlpha; uint32_t presentMode; VkBool32 clipped;
    VkSwapchainKHR oldSwapchain;
} VkSwapchainCreateInfoKHR;
typedef struct VkPresentInfoKHR {
    VK_HEADER; uint32_t waitSemaphoreCount; const VkSemaphore* pWaitSemaphores; uint32_t swapchainCount;
    const VkSwapchainKHR* pSwapchains; const uint32_t* pImageIndices; VkResult* pResults;
} VkPresentInfoKHR;

/* ---- imagens, buffers, memória --------------------------------------- */
typedef struct VkImageSubresourceRange {
    VkFlags aspectMask; uint32_t baseMipLevel, levelCount, baseArrayLayer, layerCount;
} VkImageSubresourceRange;
typedef struct VkImageSubresourceLayers { VkFlags aspectMask; uint32_t mipLevel, baseArrayLayer, layerCount; } VkImageSubresourceLayers;
typedef struct VkImageViewCreateInfo {
    VK_HEADER; VkFlags flags; VkImage image; uint32_t viewType; uint32_t format;
    uint32_t components[4]; VkImageSubresourceRange subresourceRange;
} VkImageViewCreateInfo;
typedef struct VkImageCreateInfo {
    VK_HEADER; VkFlags flags; uint32_t imageType; uint32_t format; VkExtent3D extent; uint32_t mipLevels;
    uint32_t arrayLayers; uint32_t samples; uint32_t tiling; VkFlags usage; uint32_t sharingMode;
    uint32_t queueFamilyIndexCount; const uint32_t* pQueueFamilyIndices; uint32_t initialLayout;
} VkImageCreateInfo;
typedef struct VkBufferCreateInfo {
    VK_HEADER; VkFlags flags; VkDeviceSize size; VkFlags usage; uint32_t sharingMode;
    uint32_t queueFamilyIndexCount; const uint32_t* pQueueFamilyIndices;
} VkBufferCreateInfo;
typedef struct VkMemoryRequirements { VkDeviceSize size, alignment; uint32_t memoryTypeBits; } VkMemoryRequirements;
typedef struct VkMemoryAllocateInfo { VK_HEADER; VkDeviceSize allocationSize; uint32_t memoryTypeIndex; } VkMemoryAllocateInfo;
typedef struct VkImageMemoryBarrier {
    VK_HEADER; VkFlags srcAccessMask, dstAccessMask; uint32_t oldLayout, newLayout;
    uint32_t srcQueueFamilyIndex, dstQueueFamilyIndex; VkImage image; VkImageSubresourceRange subresourceRange;
} VkImageMemoryBarrier;
typedef struct VkImageCopy {
    VkImageSubresourceLayers srcSubresource; VkOffset3D srcOffset;
    VkImageSubresourceLayers dstSubresource; VkOffset3D dstOffset; VkExtent3D extent;
} VkImageCopy;
typedef struct VkBufferImageCopy {
    VkDeviceSize bufferOffset; uint32_t bufferRowLength, bufferImageHeight;
    VkImageSubresourceLayers imageSubresource; VkOffset3D imageOffset; VkExtent3D imageExtent;
} VkBufferImageCopy;

/* ---- comandos e sincronização ---------------------------------------- */
typedef struct VkCommandPoolCreateInfo { VK_HEADER; VkFlags flags; uint32_t queueFamilyIndex; } VkCommandPoolCreateInfo;
typedef struct VkCommandBufferAllocateInfo {
    VK_HEADER; VkCommandPool commandPool; uint32_t level; uint32_t commandBufferCount;
} VkCommandBufferAllocateInfo;
typedef struct VkCommandBufferBeginInfo { VK_HEADER; VkFlags flags; const void* pInheritanceInfo; } VkCommandBufferBeginInfo;
typedef struct VkSimpleCreateInfo { VK_HEADER; VkFlags flags; } VkFenceCreateInfo, VkSemaphoreCreateInfo;
typedef struct VkSubmitInfo {
    VK_HEADER; uint32_t waitSemaphoreCount; const VkSemaphore* pWaitSemaphores; const VkFlags* pWaitDstStageMask;
    uint32_t commandBufferCount; const VkCommandBuffer* pCommandBuffers;
    uint32_t signalSemaphoreCount; const VkSemaphore* pSignalSemaphores;
} VkSubmitInfo;
#endif

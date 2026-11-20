/* vk_types.h — subconjunto mínimo da API Vulkan (sem SDK): handles e constantes.
 * Valores copiados da especificação (vulkan_core.h / vulkan_win32.h). */
#ifndef HG_VK_TYPES_H
#define HG_VK_TYPES_H
#include <stdint.h>
#include <stddef.h>

#define VKAPI_PTR __stdcall

/* Handles despacháveis são ponteiros; não despacháveis são uint64_t. */
typedef struct VkInstance_T*       VkInstance;
typedef struct VkPhysicalDevice_T* VkPhysicalDevice;
typedef struct VkDevice_T*         VkDevice;
typedef struct VkQueue_T*          VkQueue;
typedef struct VkCommandBuffer_T*  VkCommandBuffer;
typedef uint64_t VkSurfaceKHR, VkSwapchainKHR, VkImage, VkImageView, VkBuffer, VkDeviceMemory;
typedef uint64_t VkRenderPass, VkFramebuffer, VkShaderModule, VkPipeline, VkPipelineLayout;
typedef uint64_t VkDescriptorSetLayout, VkDescriptorPool, VkDescriptorSet, VkSampler;
typedef uint64_t VkCommandPool, VkFence, VkSemaphore, VkPipelineCache;

typedef uint32_t VkFlags, VkBool32;
typedef uint64_t VkDeviceSize;
typedef int32_t  VkResult;
typedef int32_t  VkStructureType;
typedef void (VKAPI_PTR* PFN_vkVoidFunction)(void);

#define VK_NULL_HANDLE 0
#define VK_TRUE 1
#define VK_FALSE 0
#define VK_API_VERSION_1_0 (1u << 22)
#define VK_QUEUE_FAMILY_IGNORED (~0u)
#define VK_SUBPASS_EXTERNAL (~0u)
#define VK_WHOLE_SIZE (~0ull)

/* VkResult */
#define VK_SUCCESS                  0
#define VK_TIMEOUT                  2
#define VK_SUBOPTIMAL_KHR           1000001003
#define VK_ERROR_OUT_OF_DATE_KHR    (-1000001004)

/* VkStructureType */
#define VK_STYPE_APPLICATION_INFO            0
#define VK_STYPE_INSTANCE_CREATE_INFO        1
#define VK_STYPE_DEVICE_QUEUE_CREATE_INFO    2
#define VK_STYPE_DEVICE_CREATE_INFO          3
#define VK_STYPE_SUBMIT_INFO                 4
#define VK_STYPE_MEMORY_ALLOCATE_INFO        5
#define VK_STYPE_FENCE_CREATE_INFO           8
#define VK_STYPE_SEMAPHORE_CREATE_INFO       9
#define VK_STYPE_BUFFER_CREATE_INFO          12
#define VK_STYPE_IMAGE_CREATE_INFO           14
#define VK_STYPE_IMAGE_VIEW_CREATE_INFO      15
#define VK_STYPE_SHADER_MODULE_CREATE_INFO   16
#define VK_STYPE_PIPELINE_SHADER_STAGE       18
#define VK_STYPE_PIPELINE_VERTEX_INPUT       19
#define VK_STYPE_PIPELINE_INPUT_ASSEMBLY     20
#define VK_STYPE_PIPELINE_VIEWPORT           22
#define VK_STYPE_PIPELINE_RASTERIZATION      23
#define VK_STYPE_PIPELINE_MULTISAMPLE        24
#define VK_STYPE_PIPELINE_COLOR_BLEND        26
#define VK_STYPE_PIPELINE_DYNAMIC            27
#define VK_STYPE_GRAPHICS_PIPELINE_CREATE    28
#define VK_STYPE_PIPELINE_LAYOUT_CREATE_INFO 30
#define VK_STYPE_SAMPLER_CREATE_INFO         31
#define VK_STYPE_DESCRIPTOR_SET_LAYOUT_CREATE 32
#define VK_STYPE_DESCRIPTOR_POOL_CREATE_INFO 33
#define VK_STYPE_DESCRIPTOR_SET_ALLOCATE     34
#define VK_STYPE_WRITE_DESCRIPTOR_SET        35
#define VK_STYPE_FRAMEBUFFER_CREATE_INFO     37
#define VK_STYPE_RENDER_PASS_CREATE_INFO     38
#define VK_STYPE_COMMAND_POOL_CREATE_INFO    39
#define VK_STYPE_COMMAND_BUFFER_ALLOCATE     40
#define VK_STYPE_COMMAND_BUFFER_BEGIN_INFO   42
#define VK_STYPE_RENDER_PASS_BEGIN_INFO      43
#define VK_STYPE_IMAGE_MEMORY_BARRIER        45
#define VK_STYPE_SWAPCHAIN_CREATE_INFO_KHR   1000001000
#define VK_STYPE_PRESENT_INFO_KHR            1000001001
#define VK_STYPE_WIN32_SURFACE_CREATE_KHR    1000009000

/* Formatos e espaços de cor */
#define VK_FORMAT_R8_UNORM            9
#define VK_FORMAT_B8G8R8A8_UNORM      44
#define VK_FORMAT_R32_SFLOAT          100
#define VK_FORMAT_R32G32B32A32_SFLOAT 109
#define VK_COLOR_SPACE_SRGB_NONLINEAR_KHR 0

/* VkPresentModeKHR */
#define VK_PRESENT_MODE_IMMEDIATE_KHR 0
#define VK_PRESENT_MODE_MAILBOX_KHR   1
#define VK_PRESENT_MODE_FIFO_KHR      2

/* VkImageLayout */
#define VK_IMAGE_LAYOUT_UNDEFINED                0
#define VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL 2
#define VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL 5
#define VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL     6
#define VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL     7
#define VK_IMAGE_LAYOUT_PRESENT_SRC_KHR          1000001002

/* Estágios de pipeline e acessos */
#define VK_STAGE_TOP_OF_PIPE      0x00000001u
#define VK_STAGE_VERTEX_INPUT     0x00000004u
#define VK_STAGE_FRAGMENT_SHADER  0x00000080u
#define VK_STAGE_COLOR_OUTPUT     0x00000400u
#define VK_STAGE_TRANSFER         0x00001000u
#define VK_STAGE_BOTTOM_OF_PIPE   0x00002000u
#define VK_STAGE_HOST             0x00004000u
#define VK_ACCESS_SHADER_READ     0x00000020u
#define VK_ACCESS_COLOR_READ      0x00000080u
#define VK_ACCESS_COLOR_WRITE     0x00000100u
#define VK_ACCESS_TRANSFER_READ   0x00000800u
#define VK_ACCESS_TRANSFER_WRITE  0x00001000u
#define VK_ACCESS_HOST_READ       0x00002000u

/* Usos, memória, filas */
#define VK_IMAGE_USAGE_TRANSFER_SRC     0x01u
#define VK_IMAGE_USAGE_TRANSFER_DST     0x02u
#define VK_IMAGE_USAGE_SAMPLED          0x04u
#define VK_IMAGE_USAGE_COLOR_ATTACHMENT 0x10u
#define VK_BUFFER_USAGE_TRANSFER_SRC    0x01u
#define VK_BUFFER_USAGE_TRANSFER_DST    0x02u
#define VK_BUFFER_USAGE_VERTEX_BUFFER   0x80u
#define VK_MEMORY_DEVICE_LOCAL  0x1u
#define VK_MEMORY_HOST_VISIBLE  0x2u
#define VK_MEMORY_HOST_COHERENT 0x4u
#define VK_QUEUE_GRAPHICS_BIT   0x1u
#define VK_ASPECT_COLOR         0x1u
#define VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU 2
#define VK_COMPOSITE_ALPHA_OPAQUE_KHR 0x1u

/* Diversos */
#define VK_CMD_POOL_RESET_COMMAND_BUFFER 0x2u
#define VK_CMD_BUFFER_ONE_TIME_SUBMIT    0x1u
#define VK_FENCE_CREATE_SIGNALED         0x1u
#define VK_SHADER_STAGE_VERTEX   0x01u
#define VK_SHADER_STAGE_FRAGMENT 0x10u
#define VK_DESCRIPTOR_SAMPLER        0
#define VK_DESCRIPTOR_SAMPLED_IMAGE  2
#define VK_ATTACHMENT_LOAD_OP_LOAD   0
#define VK_ATTACHMENT_LOAD_OP_CLEAR  1
#define VK_ATTACHMENT_LOAD_OP_DONT_CARE 2
#define VK_ATTACHMENT_STORE_OP_STORE 0
#define VK_ATTACHMENT_STORE_OP_DONT_CARE 1
#define VK_PRIMITIVE_TRIANGLE_STRIP  4
#define VK_BLEND_ONE                 1
#define VK_BLEND_ONE_MINUS_SRC_ALPHA 7
#define VK_DYNAMIC_STATE_VIEWPORT    0
#define VK_DYNAMIC_STATE_SCISSOR     1
#define VK_VERTEX_INPUT_RATE_INSTANCE 1
#define VK_FILTER_NEAREST            0
#define VK_SAMPLER_ADDRESS_CLAMP_TO_EDGE 2
#define VK_IMAGE_TYPE_2D             1
#define VK_IMAGE_VIEW_TYPE_2D        1
#define VK_SAMPLE_COUNT_1            1
#endif

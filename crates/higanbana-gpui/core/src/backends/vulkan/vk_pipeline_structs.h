/* vk_pipeline_structs.h — structs de render pass, pipeline gráfico e descritores
 * (ordem exata de membros da spec). */
#ifndef HG_VK_PIPELINE_STRUCTS_H
#define HG_VK_PIPELINE_STRUCTS_H
#include "vk_structs.h"

/* ---- render pass / framebuffer --------------------------------------- */
typedef struct VkAttachmentDescription {
    VkFlags flags; uint32_t format, samples, loadOp, storeOp, stencilLoadOp, stencilStoreOp, initialLayout, finalLayout;
} VkAttachmentDescription;
typedef struct VkAttachmentReference { uint32_t attachment, layout; } VkAttachmentReference;
typedef struct VkSubpassDescription {
    VkFlags flags; uint32_t pipelineBindPoint;
    uint32_t inputAttachmentCount; const VkAttachmentReference* pInputAttachments;
    uint32_t colorAttachmentCount; const VkAttachmentReference* pColorAttachments;
    const VkAttachmentReference* pResolveAttachments; const VkAttachmentReference* pDepthStencilAttachment;
    uint32_t preserveAttachmentCount; const uint32_t* pPreserveAttachments;
} VkSubpassDescription;
typedef struct VkSubpassDependency {
    uint32_t srcSubpass, dstSubpass; VkFlags srcStageMask, dstStageMask, srcAccessMask, dstAccessMask, dependencyFlags;
} VkSubpassDependency;
typedef struct VkRenderPassCreateInfo {
    VK_HEADER; VkFlags flags; uint32_t attachmentCount; const VkAttachmentDescription* pAttachments;
    uint32_t subpassCount; const VkSubpassDescription* pSubpasses;
    uint32_t dependencyCount; const VkSubpassDependency* pDependencies;
} VkRenderPassCreateInfo;
typedef struct VkFramebufferCreateInfo {
    VK_HEADER; VkFlags flags; VkRenderPass renderPass; uint32_t attachmentCount; const VkImageView* pAttachments;
    uint32_t width, height, layers;
} VkFramebufferCreateInfo;
typedef struct VkRenderPassBeginInfo {
    VK_HEADER; VkRenderPass renderPass; VkFramebuffer framebuffer; VkRect2D renderArea;
    uint32_t clearValueCount; const VkClearValue* pClearValues;
} VkRenderPassBeginInfo;

/* ---- pipeline gráfico ------------------------------------------------ */
typedef struct VkShaderModuleCreateInfo { VK_HEADER; VkFlags flags; size_t codeSize; const uint32_t* pCode; } VkShaderModuleCreateInfo;
typedef struct VkPipelineShaderStageCreateInfo {
    VK_HEADER; VkFlags flags; VkFlags stage; VkShaderModule module; const char* pName; const void* pSpecializationInfo;
} VkPipelineShaderStageCreateInfo;
typedef struct VkVertexInputBindingDescription { uint32_t binding, stride, inputRate; } VkVertexInputBindingDescription;
typedef struct VkVertexInputAttributeDescription { uint32_t location, binding, format, offset; } VkVertexInputAttributeDescription;
typedef struct VkPipelineVertexInputStateCreateInfo {
    VK_HEADER; VkFlags flags; uint32_t vertexBindingDescriptionCount; const VkVertexInputBindingDescription* pVertexBindingDescriptions;
    uint32_t vertexAttributeDescriptionCount; const VkVertexInputAttributeDescription* pVertexAttributeDescriptions;
} VkPipelineVertexInputStateCreateInfo;
typedef struct VkPipelineInputAssemblyStateCreateInfo {
    VK_HEADER; VkFlags flags; uint32_t topology; VkBool32 primitiveRestartEnable;
} VkPipelineInputAssemblyStateCreateInfo;
typedef struct VkPipelineViewportStateCreateInfo {
    VK_HEADER; VkFlags flags; uint32_t viewportCount; const VkViewport* pViewports; uint32_t scissorCount; const VkRect2D* pScissors;
} VkPipelineViewportStateCreateInfo;
typedef struct VkPipelineRasterizationStateCreateInfo {
    VK_HEADER; VkFlags flags; VkBool32 depthClampEnable, rasterizerDiscardEnable; uint32_t polygonMode;
    VkFlags cullMode; uint32_t frontFace; VkBool32 depthBiasEnable;
    float depthBiasConstantFactor, depthBiasClamp, depthBiasSlopeFactor, lineWidth;
} VkPipelineRasterizationStateCreateInfo;
typedef struct VkPipelineMultisampleStateCreateInfo {
    VK_HEADER; VkFlags flags; uint32_t rasterizationSamples; VkBool32 sampleShadingEnable; float minSampleShading;
    const uint32_t* pSampleMask; VkBool32 alphaToCoverageEnable, alphaToOneEnable;
} VkPipelineMultisampleStateCreateInfo;
typedef struct VkPipelineColorBlendAttachmentState {
    VkBool32 blendEnable; uint32_t srcColorBlendFactor, dstColorBlendFactor, colorBlendOp;
    uint32_t srcAlphaBlendFactor, dstAlphaBlendFactor, alphaBlendOp; VkFlags colorWriteMask;
} VkPipelineColorBlendAttachmentState;
typedef struct VkPipelineColorBlendStateCreateInfo {
    VK_HEADER; VkFlags flags; VkBool32 logicOpEnable; uint32_t logicOp; uint32_t attachmentCount;
    const VkPipelineColorBlendAttachmentState* pAttachments; float blendConstants[4];
} VkPipelineColorBlendStateCreateInfo;
typedef struct VkPipelineDynamicStateCreateInfo {
    VK_HEADER; VkFlags flags; uint32_t dynamicStateCount; const uint32_t* pDynamicStates;
} VkPipelineDynamicStateCreateInfo;
typedef struct VkGraphicsPipelineCreateInfo {
    VK_HEADER; VkFlags flags; uint32_t stageCount; const VkPipelineShaderStageCreateInfo* pStages;
    const VkPipelineVertexInputStateCreateInfo* pVertexInputState;
    const VkPipelineInputAssemblyStateCreateInfo* pInputAssemblyState;
    const void* pTessellationState;
    const VkPipelineViewportStateCreateInfo* pViewportState;
    const VkPipelineRasterizationStateCreateInfo* pRasterizationState;
    const VkPipelineMultisampleStateCreateInfo* pMultisampleState;
    const void* pDepthStencilState;
    const VkPipelineColorBlendStateCreateInfo* pColorBlendState;
    const VkPipelineDynamicStateCreateInfo* pDynamicState;
    VkPipelineLayout layout; VkRenderPass renderPass; uint32_t subpass;
    VkPipeline basePipelineHandle; int32_t basePipelineIndex;
} VkGraphicsPipelineCreateInfo;
typedef struct VkPushConstantRange { VkFlags stageFlags; uint32_t offset, size; } VkPushConstantRange;
typedef struct VkPipelineLayoutCreateInfo {
    VK_HEADER; VkFlags flags; uint32_t setLayoutCount; const VkDescriptorSetLayout* pSetLayouts;
    uint32_t pushConstantRangeCount; const VkPushConstantRange* pPushConstantRanges;
} VkPipelineLayoutCreateInfo;

/* ---- descritores / sampler ------------------------------------------- */
typedef struct VkSamplerCreateInfo {
    VK_HEADER; VkFlags flags; uint32_t magFilter, minFilter, mipmapMode, addressModeU, addressModeV, addressModeW;
    float mipLodBias; VkBool32 anisotropyEnable; float maxAnisotropy; VkBool32 compareEnable; uint32_t compareOp;
    float minLod, maxLod; uint32_t borderColor; VkBool32 unnormalizedCoordinates;
} VkSamplerCreateInfo;
typedef struct VkDescriptorSetLayoutBinding {
    uint32_t binding, descriptorType, descriptorCount; VkFlags stageFlags; const VkSampler* pImmutableSamplers;
} VkDescriptorSetLayoutBinding;
typedef struct VkDescriptorSetLayoutCreateInfo {
    VK_HEADER; VkFlags flags; uint32_t bindingCount; const VkDescriptorSetLayoutBinding* pBindings;
} VkDescriptorSetLayoutCreateInfo;
typedef struct VkDescriptorPoolSize { uint32_t type, descriptorCount; } VkDescriptorPoolSize;
typedef struct VkDescriptorPoolCreateInfo {
    VK_HEADER; VkFlags flags; uint32_t maxSets; uint32_t poolSizeCount; const VkDescriptorPoolSize* pPoolSizes;
} VkDescriptorPoolCreateInfo;
typedef struct VkDescriptorSetAllocateInfo {
    VK_HEADER; VkDescriptorPool descriptorPool; uint32_t descriptorSetCount; const VkDescriptorSetLayout* pSetLayouts;
} VkDescriptorSetAllocateInfo;
typedef struct VkDescriptorImageInfo { VkSampler sampler; VkImageView imageView; uint32_t imageLayout; } VkDescriptorImageInfo;
typedef struct VkWriteDescriptorSet {
    VK_HEADER; VkDescriptorSet dstSet; uint32_t dstBinding, dstArrayElement, descriptorCount, descriptorType;
    const VkDescriptorImageInfo* pImageInfo; const void* pBufferInfo; const void* pTexelBufferView;
} VkWriteDescriptorSet;
#endif

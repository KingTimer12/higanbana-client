/* vk_pipeline.c — render passes, layout (push constants + atlas) e pipelines de
 * quad/glyph a partir do SPIR-V gerado no build (naga). Blend pré-multiplicado. */
#include "vk_internal.h"
#include "error.h"
#include "text.h"
#include "quad_vs_spv.h"  /* gerado: hg_quad_vs_spv[] */
#include "quad_fs_spv.h"
#include "glyph_vs_spv.h"
#include "glyph_fs_spv.h"

#define F4 VK_FORMAT_R32G32B32A32_SFLOAT
#define F1 VK_FORMAT_R32_SFLOAT
static const VkVertexInputAttributeDescription k_quad_attrs[] = {
    { 0, 0, F4, offsetof(hg_quad, rect) },   { 1, 0, F4, offsetof(hg_quad, fill) },
    { 2, 0, F4, offsetof(hg_quad, border) }, { 3, 0, F4, offsetof(hg_quad, radius) },
    { 4, 0, F1, offsetof(hg_quad, border_width) }, { 5, 0, F1, offsetof(hg_quad, shadow_blur) },
    { 6, 0, F4, offsetof(hg_quad, shadow) },
};
static const VkVertexInputAttributeDescription k_glyph_attrs[] = {
    { 0, 0, F4, offsetof(hg_glyph_instance, rect) }, { 1, 0, F4, offsetof(hg_glyph_instance, uv) },
    { 2, 0, F4, offsetof(hg_glyph_instance, color) },
};

static VkResult make_pass(vk_device* d, uint32_t load, uint32_t initial, VkRenderPass* out) {
    VkAttachmentDescription att = { 0, VK_FORMAT_B8G8R8A8_UNORM, VK_SAMPLE_COUNT_1, load, VK_ATTACHMENT_STORE_OP_STORE,
                                    VK_ATTACHMENT_LOAD_OP_DONT_CARE, VK_ATTACHMENT_STORE_OP_DONT_CARE, initial,
                                    VK_IMAGE_LAYOUT_PRESENT_SRC_KHR };
    VkAttachmentReference ref = { 0, VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL };
    VkSubpassDescription sub = { 0, 0, 0, NULL, 1, &ref, NULL, NULL, 0, NULL };
    VkSubpassDependency dep = { VK_SUBPASS_EXTERNAL, 0, VK_STAGE_COLOR_OUTPUT | VK_STAGE_TRANSFER, VK_STAGE_COLOR_OUTPUT,
                                0, VK_ACCESS_COLOR_READ | VK_ACCESS_COLOR_WRITE, 0 };
    VkRenderPassCreateInfo ci = { VK_STYPE_RENDER_PASS_CREATE_INFO, NULL, 0, 1, &att, 1, &sub, 1, &dep };
    return d->api.vkCreateRenderPass(d->dev, &ci, NULL, out);
}

static VkResult make_layout(vk_device* d) {
    VkDescriptorSetLayoutBinding b[2] = {
        { 0, VK_DESCRIPTOR_SAMPLED_IMAGE, 1, VK_SHADER_STAGE_FRAGMENT, NULL },
        { 1, VK_DESCRIPTOR_SAMPLER, 1, VK_SHADER_STAGE_FRAGMENT, NULL },
    };
    VkDescriptorSetLayoutCreateInfo sci = { VK_STYPE_DESCRIPTOR_SET_LAYOUT_CREATE, NULL, 0, 2, b };
    VkResult r = d->api.vkCreateDescriptorSetLayout(d->dev, &sci, NULL, &d->pipeline.set_layout);
    if (r != VK_SUCCESS) return r;
    VkPushConstantRange push = { VK_SHADER_STAGE_VERTEX | VK_SHADER_STAGE_FRAGMENT, 0, 16 };
    VkPipelineLayoutCreateInfo ci = { VK_STYPE_PIPELINE_LAYOUT_CREATE_INFO, NULL, 0, 1, &d->pipeline.set_layout, 1, &push };
    return d->api.vkCreatePipelineLayout(d->dev, &ci, NULL, &d->pipeline.layout);
}

typedef struct shader_pair { const uint32_t* vs; size_t vs_size; const uint32_t* fs; size_t fs_size; } shader_pair;

static VkResult make_pipeline(vk_device* d, shader_pair sh, uint32_t stride, const VkVertexInputAttributeDescription* attrs,
                              uint32_t attr_count, VkPipeline* out) {
    VkShaderModule mod[2] = { 0, 0 };
    VkShaderModuleCreateInfo vci = { VK_STYPE_SHADER_MODULE_CREATE_INFO, NULL, 0, sh.vs_size, sh.vs };
    VkShaderModuleCreateInfo fci = { VK_STYPE_SHADER_MODULE_CREATE_INFO, NULL, 0, sh.fs_size, sh.fs };
    VkResult r = d->api.vkCreateShaderModule(d->dev, &vci, NULL, &mod[0]);
    if (r == VK_SUCCESS) r = d->api.vkCreateShaderModule(d->dev, &fci, NULL, &mod[1]);
    if (r == VK_SUCCESS) {
        VkPipelineShaderStageCreateInfo stages[2] = {
            { VK_STYPE_PIPELINE_SHADER_STAGE, NULL, 0, VK_SHADER_STAGE_VERTEX, mod[0], "vs_main", NULL },
            { VK_STYPE_PIPELINE_SHADER_STAGE, NULL, 0, VK_SHADER_STAGE_FRAGMENT, mod[1], "fs_main", NULL },
        };
        VkVertexInputBindingDescription bind = { 0, stride, VK_VERTEX_INPUT_RATE_INSTANCE };
        VkPipelineVertexInputStateCreateInfo vi = { VK_STYPE_PIPELINE_VERTEX_INPUT, NULL, 0, 1, &bind, attr_count, attrs };
        VkPipelineInputAssemblyStateCreateInfo ia = { VK_STYPE_PIPELINE_INPUT_ASSEMBLY, NULL, 0, VK_PRIMITIVE_TRIANGLE_STRIP, 0 };
        VkPipelineViewportStateCreateInfo vp = { VK_STYPE_PIPELINE_VIEWPORT, NULL, 0, 1, NULL, 1, NULL };
        VkPipelineRasterizationStateCreateInfo rs = { VK_STYPE_PIPELINE_RASTERIZATION, NULL, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1.0f };
        VkPipelineMultisampleStateCreateInfo ms = { VK_STYPE_PIPELINE_MULTISAMPLE, NULL, 0, VK_SAMPLE_COUNT_1, 0, 0, NULL, 0, 0 };
        VkPipelineColorBlendAttachmentState att = { 1, VK_BLEND_ONE, VK_BLEND_ONE_MINUS_SRC_ALPHA, 0,
                                                    VK_BLEND_ONE, VK_BLEND_ONE_MINUS_SRC_ALPHA, 0, 0xF };
        VkPipelineColorBlendStateCreateInfo cb = { VK_STYPE_PIPELINE_COLOR_BLEND, NULL, 0, 0, 0, 1, &att, { 0, 0, 0, 0 } };
        const uint32_t dyn[2] = { VK_DYNAMIC_STATE_VIEWPORT, VK_DYNAMIC_STATE_SCISSOR };
        VkPipelineDynamicStateCreateInfo ds = { VK_STYPE_PIPELINE_DYNAMIC, NULL, 0, 2, dyn };
        VkGraphicsPipelineCreateInfo ci = { VK_STYPE_GRAPHICS_PIPELINE_CREATE, NULL, 0, 2, stages, &vi, &ia, NULL, &vp, &rs,
                                            &ms, NULL, &cb, &ds, d->pipeline.layout, d->pipeline.pass_clear, 0, 0, -1 };
        r = d->api.vkCreateGraphicsPipelines(d->dev, 0, 1, &ci, NULL, out);
    }
    VK_DESTROY(d, vkDestroyShaderModule, mod[0]);
    VK_DESTROY(d, vkDestroyShaderModule, mod[1]);
    return r;
}

hg_result vk_pipeline_create(vk_device* d) {
    vk_pipeline* p = &d->pipeline;
    shader_pair quad = { hg_quad_vs_spv, sizeof hg_quad_vs_spv, hg_quad_fs_spv, sizeof hg_quad_fs_spv };
    shader_pair glyph = { hg_glyph_vs_spv, sizeof hg_glyph_vs_spv, hg_glyph_fs_spv, sizeof hg_glyph_fs_spv };
    VkResult r = make_pass(d, VK_ATTACHMENT_LOAD_OP_CLEAR, VK_IMAGE_LAYOUT_UNDEFINED, &p->pass_clear);
    if (r == VK_SUCCESS) r = make_pass(d, VK_ATTACHMENT_LOAD_OP_LOAD, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR, &p->pass_load);
    if (r == VK_SUCCESS) r = make_layout(d);
    if (r == VK_SUCCESS) r = make_pipeline(d, quad, sizeof(hg_quad), k_quad_attrs, 7, &p->quad);
    if (r == VK_SUCCESS) r = make_pipeline(d, glyph, sizeof(hg_glyph_instance), k_glyph_attrs, 3, &p->glyph);
    if (r != VK_SUCCESS) {
        vk_pipeline_release(d);
        return hg__fail(HG_ERR_DEVICE, "vulkan: falha ao criar pipeline");
    }
    return HG_OK;
}

void vk_pipeline_release(vk_device* d) {
    vk_pipeline* p = &d->pipeline;
    VK_DESTROY(d, vkDestroyPipeline, p->glyph);
    VK_DESTROY(d, vkDestroyPipeline, p->quad);
    VK_DESTROY(d, vkDestroyPipelineLayout, p->layout);
    VK_DESTROY(d, vkDestroyDescriptorSetLayout, p->set_layout);
    VK_DESTROY(d, vkDestroyRenderPass, p->pass_load);
    VK_DESTROY(d, vkDestroyRenderPass, p->pass_clear);
}

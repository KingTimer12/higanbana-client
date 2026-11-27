/* vk_user_shader.c — shaders do usuário no Vulkan: o módulo SPIR-V do naga (entradas vs_main e
 * ps_main, set 0: UBO em 0, imagem em 1, sampler em 2) vira um pipeline por shader, desenhado
 * como um quad (strip de 4 vértices, sem vertex buffer). Pipelines soltos esperam a GPU. */
#include "vk_internal.h"
#include "draw_list.h"
#include "error.h"
#include "shader.h"
#include <stdlib.h>

#define SPIRV_MAGIC 0x07230203u

typedef struct vk_user_shader {
    VkPipeline pipeline;
} vk_user_shader;

typedef struct vk_retired_pipeline {
    VkPipeline                  pipeline;
    VkFence                     retire; /* sinaliza quando a GPU não usa mais */
    struct vk_retired_pipeline* next;
} vk_retired_pipeline;

/* Drivers não validam SPIR-V: lixo pode derrubar o processo. Checagem mínima do cabeçalho. */
static int spirv_plausible(const hg_shader* sh) {
    return sh->spirv && sh->spirv_words >= 5 && sh->spirv[0] == SPIRV_MAGIC;
}

hg_result vk_user_shader_prepare(vk_device* d, hg_shader* sh) {
    if (!spirv_plausible(sh)) return hg__shader_set_error(sh, "vulkan: SPIR-V ausente ou inválido");
    hg_result r = vk_user_shaders_common(d);
    if (r != HG_OK) return r;
    vk_user_shader* gpu = calloc(1, sizeof *gpu);
    if (!gpu) return hg__fail(HG_ERR_OOM, "vulkan: sem memória");

    VkShaderModule mod = 0;
    VkShaderModuleCreateInfo mci = { VK_STYPE_SHADER_MODULE_CREATE_INFO, NULL, 0, sh->spirv_words * sizeof *sh->spirv,
                                     sh->spirv };
    VkResult vr = d->api.vkCreateShaderModule(d->dev, &mci, NULL, &mod);
    if (vr != VK_SUCCESS) {
        free(gpu);
        return hg__shader_set_error(sh, "vulkan: vkCreateShaderModule recusou o SPIR-V");
    }
    vk_pipeline_desc desc = { mod, "vs_main", mod, "ps_main", 0, NULL, 0, d->user_shaders.layout };
    vr = vk_pipeline_build(d, &desc, &gpu->pipeline);
    VK_DESTROY(d, vkDestroyShaderModule, mod);
    if (vr != VK_SUCCESS) {
        free(gpu);
        return hg__shader_set_error(sh, "vulkan: vkCreateGraphicsPipelines falhou para o shader");
    }
    sh->gpu = gpu;
    return HG_OK;
}

void vk_user_shader_release(vk_device* d, hg_shader* sh) {
    vk_user_shader* gpu = sh->gpu;
    if (!gpu) return;
    sh->gpu = NULL;
    vk_retired_pipeline* g = calloc(1, sizeof *g);
    if (g) {
        g->pipeline = gpu->pipeline;
        g->next = d->user_shaders.garbage;
        d->user_shaders.garbage = g;
    } else { /* sem memória para adiar: espera a GPU e destrói já */
        d->api.vkDeviceWaitIdle(d->dev);
        VK_DESTROY(d, vkDestroyPipeline, gpu->pipeline);
    }
    free(gpu);
    vk_user_shaders_collect(d);
}

void vk_user_shaders_collect(vk_device* d) {
    for (vk_retired_pipeline** p = &d->user_shaders.garbage; *p;) {
        vk_retired_pipeline* g = *p;
        /* Com frame gravando (pode ligar o pipeline), a marca só vale depois da submissão. */
        if (!g->retire && !d->images.recording) g->retire = vk_submit_marker(d);
        if (g->retire && d->api.vkWaitForFences(d->dev, 1, &g->retire, VK_TRUE, 0) == VK_SUCCESS) {
            *p = g->next;
            VK_DESTROY(d, vkDestroyFence, g->retire);
            VK_DESTROY(d, vkDestroyPipeline, g->pipeline);
            free(g);
        } else {
            p = &g->next;
        }
    }
}

void vk_retired_pipelines_release(vk_device* d) {
    while (d->user_shaders.garbage) {
        vk_retired_pipeline* g = d->user_shaders.garbage;
        d->user_shaders.garbage = g->next;
        VK_DESTROY(d, vkDestroyFence, g->retire);
        VK_DESTROY(d, vkDestroyPipeline, g->pipeline);
        free(g);
    }
}

int vk_user_shader_draw(vk_device* d, vk_surface* s, vk_frame* f, const hg_batch* b, const VkRect2D* sc) {
    hg_shader* sh = b->shader;
    const hg_shader_draw* sd = &b->shader_draw;
    vk_shader_frame* sf = &f->shaders;
    if (!(sd->rect.w > 0.0f) || !(sd->rect.h > 0.0f)) return 0;
    if (sf->used >= sf->capacity) return 0; /* prepare do frame falhou */
    /* Falha de criação: sh->error guardado, o quadro segue sem este lote. */
    if (hg__shader_gpu(sh) != HG_OK || !sh->gpu) return 0;
    const vk_user_shader* gpu = sh->gpu;
    VkCommandBuffer cmd = f->cmd;

    VkDescriptorSet set = sf->sets[0];
    if ((sd->flags & HG_SHADER_BACKDROP) && vk_backdrop_capture(d, s, cmd, &sd->rect)) {
        set = sf->sets[1];
        d->api.vkCmdSetScissor(cmd, 0, 1, sc); /* render pass novo: religa o estado do lote */
    }
    uint32_t offset = sf->used++ * VK_SHADER_UNIFORM_STRIDE;
    hg__shader_uniforms(sd, (float)s->width, (float)s->height, 1.0f, (float*)((char*)sf->uniforms.mapped + offset));

    d->api.vkCmdBindPipeline(cmd, 0, gpu->pipeline);
    d->api.vkCmdBindDescriptorSets(cmd, 0, d->user_shaders.layout, 0, 1, &set, 1, &offset);
    d->api.vkCmdDraw(cmd, 4, 1, 0, 0);
    return 1;
}

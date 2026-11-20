/* vk_atlas.h — atlas R8 de glyphs: criação sob demanda e transições de layout. */
#ifndef HG_VK_ATLAS_H
#define HG_VK_ATLAS_H
#include "vk_internal.h"

/* Cria imagem, view, sampler e descriptor set na primeira vez. */
hg_result vk_atlas_ensure(vk_device* d);
/* Grava barreira do layout atual para `layout` (TRANSFER_DST ou SHADER_READ_ONLY). */
void      vk_atlas_transition(vk_device* d, VkCommandBuffer cmd, uint32_t layout);
void      vk_atlas_release(vk_device* d);
#endif

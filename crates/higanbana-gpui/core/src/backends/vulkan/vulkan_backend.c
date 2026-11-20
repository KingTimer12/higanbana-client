/* vulkan_backend.c — STUB: substituído pela implementação real do backend. */
#include "backend.h"

static int vulkan_probe(hg_backend_info* info) { (void)info; return 0; }

const hg_backend_vtbl hg__vulkan_backend = {
    .kind = HG_BACKEND_VULKAN,
    .name = "Vulkan",
    .probe = vulkan_probe,
};

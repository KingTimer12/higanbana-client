/* d3d12_backend.c — STUB: substituído pela implementação real do backend. */
#include "backend.h"

static int d3d12_probe(hg_backend_info* info) { (void)info; return 0; }

const hg_backend_vtbl hg__d3d12_backend = {
    .kind = HG_BACKEND_D3D12,
    .name = "Direct3D 12",
    .probe = d3d12_probe,
};

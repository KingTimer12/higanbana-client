/* backend.c — registro e disponibilidade dos backends.
 *
 * Dois níveis, ambos cacheados por backend e seguros entre threads:
 *  - detect (barato, sem driver): hg_enumerate_backends;
 *  - probe (completo, carrega driver): só do backend pedido, em hg__backend_find.
 * Assim um app em D3D11 não paga a RAM dos drivers Vulkan/OpenGL/WGPU. */
#include "backend.h"
#include <string.h>
#include <windows.h>

#include "hgui_internal.h"

extern const hg_backend_vtbl hg__d3d11_backend;
extern const hg_backend_vtbl hg__d3d12_backend;
extern const hg_backend_vtbl hg__vulkan_backend;
extern const hg_backend_vtbl hg__opengl_backend;

#define BUILTIN_COUNT 4
#define MAX_BACKENDS  8

/* Ordem de enumeração. A ordem de fallback fica em context.c. */
static const hg_backend_vtbl* g_backends[MAX_BACKENDS] = {
    &hg__d3d11_backend,
    &hg__d3d12_backend,
    &hg__vulkan_backend,
    &hg__opengl_backend,
};
static int           g_count = BUILTIN_COUNT;
static volatile LONG g_sealed; /* 1 após a primeira consulta: registro fechado */

typedef struct check_cache {
    INIT_ONCE       once;
    volatile LONG   done;
    int             available;
    hg_backend_info info;
} check_cache;

static check_cache g_probe[MAX_BACKENDS];
static check_cache g_detect[MAX_BACKENDS];

hg_result hg__backend_register(const void* vtbl) {
    const hg_backend_vtbl* vt = vtbl;
    if (!vt || !vt->probe || g_sealed || g_count == MAX_BACKENDS) return HG_ERR_INVALID;
    for (int i = 0; i < g_count; i++)
        if (g_backends[i]->kind == vt->kind) return HG_ERR_INVALID;
    g_backends[g_count++] = vt;
    return HG_OK;
}

const hg_backend_vtbl* const* hg__backends(int* count) {
    *count = g_count;
    return (const hg_backend_vtbl* const*)g_backends;
}

static void info_init(hg_backend_info* info, const hg_backend_vtbl* vt) {
    memset(info, 0, sizeof *info);
    info->kind = vt->kind;
    strncpy_s(info->name, sizeof info->name, vt->name, _TRUNCATE);
}

static BOOL CALLBACK run_probe(PINIT_ONCE once, PVOID param, PVOID* ctx) {
    (void)once; (void)ctx;
    int i = (int)(INT_PTR)param;
    check_cache* c = &g_probe[i];
    info_init(&c->info, g_backends[i]);
    c->available = g_backends[i]->probe(&c->info);
    if (c->available && !c->info.adapter[0]) hg__detect_adapter(&c->info);
    InterlockedExchange(&c->done, 1);
    return TRUE;
}

static BOOL CALLBACK run_detect(PINIT_ONCE once, PVOID param, PVOID* ctx) {
    (void)once; (void)ctx;
    int i = (int)(INT_PTR)param;
    const hg_backend_vtbl* vt = g_backends[i];
    check_cache* c = &g_detect[i];
    info_init(&c->info, vt);
    c->available = vt->detect ? vt->detect(&c->info) : vt->probe(&c->info);
    if (c->available && !c->info.adapter[0]) hg__detect_adapter(&c->info);
    InterlockedExchange(&c->done, 1);
    return TRUE;
}

static const check_cache* check(check_cache* table, PINIT_ONCE_FN fn, int i) {
    InterlockedExchange(&g_sealed, 1);
    InitOnceExecuteOnce(&table[i].once, fn, (PVOID)(INT_PTR)i, NULL);
    return &table[i];
}

const hg_backend_vtbl* hg__backend_find(hg_backend_kind kind, hg_backend_info* info) {
    for (int i = 0; i < g_count; i++) {
        if (g_backends[i]->kind != kind) continue;
        const check_cache* c = check(g_probe, run_probe, i);
        if (!c->available) return NULL;
        if (info) *info = c->info;
        return g_backends[i];
    }
    return NULL;
}

/* Resultado do probe completo quando já existe (mais preciso); senão o detect. */
int hg_enumerate_backends(hg_backend_info* out, int max) {
    int n = 0;
    for (int i = 0; i < g_count && n < max; i++) {
        const check_cache* c = g_probe[i].done ? &g_probe[i] : check(g_detect, run_detect, i);
        if (c->available) out[n++] = c->info;
    }
    return n;
}

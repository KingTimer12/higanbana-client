/* backend.c — registro, probe cacheado e enumeração dos backends. */
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
static volatile LONG g_sealed; /* 1 após o primeiro probe: registro fechado */
#define BACKEND_COUNT g_count

typedef struct probe_cache {
    int             available;
    hg_backend_info info;
} probe_cache;

static probe_cache g_cache[MAX_BACKENDS];
static INIT_ONCE   g_probe_once = INIT_ONCE_STATIC_INIT;

hg_result hg__backend_register(const void* vtbl) {
    const hg_backend_vtbl* vt = vtbl;
    if (!vt || !vt->probe || g_sealed || g_count == MAX_BACKENDS) return HG_ERR_INVALID;
    for (int i = 0; i < g_count; i++)
        if (g_backends[i]->kind == vt->kind) return HG_ERR_INVALID;
    g_backends[g_count++] = vt;
    return HG_OK;
}

const hg_backend_vtbl* const* hg__backends(int* count) {
    *count = BACKEND_COUNT;
    return (const hg_backend_vtbl* const*)g_backends;
}

/* Probe de todos os backends uma única vez, seguro entre threads. */
static BOOL CALLBACK probe_all(PINIT_ONCE once, PVOID param, PVOID* ctx) {
    (void)once; (void)param; (void)ctx;
    InterlockedExchange(&g_sealed, 1);
    for (int i = 0; i < BACKEND_COUNT; i++) {
        probe_cache* c = &g_cache[i];
        memset(&c->info, 0, sizeof c->info);
        c->info.kind = g_backends[i]->kind;
        strncpy_s(c->info.name, sizeof c->info.name, g_backends[i]->name, _TRUNCATE);
        c->available = g_backends[i]->probe(&c->info);
    }
    return TRUE;
}

static int probe_index(int i) {
    InitOnceExecuteOnce(&g_probe_once, probe_all, NULL, NULL);
    return g_cache[i].available;
}

const hg_backend_vtbl* hg__backend_find(hg_backend_kind kind, hg_backend_info* info) {
    for (int i = 0; i < BACKEND_COUNT; i++) {
        if (g_backends[i]->kind != kind || !probe_index(i)) continue;
        if (info) *info = g_cache[i].info;
        return g_backends[i];
    }
    return NULL;
}

int hg_enumerate_backends(hg_backend_info* out, int max) {
    int n = 0;
    for (int i = 0; i < BACKEND_COUNT && n < max; i++)
        if (probe_index(i)) out[n++] = g_cache[i].info;
    return n;
}

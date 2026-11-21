/* stats.c — métricas do contexto (tempo de frame, memória, draw calls). */
#include "context.h"
#include "text.h"
#include <string.h>
#include <windows.h>

void hg__stats_frame_end(hg_context* ctx) {
    LARGE_INTEGER now, freq;
    QueryPerformanceCounter(&now);
    QueryPerformanceFrequency(&freq);
    if (ctx->last_frame_ticks)
        ctx->stats.frame_ms = (float)((double)(now.QuadPart - ctx->last_frame_ticks) * 1000.0 / (double)freq.QuadPart);
    ctx->last_frame_ticks = now.QuadPart;
}

void hg_context_stats(const hg_context* ctx, hg_stats* out) {
    if (!out) return;
    if (!ctx) { memset(out, 0, sizeof *out); return; }
    *out = ctx->stats;
    out->atlas_entries = ctx->text ? hg__atlas_entry_count(ctx->text->atlas) : 0;
}

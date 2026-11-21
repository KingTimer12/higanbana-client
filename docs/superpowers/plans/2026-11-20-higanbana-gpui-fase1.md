# higanbana-gpui Fase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Core C com janela Win32, backend D3D11, quads SDF, texto DirectWrite e exemplo Rust que abre janela e desenha.

**Architecture:** Core C11 em `crates/higanbana-gpui/core`, compilado pelo `build.rs` do crate `higanbana-gpui-sys` (crate `cc`), bindings via `bindgen`. Backends atrás de vtable `hg_backend_vtbl`; fase 1 registra só D3D11, mas registro/probe/fallback já genéricos.

**Tech Stack:** C11 (MSVC via `cc`), D3D11 + DXGI + DirectWrite (COM em C, `COBJMACROS`), HLSL compilado com `fxc` no build e embutido como bytes, Rust 2024, `bindgen` (libclang em `D:\Clang\bin`).

**Spec:** `docs/superpowers/specs/2026-11-20-higanbana-gpui-design.md`

## Global Constraints

- Nenhum link estático a DLL gráfica: `d3d11.dll`, `dxgi.dll`, `dwrite.dll` via `LoadLibraryW` + `GetProcAddress`.
- Toda função pública retorna `hg_result` (0 = `HG_OK`); mensagem em `hg_last_error()` thread-local.
- Zero `malloc` no caminho de frame: draw list usa `hg_arena` resetada em `hg_begin_frame`.
- API pública só em `core/include/hgui.h`, prefixo `hg_`.
- Default de backend: D3D11. Ordem de fallback: D3D11 → OpenGL → WGPU (fase 1: só D3D11 existe).
- Commits sem trailer Co-Authored-By.

## Review Focus

- Janela minimizada (tamanho 0×0): resize não pode criar swapchain 0 nem crashar → `resize` ignora 0, frame pulado. Teste em Task 4.
- Backend pedido indisponível (ex.: Vulkan): `hg_context_create` cai no fallback e reporta backend real. Teste em Task 3.
- Draw list além da capacidade inicial da arena: cresce por bloco novo, sem realocar/invalidar ponteiros anteriores. Teste em Task 1.
- Texto com caracteres não-ASCII/japoneses (títulos de anime): UTF-8 → UTF-16 correto, glyph faltando não crasha. Teste em Task 5.
- Atlas cheio: eviction LRU em vez de falha/crescimento infinito. Teste em Task 2.

---

### Task 1: Scaffold + arena + draw list

**Files:**
- Create: `crates/higanbana-gpui/core/include/hgui.h`, `core/src/arena.c`, `core/src/arena.h`, `core/src/draw_list.c`, `core/src/draw_list.h`, `core/src/error.c`
- Create: `crates/higanbana-gpui-sys/{Cargo.toml,build.rs,src/lib.rs}`, `crates/higanbana-gpui-sys/tests/core.rs`
- Modify: `Cargo.toml` (workspace members + `cc`, `bindgen` em workspace deps), `crates/higanbana-gpui/Cargo.toml` (dep `higanbana-gpui-sys`), `crates/higanbana-gpui/src/lib.rs` (remover `add`)

**Interfaces:**
- Produces (hgui.h):
  - `typedef int32_t hg_result; #define HG_OK 0, HG_ERR_UNAVAILABLE 1, HG_ERR_DEVICE 2, HG_ERR_OOM 3, HG_ERR_INVALID 4`
  - `const char* hg_last_error(void);`
  - `typedef struct { float x, y, w, h; } hg_rect; typedef struct { float r, g, b, a; } hg_color;`
  - `typedef struct { hg_rect rect; hg_color fill; hg_color border; float radius[4]; float border_width; float shadow_blur; hg_color shadow; } hg_quad;`
  - `hg_draw_list* hg_draw_list_create(void); void hg_draw_list_destroy(hg_draw_list*); void hg_draw_list_reset(hg_draw_list*); hg_result hg_draw_quad(hg_draw_list*, const hg_quad*); hg_result hg_push_clip(hg_draw_list*, hg_rect); void hg_pop_clip(hg_draw_list*); uint32_t hg_draw_list_quad_count(const hg_draw_list*);`
  - Interno: `hg_arena` com blocos encadeados de 64 KiB; `void* hg_arena_alloc(hg_arena*, size_t size, size_t align)`; `void hg_arena_reset(hg_arena*)` (mantém blocos).
- `build.rs`: `cc::Build` compila `core/src/**/*.c` com `/std:c11`, include `core/include`; `bindgen` sobre `hgui.h`, allowlist `hg_.*`, `cargo:rerun-if-changed=../higanbana-gpui/core`.

- [ ] **Step 1: Testes falhando em `tests/core.rs`**

```rust
#[test] fn draw_list_counts_quads() { /* create, 3x hg_draw_quad, assert count==3, reset, assert count==0 */ }
#[test] fn draw_list_grows_past_first_block() { /* 10_000 quads (>64KiB), count==10_000, todos HG_OK */ }
#[test] fn clip_pop_on_empty_stack_is_noop() { /* hg_pop_clip sem push não crasha */ }
#[test] fn null_quad_is_invalid() { /* hg_draw_quad(dl, null) == HG_ERR_INVALID e hg_last_error não vazio */ }
```

- [ ] **Step 2:** `cargo test -p higanbana-gpui-sys` → falha de compilação (símbolos inexistentes).
- [ ] **Step 3:** Implementar header, arena, draw_list, error (`__declspec(thread)` para last error).
- [ ] **Step 4:** `cargo test -p higanbana-gpui-sys` → 4 passed.
- [ ] **Step 5:** Commit `feat(gpui): core C com arena e draw list`.

### Task 2: Atlas com LRU

**Files:** Create `core/src/atlas.c`, `core/src/atlas.h`; Test `crates/higanbana-gpui-sys/tests/atlas.rs` (atlas exposto em `hgui.h` sob `HG_INTERNAL_TESTS` definido no build de teste — ou exportado como `hg__atlas_*`, escolha do implementador, documentar no header).

**Interfaces:**
- Produces: `hg_atlas* hg__atlas_create(uint32_t w, uint32_t h); void hg__atlas_destroy(hg_atlas*); hg_result hg__atlas_get_or_insert(hg_atlas*, uint64_t key, uint32_t w, uint32_t h, uint64_t frame, hg_rect* out_uv, int* out_inserted);` — skyline packer; ao falhar, evicta entradas com `last_used_frame` mais antigo até caber; se nem atlas vazio cabe → `HG_ERR_OOM`. Callback `on_evict(key)` opcional para quem guarda cache.

- [ ] **Step 1: Testes**

```rust
#[test] fn same_key_returns_same_uv_without_insert() {}
#[test] fn full_atlas_evicts_least_recent() { /* 256x256, insere 16 de 64x64 frames 0..15, toca key 0 no frame 16, insere novo: key 1 evictada, key 0 continua */ }
#[test] fn oversize_entry_is_oom() { /* 512x512 em atlas 256 → HG_ERR_OOM */ }
```
- [ ] **Step 2:** rodar → falha. **Step 3:** implementar. **Step 4:** 3 passed.
- [ ] **Step 5:** Commit `feat(gpui): atlas com skyline e eviction LRU`.

### Task 3: Registro de backends, probe e fallback

**Files:** Create `core/src/backend.c`, `core/src/backend.h`, `core/src/context.c`; Test `tests/backend.rs`.

**Interfaces:**
- Produces (hgui.h):
  - `typedef enum { HG_BACKEND_AUTO=0, HG_BACKEND_D3D11, HG_BACKEND_D3D12, HG_BACKEND_VULKAN, HG_BACKEND_OPENGL, HG_BACKEND_WGPU } hg_backend_kind;`
  - `typedef struct { hg_backend_kind kind; char name[32]; char adapter[128]; } hg_backend_info;`
  - `int hg_enumerate_backends(hg_backend_info* out, int max);` — só os cujo `probe()` passa.
  - `hg_result hg_context_create(hg_backend_kind preferred, hg_context** out); void hg_context_destroy(hg_context*); hg_backend_kind hg_context_backend(const hg_context*);`
- Interno: `typedef struct hg_backend_vtbl { hg_backend_kind kind; const char* name; int (*probe)(hg_backend_info*); hg_result (*create)(hg_context*); void (*destroy)(hg_context*); hg_result (*resize)(hg_context*, uint32_t w, uint32_t h); hg_result (*begin_frame)(hg_context*, hg_color clear); hg_result (*submit)(hg_context*, const hg_draw_list*); hg_result (*end_frame)(hg_context*, int vsync); hg_result (*upload_texture)(hg_context*, uint32_t id, uint32_t x, uint32_t y, uint32_t w, uint32_t h, const void* rgba); } hg_backend_vtbl;` Tabela estática; fase 1 só D3D11 (Task 4). Probe resultado cacheado.
- Fallback: preferred se disponível, senão D3D11 → OpenGL → WGPU; nenhum → `HG_ERR_UNAVAILABLE`.
- Context criado sem janela é "headless" (só device); janela anexada em Task 4.

- [ ] **Step 1: Testes**

```rust
#[test] fn enumerate_does_not_crash_and_lists_d3d11_on_windows() { /* n>=1, out[0].kind==D3D11 */ }
#[test] fn unavailable_preferred_falls_back() { /* create(VULKAN) == OK e backend()==D3D11 */ }
#[test] fn auto_picks_d3d11() {}
```
- [ ] **Step 2–4:** falha → implementar (com stub D3D11 probe que testa `LoadLibraryW(L"d3d11.dll")`) → 3 passed.
- [ ] **Step 5:** Commit `feat(gpui): registro de backends com probe e fallback`.

### Task 4: Janela Win32 + backend D3D11 + quads SDF

**Files:** Create `core/src/window_win32.c`, `core/src/input.c`, `core/src/backends/d3d11.c`, `core/src/shaders/quad.hlsl`; Modify `build.rs` (compilar HLSL com `fxc /T vs_5_0` e `ps_5_0` para `OUT_DIR`, gerar `quad_vs.h`/`quad_ps.h` como arrays de bytes); Test `tests/window.rs`.

**Interfaces:**
- Produces (hgui.h):
  - `typedef struct { const char* title_utf8; uint32_t width, height; int resizable; } hg_window_desc;`
  - `hg_result hg_window_create(hg_context*, const hg_window_desc*, hg_window** out); void hg_window_destroy(hg_window*);`
  - `typedef enum { HG_EVENT_NONE, HG_EVENT_CLOSE, HG_EVENT_RESIZE, HG_EVENT_MOUSE_MOVE, HG_EVENT_MOUSE_DOWN, HG_EVENT_MOUSE_UP, HG_EVENT_SCROLL, HG_EVENT_KEY_DOWN, HG_EVENT_KEY_UP, HG_EVENT_CHAR } hg_event_kind;` + `hg_event` com union (x,y,button,key,mods,codepoint,width,height,dpi_scale).
  - `int hg_window_poll_event(hg_window*, hg_event* out);` — 0 quando fila vazia. `hg_result hg_window_wait_event(hg_window*, hg_event* out);` (bloqueia, para app ocioso não gastar CPU).
  - `hg_result hg_frame_begin(hg_window*, hg_color clear); hg_result hg_frame_submit(hg_window*, const hg_draw_list*); hg_result hg_frame_end(hg_window*, int vsync);`
  - `float hg_window_dpi_scale(const hg_window*);` — Per-Monitor V2 DPI aware.
- D3D11: swapchain flip-discard, `DXGI_FORMAT_B8G8R8A8_UNORM`, 2 buffers; instance buffer dinâmico que só cresce (potência de 2) e é reutilizado; 1 draw instanciado por lote de quads entre clips (scissor rect).
- Shader quad: SDF de retângulo arredondado com raio por canto, borda, sombra gaussiana aproximada (erf), AA por `fwidth`.
- Resize 0×0 (minimizado): não chama `ResizeBuffers`; `hg_frame_begin` retorna `HG_OK` e frame vira no-op.

- [ ] **Step 1: Testes** (`#[ignore]` não: rodam em Windows com GPU/WARP; se `create` falhar com `HG_ERR_DEVICE`, D3D11 usa `D3D_DRIVER_TYPE_WARP` como fallback interno)

```rust
#[test] fn window_renders_quads_frames() { /* janela 320x240 oculta? não — visível ok; 3 frames com 100 quads, todos HG_OK */ }
#[test] fn zero_size_resize_is_noop() { /* chama interno hg__window_inject_resize(w,0,0) exportado para teste; frame_begin/submit/end == OK */ }
#[test] fn close_event_after_destroy_request() { /* PostMessage WM_CLOSE via hg__window_request_close; poll retorna HG_EVENT_CLOSE */ }
```
- [ ] **Step 2–4:** falha → implementar → 3 passed.
- [ ] **Step 5:** Commit `feat(gpui): janela win32 e backend d3d11 com quads sdf`.

### Task 5: Texto DirectWrite + atlas de glyph

**Files:** Create `core/src/text_dwrite.c`, `core/src/text.h`; Modify `d3d11.c` (segunda pipeline: glyph quad amostrando atlas R8 2048×2048), `shaders/glyph.hlsl`, `hgui.h`; Test `tests/text.rs`.

**Interfaces:**
- Produces (hgui.h):
  - `typedef uint32_t hg_font_id;` `hg_result hg_font_load(hg_context*, const char* family_utf8, float weight, int italic, hg_font_id* out);`
  - `typedef struct { float width, height, baseline; } hg_text_metrics;`
  - `hg_result hg_text_measure(hg_context*, hg_font_id, float size_px, const char* utf8, size_t len, float max_width, hg_text_metrics* out);`
  - `hg_result hg_draw_text(hg_draw_list*, hg_font_id, float size_px, float x, float y, float max_width, hg_color color, const char* utf8, size_t len);` — draw list guarda texto na arena; shaping/rasterização no submit.
- Raster: `IDWriteGlyphRunAnalysis` `DWRITE_TEXTURE_ALIASED_1x1`→ usar `CLEARTYPE_3x1` convertido para cobertura média em R8; key atlas = hash(font, size*4 arredondado, glyph index, subpixel x/4). Usa `hg__atlas_*` da Task 2 com frame atual.
- Fallback de fonte do sistema (`IDWriteFontFallback`) para CJK.

- [ ] **Step 1: Testes**

```rust
#[test] fn measure_ascii_nonzero_and_monotonic() { /* "A" < "AAAA" em largura */ }
#[test] fn measure_japanese_title() { /* "葬送のフリーレン" width>0, OK */ }
#[test] fn wraps_with_max_width() { /* texto longo com max_width=100 → height > 1 linha */ }
#[test] fn invalid_utf8_is_invalid_not_crash() { /* bytes [0xff,0xfe] → HG_ERR_INVALID */ }
#[test] fn draws_text_in_window() { /* 3 frames com texto, OK */ }
```
- [ ] **Step 2–4:** falha → implementar → 5 passed.
- [ ] **Step 5:** Commit `feat(gpui): texto via directwrite com atlas de glyph`.

### Task 6: Exemplo + métricas

**Files:** Create `crates/higanbana-gpui-sys/examples/hello.rs`; Create `hg_stats` em `hgui.h`/`context.c`.

**Interfaces:**
- Produces: `typedef struct { float frame_ms; uint64_t gpu_bytes; uint64_t arena_bytes; uint32_t draw_calls; uint32_t atlas_entries; } hg_stats; void hg_context_stats(const hg_context*, hg_stats* out);`
- Exemplo: aceita `--backend d3d11|auto`, lista backends no stdout, abre janela 960×600, desenha cards arredondados com sombra + título japonês, usa `hg_window_wait_event` (ocioso não redesenha), imprime stats a cada 120 frames no resize/animação.

- [ ] **Step 1: Teste** `#[test] fn stats_after_frame_have_draw_calls()` em `tests/window.rs`: após 1 frame com quads e texto, `draw_calls >= 2`, `arena_bytes > 0`.
- [ ] **Step 2–4:** falha → implementar → passa; `cargo run -p higanbana-gpui-sys --example hello` abre janela (verificação visual manual pelo usuário).
- [ ] **Step 5:** Commit `feat(gpui): exemplo hello e estatísticas de frame`.

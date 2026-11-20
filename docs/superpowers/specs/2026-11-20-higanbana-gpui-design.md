# higanbana-gpui — Design

## Intent

UI framework próprio para o Higanbana com foco em **máximo desempenho e mínimo de RAM**.
Core em C (chamável por qualquer linguagem), componentes prontos em Rust no estilo shadcn.
O usuário escolhe em runtime o backend gráfico entre os que a máquina suporta.

## Goals

- Backends: **D3D12, D3D11, Vulkan, OpenGL, WGPU**, todos selecionáveis em runtime.
- Detecção automática: cada backend é sondado (DLL carrega + device cria). Indisponíveis
  não aparecem na lista (ex.: Vulkan sem `vulkan-1.dll`, Metal nunca no Windows).
- Fallback automático se o backend escolhido falhar: D3D11 → OpenGL → WGPU.
- RAM baixa: atlas únicos de glyph/imagem com eviction LRU, buffers de GPU reaproveitados
  por frame, zero alocação por frame no caminho quente (arenas resetadas a cada frame).
- Desempenho: renderização em lote por instância (1 draw call por tipo de primitiva/atlas),
  redraw só quando sujo, vsync configurável.
- API Rust segura + biblioteca de componentes estilo shadcn (variants, tamanhos, tema por tokens).

## Non-goals (primeira versão)

- macOS/Linux (arquitetura permite; não implementado agora). Metal fora.
- Acessibilidade (UIA), IME completo, rich text editing. Planejado depois.
- Substituir GPUI no `higanbana-desktop` agora — migração é decisão posterior.

## Anti-goals

- Backend que "existe" na lista mas crasha ao selecionar.
- Dependência obrigatória de runtime externo (Vulkan SDK, wgpu DLL) para o app abrir.
- Alocação/cópia por frame que faça RAM crescer com o tempo.

## Approaches

- **A. Core em C + bindings Rust (escolhido pelo usuário).** C11, vtable por backend,
  ABI estável chamável por qualquer linguagem.
- B. Core em Rust exportando ABI C via cbindgen — mesma perf, mais segurança; descartado
  porque usuário quer core em C.
- C. Fork do GPUI com novos backends — descartado: renderer acoplado, merges caros.

## Arquitetura

```
crates/higanbana-gpui/
  core/                      # C11, compilado por build.rs (crate cc)
    include/hgui.h           # única API pública C
    src/
      context.c  window_win32.c  input.c  arena.c  atlas.c
      text_dwrite.c          # shaping/raster via DirectWrite
      draw_list.c            # acumula primitivas do frame
      backend.c              # registro, probe, seleção, fallback
      backends/
        d3d11.c  d3d12.c  vulkan.c  opengl.c  wgpu.c
      shaders/               # HLSL (D3D), SPIR-V pré-compilado (Vulkan), GLSL (GL), WGSL (wgpu)
  higanbana-gpui-sys/        # bindgen do hgui.h (FFI cru)
  src/                       # crate higanbana-gpui: API Rust segura
    app.rs window.rs backend.rs
    layout/   (taffy)
    element/  (div, text, image, scroll)
    theme/    (tokens: cores, raio, espaçamento, tipografia)
    components/ (button, input, card, dialog, tabs, badge, avatar,
                 select, checkbox, switch, slider, tooltip, toast, separator, skeleton)
```

### Core C

- `hg_backend_desc hg_enumerate_backends(hg_backend_info* out, int max)` — retorna os
  disponíveis com nome, versão, GPU.
- `hg_context_create(hg_backend_kind preferred)`; `hg_context_switch_backend` recria
  device/swapchain e reupload de atlas sem fechar janela.
- Vtable `hg_backend_vtbl { probe, create, destroy, resize, begin_frame, submit, end_frame,
  upload_texture, free_texture }`.
- Carregamento dinâmico: `d3d12.dll`, `d3d11.dll`, `vulkan-1.dll`, `opengl32.dll`,
  `wgpu_native.dll` via `LoadLibrary`. Nenhum link estático obrigatório.
- Primitivas: quad (cor/gradiente, raio por canto, borda, sombra via SDF), glyph, imagem,
  clip rect. Instanciadas, um buffer por tipo, ordenadas por camada.
- Erros: todo retorno `hg_result`; `hg_last_error()` thread-local.
- Threads: janela/UI em uma thread; upload de imagem pode vir de qualquer thread via fila.

### Rust

- `-sys`: bindgen gerado em build, sem bindings commitados.
- Camada segura: handles com `Drop`, lifetimes impedem uso após destroy.
- Modelo de UI: modo retido leve — árvore de elementos reconstruída por `render()` só quando
  estado muda; layout via `taffy`; diff mínimo para evitar redraw.
- Componentes: API builder estilo GPUI (`Button::new("Salvar").variant(Variant::Outline).size(Size::Sm)`),
  tema por tokens no estilo shadcn (background, foreground, primary, muted, accent,
  destructive, border, ring, radius).

## Fases

1. Core + janela Win32 + D3D11 + quads + texto DirectWrite + exemplo.
2. D3D12, Vulkan, OpenGL, WGPU + troca em runtime + fallback.
3. Crate Rust segura + layout + elementos base.
4. Componentes shadcn-like + tema + exemplo galeria.

## Testes

- C: testes unitários de arena/atlas/draw_list (sem GPU).
- Rust: testes de layout e componentes sem GPU; teste de probe que só verifica que a
  enumeração não crasha.
- Exemplo `gallery` para validação visual por backend.
- Medição: RAM/frame time logados no exemplo para comparar backends.

## Calls do builder (decididas por mim, revisáveis)

- C11, MSVC via crate `cc`; shaders pré-compilados e embutidos (sem compilar em runtime).
- Texto: DirectWrite (nativo, sem FreeType/HarfBuzz no Windows).
- WGPU via `wgpu-native` DLL carregada dinamicamente; se ausente, backend some.
- Layout em Rust (`taffy`), não no C — C fica só render/janela/input/texto.
- Default de backend: D3D11 (melhor compat./RAM no Windows), salvo nas configurações do app.

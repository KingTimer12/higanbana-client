# Shaders do usuário + README — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Permitir que quem usa a `higanbana-gpui` escreva uma função WGSL `shade` e a use como fundo de uma `div` (com backdrop, `time`, parâmetros) nos 5 backends, com um `LiquidGlass` de exemplo e um README completo da crate.

**Architecture:** Rust monta o módulo WGSL (prelúdio + `shade` do usuário + entradas), valida e traduz com `naga` em tempo de execução para HLSL/GLSL/SPIR-V e guarda o WGSL. O núcleo C recebe todas as traduções num `hg_shader` (espelha `hg_image`: dados na CPU, objeto de GPU criado sob demanda no backend ativo e liberado na troca de backend). `hg_draw_shader` fecha a camada, vira lote próprio e abre camada nova; antes do lote, o backend copia o retângulo de trás para uma textura de backdrop.

**Tech Stack:** Rust 2024, C11, `naga =30.0.1` (runtime: `wgsl-in`, `hlsl-out`, `glsl-out`, `spv-out`), D3D11/D3D12 (`D3DCompile` de `d3dcompiler_47.dll`, carregada dinamicamente), OpenGL 3.3 core, Vulkan, `wgpu =30.0.1`.

**Spec:** `docs/superpowers/specs/2026-11-27-gpui-shaders-design.md`

## Global Constraints

- 5 backends: D3D11, D3D12, OpenGL, Vulkan, WGPU. Backend indisponível na máquina → teste dele pula (padrão dos testes existentes, ex.: `higanbana-gpui-sys/tests/image_wgpu.rs`).
- Sem shader na tela = caminho de desenho idêntico ao atual (nenhum custo novo por quadro).
- Até **16** `vec4` de parâmetros por desenho.
- Cor devolvida por `shade` é **não pré-multiplicada**; a lib pré-multiplica, aplica opacidade, SDF dos cantos (anti-alias), clip e blend como os quads.
- HLSL: perfil `vs_5_0`/`ps_5_0`, entradas `vs_main`/`ps_main` (igual `build/shaders.rs`). GLSL: `#version 330 core`.
- Bindings do módulo (fixos, iguais em todos os backends): `@group(0) @binding(0)` uniform `HgShaderUniforms`; `@binding(1)` `texture_2d<f32>` backdrop; `@binding(2)` `sampler` linear clamp.
- Um arquivo por responsabilidade; arquivos C por backend novos (`*_user_shader.c`), sem inchar `*_render.c` além do laço.
- Medir CPU e RAM (antes/depois) na Task 9.
- Commits sem trailer `Co-Authored-By`. Sem push. Branch `feat/gpui-shaders`.
- Suite: `cargo test --workspace` (na raiz). Feature de vídeo: `cargo test -p higanbana-gpui --features video` quando tocar o laço do app.

## Review Focus

1. Troca de backend em tempo de execução com shader na tela → recompila no backend novo, sem vazar objeto do antigo, sem magenta.
2. Shader com backdrop encostado na borda da janela / maior que a janela / retângulo negativo após clip → cópia recortada, sem erro de API gráfica.
3. Redimensionar a janela com vidro na tela → textura de backdrop recriada, sem ler fora.
4. Janela minimizada com shader `animated` → laço volta a bloquear (`wait_event`), CPU ~0.
5. Vários shaders sobrepostos no mesmo quadro (vidro sobre vidro) → o de cima vê o de baixo no backdrop.

---

### Task 1: `hg_shader` no núcleo C (sem backend)

**Files:**
- Create: `core/src/shader/shader.h`, `core/src/shader/shader.c`, `core/src/shader/draw_shader.c`
- Modify: `core/include/hgui.h`, `core/src/draw_list.h` (lote ganha `shader`), `core/src/backend.h` (vtable), `core/src/context_switch.c` e `core/src/context.c` (liberar GPU na troca/destruição), `higanbana-gpui-sys/build/core.rs` se a lista de `.c` for explícita
- Test: `higanbana-gpui-sys/tests/shader_core.rs`

**Interfaces:**
- Produces (`hgui.h`):
```c
typedef struct hg_shader hg_shader;
typedef struct hg_shader_code {
    const char* hlsl;  const char* glsl;  const char* wgsl;   /* UTF-8, terminadas em 0 */
    const uint32_t* spirv; size_t spirv_words;
} hg_shader_code;
hg_result   hg_shader_create(hg_context* ctx, const hg_shader_code* code, hg_shader** out); /* copia tudo */
void        hg_shader_destroy(hg_shader* sh);
const char* hg_shader_error(const hg_shader* sh); /* erro do compilador nativo no backend atual; NULL = ok */
#define HG_SHADER_BACKDROP 1u
#define HG_SHADER_PARAMS 16
typedef struct hg_shader_draw {
    hg_rect rect; float radius[4]; float time; float scale; float opacity;
    float params[HG_SHADER_PARAMS][4]; uint32_t flags;
} hg_shader_draw;
hg_result hg_draw_shader(hg_draw_list* dl, hg_shader* sh, const hg_shader_draw* d);
```
- `struct hg_shader { hg_context* ctx; char *hlsl, *glsl, *wgsl; uint32_t* spirv; size_t spirv_words; void* gpu; char* error; prev/next }` — lista no contexto como `hg_image`.
- `hg_batch` ganha `hg_shader* shader; hg_shader_draw shader_draw;` (lote de shader não tem quads/imagens/texto). `hg_draw_shader` = `hg_draw_list_layer` + lote de shader + `hg_draw_list_layer`; herda o clip atual.
- Vtable ganha opcionais: `hg_result (*shader_prepare)(hg_context*, hg_shader*)` (cria `sh->gpu`; em falha do compilador grava `sh->error` e devolve `HG_ERR_DEVICE`) e `void (*shader_release)(hg_context*, hg_shader*)`. Backend com `NULL` → lote de shader é pulado.
- `hg__shaders_release_gpu(ctx)` chamado onde `hg__images_release_gpu` é chamado.

- [ ] **Step 1: Testes que falham** — `hg_shader_create` com códigos falsos (`"x"`) → `HG_OK`; `hg_shader_error` → `NULL`; `hg_draw_shader` entre dois `hg_draw_quad` gera 3 lotes não vazios na ordem quad / shader / quad (expor contagem via função de teste `hg__draw_list_batch_count` ou ler a estrutura pelos bindings); `hg_draw_shader` com `sh == NULL` → `HG_ERR_INVALID`; destruir o contexto com shader vivo não vaza (sem crash no destroy).
- [ ] **Step 2: Rodar** `cargo test -p higanbana-gpui-sys --test shader_core` → FAIL (símbolos não existem).
- [ ] **Step 3: Implementar.** Todos os laços de render existentes (`*_render.c`, `wgpu_backend/render.rs`) devem **pular** lote com `shader != NULL` por enquanto.
- [ ] **Step 4: Rodar** o teste → PASS; `cargo test --workspace` → PASS.
- [ ] **Step 5: Commit** `feat(gpui-core): objeto hg_shader e desenho na draw list`

### Task 2: Compositor e tradutor WGSL em Rust

**Files:**
- Create: `src/gfx/shader/mod.rs`, `src/gfx/shader/prelude.wgsl`, `src/gfx/shader/entry.wgsl`, `src/gfx/shader/compose.rs`, `src/gfx/shader/translate.rs`, `src/gfx/shader/error.rs`, `src/gfx/shader/handle.rs`
- Modify: `Cargo.toml` da `higanbana-gpui` (feature `shaders` padrão; `naga` runtime opcional), `src/gfx/mod.rs`, `src/gfx/draw.rs` (`DrawList::shader`), `src/lib.rs` (reexport)
- Test: `src/gfx/shader/tests.rs`

**Interfaces:**
- Produces:
```rust
pub struct Shader { /* Rc: fonte, traduções, flags, origem do time, Cell<Option<ShaderError>> */ }
impl Shader {
    pub fn wgsl(src: &str) -> Result<Shader, ShaderError>;
    pub fn backdrop(self, on: bool) -> Self;
    pub fn animated(self, on: bool) -> Self;
    pub fn last_error(&self) -> Option<ShaderError>;
}
#[derive(Debug, Clone, PartialEq)]
pub enum ShaderError { Parse { line: u32, column: u32, message: String }, Validate { line: u32, column: u32, message: String }, Native(String), Unsupported }
pub type ShaderParams = [[f32; 4]; 16];
#[macro_export] macro_rules! params { ($($v:expr),* $(,)?) => { /* cada $v: Into<[f32;4]>; resto zero */ } }
impl<'a> DrawList<'a> { pub fn shader(&mut self, gpu: &'a GpuShader<'_>, d: &ShaderDraw) -> Result<()>; }
pub struct GpuShader<'c> /* dono do *mut hg_shader, criado por Context */;
impl Context { pub fn shader(&self, s: &Shader) -> Result<GpuShader<'_>>; }
```
- `compose(user: &str) -> (String, u32 /* linhas do prelúdio */)`; `translate(module) -> Translations { hlsl, glsl, spirv: Vec<u32>, wgsl }`; erros do naga com `span` → linha/coluna **menos** as linhas do prelúdio.
- `prelude.wgsl` declara `ShadeInput` (campos do spec), `HgShaderUniforms` (`rect: vec4`, `radius: vec4`, `time_scale_opacity_flags: vec4`, `viewport: vec4`, `params: array<vec4<f32>,16>`), bindings fixos, `param(i)`, `backdrop(screen_px)`, `backdrop_blur(screen_px, r)`, `rounded_sdf(px, size, radius)`. `entry.wgsl`: `vs_main` (quad de 4 vértices por `vertex_index`, posição em NDC a partir de `rect`/`viewport`) e `ps_main`/`fs_main` (chama `shade`, aplica SDF, opacidade, pré-multiplica).
- Eixo Y: `screen` sempre com origem no topo-esquerda; o GLSL recebe um uniform/define do backend para inverter a leitura do backdrop no OpenGL (Task 5 decide; Task 2 deixa `viewport.w` = sinal do Y).
- Feature `shaders` desligada: `Shader::wgsl` → `Err(ShaderError::Unsupported)`; nada mais muda.

- [ ] **Step 1: Testes que falham**
  - `Shader::wgsl("fn shade(p: ShadeInput) -> vec4<f32> { return vec4(1.0, 0.0, 0.0, 1.0); }")` → `Ok`; traduções não vazias para HLSL (contém `vs_main` e `ps_main`), GLSL (começa com `#version 330`), SPIR-V (palavra mágica `0x07230203`).
  - erro de sintaxe na linha 3 do código do usuário → `Parse { line: 3, .. }`.
  - função `shade` ausente → `Validate`/`Parse` com mensagem citando `shade`.
  - `params![[1.,0.,0.,1.]]` → índice 0 preenchido, 1..15 zero.
  - `#[cfg(not(feature = "shaders"))]` → `Unsupported`.
- [ ] **Step 2: Rodar** `cargo test -p higanbana-gpui shader` → FAIL.
- [ ] **Step 3: Implementar.**
- [ ] **Step 4: Rodar** → PASS; `cargo test -p higanbana-gpui --no-default-features --features wgpu shader` → PASS (caminho `Unsupported`).
- [ ] **Step 5: Commit** `feat(gpui): Shader WGSL traduzido pelo naga em tempo de execução`

### Task 3: D3D11

**Files:**
- Create: `core/src/backends/d3d11/d3d11_user_shader.c` (compilar via `D3DCompile`, pipeline, constant buffer), `core/src/backends/d3d11/d3d11_backdrop.c` (textura por superfície + cópia)
- Modify: `d3d11_loader.c` (carrega `d3dcompiler_47.dll` sob demanda), `d3d11_internal.h`, `d3d11_render.c` (lote de shader no laço), `d3d11_backend.c` (vtable), `d3d11_swapchain.c` (resize libera backdrop)
- Test: `tests/shader_backends.rs` (na `higanbana-gpui`, padrão de `tests/gpu.rs`, parametrizado por backend)

**Interfaces:**
- Consumes: Task 1 (vtable `shader_prepare/release`), Task 2 (`Context::shader`, `DrawList::shader`).
- Produces: helpers de teste reutilizados pelas Tasks 4–7: `fn run_case(kind: Backend, case: Case) -> Option<Vec<u32>>` (None = backend indisponível) e os casos abaixo.

- [ ] **Step 1: Testes que falham** (D3D11), janela 320×240, leitura de pixel:
  - **sólido:** shade devolve vermelho → pixel central `0xFFFF0000` (BGRA conforme `tests/gpu.rs`).
  - **cantos:** raio 40 → pixel (rect.x+1, rect.y+1) = cor de fundo.
  - **ordem:** quad azul → shader verde meio transparente → quad branco pequeno por cima: centro branco, borda verde-azulada.
  - **backdrop:** quad vermelho atrás; shade = `vec4(1.0) - backdrop(p.screen)` com `.backdrop(true)` → ciano.
  - **backdrop desligado:** mesmo shader do caso backdrop sem `.backdrop(true)` → `backdrop()` transparente → branco.
  - **opacidade:** opacity 0.5 sobre fundo preto → canal ≈ 0x80 (±2).
  - **params:** `param(0)` como cor → pixel igual ao parâmetro.
  - **erro nativo:** HLSL inválido injetado direto via `hg_shader_create` → `hg_shader_error` não nulo, nada desenhado, sem crash.
  - **borda/resize (Review Focus 2,3):** vidro com backdrop em (-50,-50,200,200) e depois após `resize` para 640×480 → sem erro.
  - **sobreposição (Review Focus 5):** dois vidros com backdrop sobrepostos → o de cima inverte o resultado do de baixo.
- [ ] **Step 2: Rodar** `cargo test -p higanbana-gpui --test shader_backends d3d11` → FAIL (nada desenhado).
- [ ] **Step 3: Implementar.** Constant buffer dinâmico por desenho (`Map WRITE_DISCARD`); cópia `CopySubresourceRegion` só do retângulo do lote expandido por `max(param blur)`= margem fixa de 64 px, recortado à superfície; depois rebinda o RTV e o estado comum (`bind_common`).
- [ ] **Step 4: Rodar** → PASS; `cargo test --workspace` → PASS.
- [ ] **Step 5: Commit** `feat(gpui-core): shaders do usuário no D3D11`

### Task 4: D3D12

**Files:** Create `core/src/backends/d3d12/d3d12_user_shader.c`, `d3d12_backdrop.c`; modify `d3d12_loader.c`, `d3d12_internal.h`, `d3d12_render.c`, `d3d12_backend.c`, `d3d12_descriptors.c` (SRV do backdrop), `d3d12_swapchain.c`.
- Consumes: casos de `tests/shader_backends.rs`.
- [ ] **Step 1:** habilitar os mesmos casos para `D3D12`. **Step 2:** FAIL. **Step 3:** root signature com CBV (b0), SRV (t1), sampler (s2); PSO por shader criado em `shader_prepare`; cópia com barreiras `RENDER_TARGET→COPY_SOURCE→RENDER_TARGET`; uniforms no anel de upload do quadro (`d3d12_upload.c`). **Step 4:** PASS + workspace. **Step 5: Commit** `feat(gpui-core): shaders do usuário no D3D12`

### Task 5: OpenGL

**Files:** Create `core/src/backends/opengl/gl_user_shader.c`, `gl_backdrop.c`; modify `gl_api.h`/`gl_loader.c` (funções novas: `glCopyTexSubImage2D`, UBO), `gl_internal.h`, `gl_render.c`, `opengl_backend.c`, `gl_surface.c`.
- [ ] **Step 1:** casos para `OpenGL`. **Step 2:** FAIL. **Step 3:** compila o GLSL do naga (log do driver em `sh->error`); bindings do naga mapeados para nomes (`glGetUniformBlockIndex`/`glUniform1i`); backdrop com `glCopyTexSubImage2D` e Y invertido (origem embaixo) — corrigido via `viewport.w`. **Step 4:** PASS + workspace. **Step 5: Commit** `feat(gpui-core): shaders do usuário no OpenGL`

### Task 6: Vulkan

**Files:** Create `core/src/backends/vulkan/vk_user_shader.c`, `vk_backdrop.c`; modify `vk_api.h`/`vk_loader.c`, `vk_internal.h`, `vk_render.c`, `vk_pipeline.c` (layout de descritores), `vulkan_backend.c`, `vk_swapchain.c`.
- [ ] **Step 1:** casos para `Vulkan`. **Step 2:** FAIL. **Step 3:** módulo SPIR-V direto; descriptor set (UBO dinâmico, imagem, sampler); backdrop: encerra o render pass, `vkCmdCopyImage` com barreiras, retoma com `LOAD_OP_LOAD`. **Step 4:** PASS + workspace. **Step 5: Commit** `feat(gpui-core): shaders do usuário no Vulkan`

### Task 7: WGPU

**Files:** Create `higanbana-gpui-sys/src/wgpu_backend/user_shader.rs`, `backdrop.rs`; modify `wgpu_backend/ffi.rs` (callbacks `shader_prepare/release`), `mod.rs` (vtable), `render.rs` (lote de shader), `surface.rs` (resize).
- [ ] **Step 1:** casos para `WGPU`. **Step 2:** FAIL. **Step 3:** `create_shader_module` com o WGSL composto (sem tradução); erro de validação → `sh->error`; backdrop via `copy_texture_to_texture` entre passes (a textura do surface precisa de `COPY_SRC` no `usage`). **Step 4:** PASS + workspace. **Step 5: Commit** `feat(gpui-sys): shaders do usuário no WGPU`

### Task 8: Elemento, pintor e animação

**Files:**
- Modify: `src/style/mod.rs` (`Style.shader: Option<ShaderFill>`), `src/style/styled.rs` (`fn shader(self, s: &Shader, p: ShaderParams) -> Self`), `src/paint/canvas.rs` (`Canvas::shader` + `Painter::shader`), `src/paint/gpu.rs` (`GpuCanvas` cria/usa `GpuShader` por cache), `src/paint/painter.rs` (ordem: sombra → shader → borda → filhos), `src/app/run.rs` + `src/app/requests.rs` (pedido de quadro para shader animado pintado, nunca com `window_state.minimized`)
- Create: `src/paint/shaders.rs` (`ShaderCache<'c>`: `Shader` → `GpuShader`, como `ImageCache`), `src/paint/tests_shader.rs`

**Interfaces:**
- Consumes: Task 2 (`Shader`, `GpuShader`, `DrawList::shader`).
- Produces: `pub struct ShaderFill { pub shader: Shader, pub params: ShaderParams }`; `trait Canvas { fn shader(&mut self, rect: Rect, radius: Corners<f32>, fill: &ShaderFill, opacity: f32) {} }` (padrão vazio para canvases de teste); `Painter::shader(...)`. Shader que falha (`last_error` ou criação) → pinta quad magenta `#FF00FF` com o mesmo raio e faz `eprintln!` uma vez por `Shader`.

- [ ] **Step 1: Testes que falham** (canvas gravador de `paint/tests.rs`):
  - `div().bg(RED).shader(&s, p)` → gravado: sem quad com `fill` vermelho; um `shader` com o rect da div.
  - com sombra e borda → ordem gravada: quad de sombra (fill transparente), shader, quad só de borda.
  - opacidade 0.5 herdada → `shader(..., opacity = 0.5)`.
  - shader inválido → quad magenta.
  - app (`tests/` do laço, ou teste de `Runner` existente): shader `animated` pintado → `deadline()` imediato; com `minimized` → sem prazo (Review Focus 4).
- [ ] **Step 2:** FAIL. **Step 3:** Implementar. **Step 4:** `cargo test --workspace` e `cargo test -p higanbana-gpui --features video` → PASS.
- [ ] **Step 5: Commit** `feat(gpui): shader como fundo de div e no canvas`

### Task 9: LiquidGlass, galeria e medição

**Files:**
- Create: `examples/gallery/sections/liquid_glass.rs`, `examples/gallery/sections/liquid_glass.wgsl`, `examples/elements/shader.rs` (exemplo mínimo: gradiente animado)
- Modify: registro de seções da galeria (`examples/gallery/sections/mod.rs`/`shell.rs`)

**Interfaces:**
- Produces: `fn liquid_glass<V: View>(params: GlassParams) -> Element<V>` só com API pública: refração na borda (desloca `backdrop` pela normal de `rounded_sdf`, força em `param(0).x`), `backdrop_blur` (raio `param(0).y`), tinta `param(1)`, brilho especular móvel com `time`. Seção com fundo colorido/imagem e cards de vidro arrastáveis ou animados, slider de força/blur.
- [ ] **Step 1:** Rodar a galeria em cada backend disponível (`cargo run --example gallery`, trocar backend pelo seletor existente) e capturar print; vidro visível e refração perceptível, sem magenta.
- [ ] **Step 2: Medir** com a galeria aberta na seção de vidro e numa seção sem shader, D3D11: CPU do processo (média 10 s), working set, VRAM (`ctx.stats().gpu_bytes`), tempo do 1º `Context::shader` (naga + D3DCompile), tamanho do `.exe` release com e sem `--no-default-features --features wgpu`. Comparar seção sem shader com `master`. Anotar números no commit.
- [ ] **Step 3: Commit** `feat(gpui): exemplo LiquidGlass na galeria`

### Task 10: README da crate

**Files:**
- Create: `crates/higanbana-gpui/README.md`

- [ ] **Step 1:** Ler `src/lib.rs` (`prelude` e reexports), cada módulo público e cada pasta de `src/components/`, e `examples/`.
- [ ] **Step 2:** Escrever, em EN, seções: visão geral e arquitetura (Rust → `higanbana-gpui-sys` → núcleo C → 5 backends; AUTO e troca); começo rápido (`App`, `View`, `run`); laço e contexto (`RenderCx`, `EventCx`, timers, tarefas, mailbox, `request_redraw_in`); elementos (`div`, texto, imagem, ícone, polígono, overlay, `text_field`, scroll, listas virtuais); estilo (`Styled` — todas as funções, agrupadas), layout (flex/taffy, `px/pct/auto`), eventos, foco e teclado; texto e fontes (`App::font_dirs`, família padrão); imagens e ícones (`ImageSource`, `IconSet`); animação (`Transition`, `Easing`, spring, presença, repetição); tema (`Theme`); **componentes** — tabela por categoria (`basic`, `carousel`, `form`, `list`, `nav`, `overlay`, `video`, `window`) com construtor e exemplo curto de cada; vídeo (feature `video`); shaders (contrato WGSL, `ShadeInput`, funções do prelúdio, `Shader`, `params!`, backdrop, animação, erros, feature `shaders`, LiquidGlass); exemplos (`cargo run --example ...` para cada um em `examples/`); testes e features Cargo.
- [ ] **Step 3: Verificar** — todo nome de função/tipo citado existe: `grep` de cada identificador do README em `src/`; snippets curtos compilam (copiar 3 deles para um teste `doc`/exemplo temporário e rodar `cargo build --examples`).
- [ ] **Step 4: Commit** `docs(gpui): README da crate`

# Migração desktop → higanbana-gpui · Plano 1: fundações da lib

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dar à `higanbana-gpui` tudo de base que o desktop precisa antes de portar telas: ícones SVG, gradiente, tema por JSON, tarefas em segundo plano, cursor oculto, scroll horizontal/programático, Popover, RadioGroup, Progress e Spinner.

**Architecture:** Recursos visuais novos passam pelo caminho de imagens já existente (funciona nos 5 backends sem shader novo): SVG e gradiente viram `ImageSource` rasterizados na CPU e cacheados. Tarefas em segundo plano usam thread + `Waker` (`hg_window_wake`) e entregam o resultado à view no thread da UI. Componentes seguem o padrão da lib: builder + `*State` na view + lens.

**Tech Stack:** Rust 2024, `higanbana-gpui` (+ core C), `resvg` (nova dep), `serde_json` (já no workspace).

**Spec:** `docs/superpowers/specs/2026-11-21-desktop-gpui-migration-design.md`

**Planos seguintes** (escritos depois deste, pois dependem das APIs daqui): Plano 2 = shell do app (main, app, chrome com TitleBar/Sidebar/busca/notificações) + Sidebar/Command/Kbd na lib. Plano 3 = player (peças na lib + tela no app). Plano 4 = telas restantes (+ Carousel, Breadcrumb, PopupMenu, Link, lista virtual) e remoção do gpui-kit.

## Global Constraints

- Branch `feat/desktop-gpui`. Commits sem trailer de atribuição.
- Arquivos pequenos por responsabilidade; doc/comentários em português.
- Recursos valem nos 5 backends (via caminho comum de imagens/paint); validação manual só Windows/D3D11.
- Sem frames contínuos sem animação ativa: CPU ociosa ~0%.
- `cargo test -p higanbana-gpui` e `cargo test -p higanbana-gpui --features video` verdes ao fim de cada task.
- Visual dos componentes novos = o do gpui-kit atual no desktop (mesmos tamanhos/cores dos tokens).

## Review Focus

- Ícone pedido em tamanhos/escala DPI diferentes: rasteriza por (ícone, px físico) e não borra nem recria a cada frame (teste Task 1 `icon_cache_reuses`).
- Tarefa em segundo plano cuja view/janela já fechou: resultado descartado sem pânico (teste Task 4 `task_after_close_dropped`).
- Popover aberto e janela redimensionada / âncora sai de vista: popover segue âncora ou fecha, nunca fica solto (teste Task 7 `popover_follows_anchor`).
- Tema JSON com chave faltando ou cor inválida: cai no token padrão do tema escuro, não falha o app (teste Task 3 `theme_json_missing_keys_default`).
- Scroll programático além do conteúdo: clamp em [0, max] (teste Task 6 `scroll_to_clamps`).

---

### Task 1: Ícones SVG

**Files:**
- Create: `src/icon/mod.rs` (API `Icon`, `icon()`), `src/icon/raster.rs` (resvg → RGBA, cache), `src/icon/registry.rs` (`IconSet`: nome → bytes SVG), `src/icon/tests.rs`
- Modify: `Cargo.toml` (`resvg`), `src/lib.rs` (+prelude), `src/components/basic/button.rs` (`.icon()`), `src/paint/painter.rs` se preciso
- Copy: SVGs Lucide de `crates/higanbana-desktop/icons/` + os do kit usados pelo app → `crates/higanbana-gpui/icons/lucide/` (somente os usados)

**Interfaces:**
- Produces:
  - `pub struct IconSet` com `IconSet::new() -> Self`, `.add(name: &'static str, svg: &'static [u8]) -> Self`; `App::icons(self, set: IconSet) -> Self`. Macro `icon_set!["arrow-left" => "icons/arrow-left.svg", ...]` usando `include_bytes!`.
  - `IconSet::lucide() -> IconSet` com os ícones copiados (nomes kebab-case do Lucide).
  - `pub fn icon<V>(name: &'static str) -> Element<V>`; builders `.size(px: f32)` (padrão 16) e cor via `text_color` herdada (tint).
  - `Button::icon(self, name: &'static str) -> Self` (ícone à esquerda do label; só ícone quando sem label).
  - Raster: SVG renderizado em branco (alfa) no tamanho físico `round(px * scale)`, tint aplicado no draw. Cache `HashMap<(&'static str, u32), ImageSource>`.

- [ ] **Step 1: Testes em `src/icon/tests.rs`**
  - `raster_has_alpha`: rasterizar `circle` 16px escala 1 → 16×16, pixel central alfa>0, canto alfa=0.
  - `icon_cache_reuses`: mesmo (nome, 16px, escala 2) duas vezes → mesmo `ImageSource::id()`; escala 1 → id diferente.
  - `unknown_icon_is_empty_box`: nome inexistente → elemento com tamanho `size` e sem imagem (não panica).
  - `icon_tinted_by_text_color`: canvas de gravação recebe `tint` = `text_color` do pai.
- [ ] **Step 2:** `cargo test -p higanbana-gpui icon::` → FAIL.
- [ ] **Step 3:** Implementar.
- [ ] **Step 4:** `cargo test -p higanbana-gpui` → PASS.
- [ ] **Step 5:** Commit `feat(gpui): ícones SVG (resvg) com cache e tint`.

### Task 2: Gradiente linear

**Files:**
- Create: `src/style/gradient.rs`, `src/paint/gradient.rs` (rampa → `ImageSource` 1×256 ou 256×1, cache por gradiente)
- Modify: `src/style/mod.rs` (`Background`), `src/style/styled.rs`, `src/paint/painter.rs`

**Interfaces:**
- Produces: `pub struct LinearGradient { pub dir: GradientDir, pub stops: Vec<(f32, Color)> }`, `pub enum GradientDir { Down, Up, Right, Left }`; `Styled::bg_gradient(self, g: LinearGradient)`; atalho `LinearGradient::scrim(dir, from: Color, to: Color)`.
- Pintura: imagem da rampa esticada no `rect` do nó, com `radius` do nó. Cache por hash dos stops.

- [ ] **Step 1: Testes** (`src/paint/tests_gradient.rs`): `ramp_endpoints` (pixel 0 = cor inicial, último = final, ±1/255); `gradient_draws_image_in_rect` (canvas de gravação recebe 1 imagem com dest = rect do nó); `same_gradient_cached` (dois nós iguais → mesmo id de imagem).
- [ ] **Step 2:** rodar → FAIL. **Step 3:** implementar. **Step 4:** `cargo test -p higanbana-gpui` → PASS.
- [ ] **Step 5:** Commit `feat(gpui): gradiente linear em bg`.

### Task 3: Tema por JSON e tokens do desktop

**Files:**
- Create: `src/theme/json.rs`, `src/theme/tests.rs`
- Modify: `src/theme/mod.rs`, `Cargo.toml` (`serde_json` opcional? não: dep normal)

**Interfaces:**
- Consumes: formato de `crates/higanbana-desktop/themes/higanbana.json` (ler o arquivo: chaves no estilo gpui-component, ex. `"primary.background"`, `"list.active.background"`).
- Produces: `Theme::from_json(json: &str) -> Result<Theme, String>` (variante escura do arquivo; chave ausente/inválida → valor de `Theme::dark()`). Novos campos em `Theme`: `radius_lg: f32`, `list_active: Color`, `list_hover: Color`, `success: Color`, `warning: Color`, `info: Color` (+ o que o desktop usar de `theme.*`: levantar via `rg "theme\.\w+" crates/higanbana-desktop/src -o | sort -u` e mapear cada um).
- Cores do JSON: `#rrggbb`, `#rrggbbaa`.

- [ ] **Step 1: Testes:** `parses_desktop_theme` (lê `../higanbana-desktop/themes/higanbana.json` via `include_str!`, `primary` = valor do arquivo); `theme_json_missing_keys_default` (`"{}"` → igual a `Theme::dark()` nos campos de cor); `bad_color_falls_back`.
- [ ] **Step 2:** FAIL. **Step 3:** implementar (atualizar `palette.rs` com valores padrão dos campos novos). **Step 4:** PASS.
- [ ] **Step 5:** Commit `feat(gpui): tema a partir de JSON e tokens extras`.

### Task 4: Tarefas em segundo plano

**Files:**
- Create: `src/cx/task.rs`, `src/cx/tests_task.rs`
- Modify: `src/cx/event.rs`, `src/cx/render.rs`, `src/app/run.rs` (drena resultados no `HG_EVENT_WAKE`), `src/app/requests.rs`

**Interfaces:**
- Produces:
  - `EventCx::spawn<V: 'static, T: Send + 'static>(&mut self, work: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(&mut V, T, &mut EventCx) + 'static) -> TaskId` — roda `work` numa thread (pool simples de N=4 threads), entrega `done` na UI e acorda a janela.
  - Mesmo método em `RenderCx`. `EventCx::cancel_task(TaskId)` (resultado descartado).
  - `pub struct TaskId(u64)`.
  - Para código async (tokio do desktop): `spawn` com `work` que chama `handle.block_on(fut)` — documentar; sem runtime async na lib.

- [ ] **Step 1: Testes:** `task_result_delivered` (fila de tarefas sem janela: `work` retorna 42, ao drenar `done` recebe 42 e muta a view); `cancelled_task_dropped`; `task_after_close_dropped` (fila destruída antes da conclusão → sem pânico, `work` termina).
- [ ] **Step 2:** FAIL. **Step 3:** implementar. **Step 4:** PASS.
- [ ] **Step 5:** Commit `feat(gpui): tarefas em segundo plano com entrega na UI`.

### Task 5: Cursor oculto

**Files:**
- Modify: `core/include/hgui.h` (`HG_CURSOR_NONE`), `core/src/window/cursor.c` (`SetCursor(NULL)`), `src/gfx/cursor.rs` (`CursorStyle::None`), `crates/higanbana-gpui-sys` bindings se gerados à mão
- Test: `crates/higanbana-gpui-sys/tests/cursor.rs`

- [ ] **Step 1: Teste:** `cursor_none_accepted` (`hg_window_set_cursor(win, HG_CURSOR_NONE)` retorna OK; `CursorStyle::None.to_hg()` mapeia).
- [ ] **Step 2:** FAIL. **Step 3:** implementar. **Step 4:** PASS (`cargo test -p higanbana-gpui-sys`).
- [ ] **Step 5:** Commit `feat(gpui): cursor oculto`.

### Task 6: Scroll horizontal e programático

**Files:**
- Modify: `src/style/mod.rs` (`overflow_x`), `src/layout/position.rs`, `src/tree/mod.rs`, `src/input/dispatch.rs` (roda com Shift ou wheel horizontal), `src/cx/event.rs`
- Create: `src/tree/tests_scroll.rs`

**Interfaces:**
- Produces: `scroll_x<V>(id) -> Element<V>`; `EventCx::scroll_to(id, offset: f32)`, `EventCx::scroll_into_view(container, child)`; `RenderCx::scroll_offset(id) -> f32` (já existe no tree; expor) e `scroll_max(id) -> f32`.

- [ ] **Step 1: Testes:** `horizontal_scroll_moves_children`; `shift_wheel_scrolls_x`; `scroll_to_clamps` (offset 9999 → max; -5 → 0); `scroll_into_view_reveals_child`.
- [ ] **Step 2:** FAIL. **Step 3:** implementar. **Step 4:** PASS.
- [ ] **Step 5:** Commit `feat(gpui): scroll horizontal e programático`.

### Task 7: Popover

**Files:**
- Create: `src/components/overlay/popover.rs`, `src/components/overlay/tests/popover.rs`
- Modify: `src/components/overlay/mod.rs`

**Interfaces:**
- Consumes: `overlay(Anchor, ..)`, `AnchorAlign`, `on_dismiss` (existentes).
- Produces: `pub struct PopoverState { pub open: bool }`; `Popover::new(id, &mut PopoverState, lens, trigger: Element<V>, content: impl FnOnce(&mut RenderCx<V>) -> Element<V>)` com `.anchor(PopoverAnchor::{BottomLeft, BottomRight, TopLeft, TopRight})`, `.w(px)`. Visual: `popover` bg, 1px `border`, `radius_lg`, `shadow_lg`, padding 12 (conferir kit). Abre/fecha com fade+leve translate (`presence`, 160ms). Fecha: clique fora, Esc, `state.open = false`.

- [ ] **Step 1: Testes:** `click_trigger_toggles`; `escape_closes`; `click_outside_closes`; `popover_follows_anchor` (âncora move após relayout → overlay reposiciona); `bottom_right_aligns_right_edges`.
- [ ] **Step 2:** FAIL. **Step 3:** implementar. **Step 4:** PASS.
- [ ] **Step 5:** Commit `feat(gpui): componente Popover`.

### Task 8: RadioGroup

**Files:**
- Create: `src/components/form/radio.rs`, `src/components/form/tests_radio.rs`
- Modify: `src/components/form/mod.rs`

**Interfaces:**
- Produces: `RadioGroup::new(id, selected: Option<usize>, items: impl IntoIterator<Item = impl Into<String>>, on_change: impl Fn(&mut V, usize, &mut EventCx) + 'static)`, `.horizontal()`; visual igual ao Radio do kit (círculo 16px, borda `primary`, ponto 8px quando marcado). Teclado: setas mudam seleção; foco visível.

- [ ] **Step 1: Testes:** `click_selects`; `arrows_move_selection`; `horizontal_lays_out_row`.
- [ ] **Step 2:** FAIL. **Step 3:** implementar. **Step 4:** PASS.
- [ ] **Step 5:** Commit `feat(gpui): componente RadioGroup`.

### Task 9: Progress e Spinner

**Files:**
- Create: `src/components/basic/progress.rs`, `src/components/basic/spinner.rs`, testes em `src/components/basic/tests.rs`
- Modify: `src/components/basic/mod.rs`, `src/style/mod.rs` + `src/tree/transform.rs` (rotação) **só se** o Spinner do kit girar (conferir em `~/.cargo/registry/src/*/gpui-kit-*/src/**/spinner*`)

**Interfaces:**
- Produces: `progress(value: f32 /*0..100*/) -> Element<V>` com `.size(ProgressSize::{XSmall, Small, Medium})` (alturas iguais ao kit), cor `primary`, trilho `secondary`/conforme kit. `Spinner::new().size(px).color(c)` animado via `animate_loop` (frames só enquanto visível).
- Se precisar rotação: `Styled::rotate(rad)` aplicada só a ícone/imagem (rotação de quad de imagem no paint; não em layout).

- [ ] **Step 1: Testes:** `progress_fill_width` (value 25 em 200px → fill 50px); `progress_clamps` (−5→0, 150→100%); `spinner_requests_frames_only_when_rendered`.
- [ ] **Step 2:** FAIL. **Step 3:** implementar. **Step 4:** PASS.
- [ ] **Step 5:** Commit `feat(gpui): Progress e Spinner`.

### Task 10: Galeria

**Files:** `examples/gallery/sections/{icons,popover,radio,progress}.rs`, registro em `sections/mod.rs`.

- [ ] **Step 1:** Seções novas usando ícones Lucide, gradiente, Popover, RadioGroup, Progress, Spinner.
- [ ] **Step 2:** `cargo build -p higanbana-gpui --example gallery --release` sem warnings.
- [ ] **Step 3:** Rodar, screenshot (PrintWindow), conferir contra o kit; CPU ociosa ~0% (Get-Process, 10s) com spinner fora da tela.
- [ ] **Step 4:** Commit `feat(gpui): galeria com ícones, gradiente, popover, radio e progresso`.

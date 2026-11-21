# higanbana-gpui Animações Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Estender `src/anim` com transform de subárvore, springs, delay/repetição/yoyo, valores multi-componente e presença genérica, mais seção na galeria.

**Architecture:** Mesmo modelo atual: tween consultado no render via `RenderCx`, guardado no `Animator` por `ElementId`, descartado se não consultado. Transform vira passo pós-posicionamento na árvore (`after_position`), que move `rect/content/clip` da subárvore — paint e hit-test herdam sem mudança; escala de texto/raio usa `Node::paint_scale`.

**Tech Stack:** Rust 2024, crate `higanbana-gpui` (`crates/higanbana-gpui`).

**Spec:** `docs/superpowers/specs/2026-11-20-higanbana-gpui-anim-decor-video-design.md` (seção 1)

## Global Constraints

- Sem frames contínuos sem animação ativa: `Animator::is_active()` só `true` se algum tween consultado neste render não terminou (ou é infinito e foi consultado).
- Arquivos pequenos por responsabilidade (nada de arquivo deus); comentários/doc em português como o resto do crate.
- Sem rotação. Escala com origem no centro do `rect` do nó.
- Commits sem trailer de atribuição. Branch: `feat/higanbana-gpui` (mesma da lib).
- Todos os testes do crate continuam passando: `cargo test -p higanbana-gpui`.

## Review Focus

- Tween infinito (`Repeat::Forever`) de elemento que some da árvore: deve parar de agendar frames no render seguinte (teste na Task 2).
- Retarget de spring no meio do movimento: sem salto de valor (teste na Task 1).
- Transform aninhado (pai `scale(0.5)` + filho `translate`): compõe na ordem pai→filho (teste na Task 4).
- Clique num elemento transladado acerta a posição visual, não a original (teste na Task 4).
- `presence` com abrir/fechar rápido: valor contínuo, sem reiniciar de 0 (teste na Task 3).

---

### Task 1: Springs

**Files:**
- Create: `src/anim/spring.rs`
- Modify: `src/anim/easing.rs` (Transition ganha `kind`), `src/anim/animator.rs` (Tween guarda velocidade), `src/anim/mod.rs`

**Interfaces:**
- Produces: `pub struct Spring { pub stiffness: f32, pub damping: f32, pub mass: f32 }`; `Spring::DEFAULT` (stiffness 170, damping 26, mass 1 — "gentle"); `Transition::spring(stiffness: f32, damping: f32) -> Transition`; `Spring::sample(&self, from: f32, to: f32, v0: f32, t: f32) -> (value: f32, velocity: f32)` em forma fechada (sub/crit/super-amortecido); assentado quando `|x-to| < 1e-3*max(1,|to-from|)` e `|v| < 1e-2`.
- `Transition` passa a ter `kind: Motion` com `enum Motion { Timed { duration, easing }, Spring(Spring) }`; `Transition::ms` e `.easing()` continuam funcionando (compat).

- [ ] **Step 1: Testes falhando em `src/anim/spring.rs`**
  - `spring_settles_at_target`: sample em t=3s → valor ≈ to (1e-3), velocidade ≈ 0.
  - `critically_damped_no_overshoot`: damping = 2*sqrt(k*m) → nunca passa de `to` em 0..2s (amostras a cada 10ms).
  - `retarget_keeps_continuity` (em animator.rs): spring 0→1, em t=100ms muda alvo para 0; valor logo após retarget difere < 1e-3 do valor antes.
  - `spring_active_until_settled`: `is_active()` true em 100ms, false após 3s.
- [ ] **Step 2:** `cargo test -p higanbana-gpui anim::` → FAIL (não compila).
- [ ] **Step 3:** Implementar `Spring` + `Motion`; `Tween` guarda `v0` e no retarget usa `sample` para capturar valor e velocidade atuais.
- [ ] **Step 4:** `cargo test -p higanbana-gpui` → PASS.
- [ ] **Step 5:** Commit `feat(gpui): springs nas animações`.

### Task 2: Delay, repetição e yoyo

**Files:**
- Create: `src/anim/repeat.rs` (mapeamento tempo→progresso)
- Modify: `src/anim/easing.rs` (builders), `src/anim/animator.rs`

**Interfaces:**
- Produces: `Transition::delay(self, ms: u64) -> Self`, `Transition::repeat(self, r: Repeat) -> Self`, `Transition::yoyo(self) -> Self`; `pub enum Repeat { Once, Times(u32), Forever }`; `fn phase(elapsed: f32, duration: f32, delay: f32, repeat: Repeat, yoyo: bool) -> (progress: f32, done: bool)` em `repeat.rs`.
- Repetição só vale para `Motion::Timed` (spring ignora, documentar).
- `cx.animate_loop(key, Transition) -> f32` em `src/cx/render.rs`: 0..1 contínuo desde a primeira consulta (açúcar para spinners: `animate_from(key, 0, 1, t.repeat(Forever))`).

- [ ] **Step 1: Testes em `src/anim/repeat.rs`**
  - `delay_holds_start`: delay 50ms, t=30ms → progress 0, done false.
  - `times_two_restarts`: dur 100, Times(2): t=150 → 0.5; t=250 → 1.0 done.
  - `yoyo_returns`: dur 100, Times(2), yoyo: t=150 → 0.5 descendo (=0.5), t=180 → 0.2.
  - `forever_never_done`: t=10_000 → done false.
  - Em animator.rs `forever_stops_when_not_queried`: render 1 consulta tween Forever → active; render 2 não consulta → após `end()` vazio e `is_active()` false.
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** Implementar `phase` e usá-lo em `Tween::progress`.
- [ ] **Step 4:** `cargo test -p higanbana-gpui` → PASS.
- [ ] **Step 5:** Commit `feat(gpui): delay, repetição e yoyo nas animações`.

### Task 3: Valores multi-componente e presença

**Files:**
- Create: `src/anim/lerp.rs`, `src/anim/presence.rs`
- Modify: `src/anim/animator.rs` (Tween de `[f32; 4]`), `src/cx/render.rs`, `src/components/overlay/common.rs` (usar o genérico)

**Interfaces:**
- Produces: `pub trait Animatable: Copy { fn to_arr(self) -> [f32; 4]; fn from_arr(a: [f32; 4]) -> Self; }` implementado para `f32`, `Color`, `Point`, `Size`, `Rect` (Rect usa 4). `RenderCx::animate_value<T: Animatable>(&mut self, key, target: T, t: Transition) -> T` e `animate_value_from`. `animate`/`animate_from` existentes viram wrappers de `f32`. Spring age por componente.
- `RenderCx::presence(&mut self, key: impl Into<ElementId>, visible: bool, t: Transition) -> Option<f32>`: `None` quando invisível e saída terminada (componente não renderiza); `Some(p)` 0..1 caso contrário. `overlay/common.rs::presence` passa a chamar isso (mantém assinatura interna devolvendo `f32` = `unwrap_or(0.0)`).

- [ ] **Step 1: Testes**
  - `color_interpolates_per_channel`: preto→branco linear 100ms, em 50ms → (0.5,0.5,0.5,1).
  - `rect_animates`: Rect(0,0,10,10)→(100,0,10,10), em 50ms x=50.
  - `presence_none_after_exit` (cx/render tests ou anim/presence.rs): visible true → Some(..); false → Some decrescente; após 200ms → None.
  - `presence_reopen_is_continuous`: fecha em 0, reabre em 75ms → valor ≈ valor no instante (sem voltar a 0).
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** Implementar; Tween guarda `[f32;4]` + `dims`.
- [ ] **Step 4:** `cargo test -p higanbana-gpui` → PASS (testes de overlays inclusos).
- [ ] **Step 5:** Commit `feat(gpui): animar cores/retângulos e presença genérica`.

### Task 4: Transform de subárvore (translate + scale)

**Files:**
- Create: `src/tree/transform.rs`, `src/tree/tests_transform.rs`
- Modify: `src/style/mod.rs` (campos), `src/style/styled.rs` (builders), `src/tree/node.rs` (`paint_scale`), `src/tree/relayout.rs` (`after_position` chama `apply_transforms`), `src/paint/painter.rs` (texto/raio/borda/sombra usam `paint_scale`), `src/paint/field.rs`

**Interfaces:**
- Produces: `Style { translate: Point, scale: f32 /* 1.0 */ }`; `Styled::translate(self, x: f32, y: f32)`, `Styled::translate_x`, `translate_y`, `Styled::scale(self, s: f32)` (clamp ≥ 0).
- `Node::paint_scale: f32` (produto das escalas dos ancestrais e do próprio).
- `Tree::apply_transforms(&mut self)`: pré-ordem; cada nó com transform aplica `p' = c + (p - c)*s + t` (c = centro do `rect` do nó já transformado pelo pai) a `rect`, `content`, `clip` dele e de toda a subárvore. Sem transform: custo zero (pula).
- Texto: tamanho de fonte * `paint_scale`; raio/borda/blur de sombra * `paint_scale`.

- [ ] **Step 1: Testes em `src/tree/tests_transform.rs`** (usar helpers de `paint/tests.rs`/`components/basic/tests_layout.rs`)
  - `translate_moves_subtree`: pai 100x100 em (0,0) com `.translate(20, 10)`, filho 10x10 → filho rect em (20,10).
  - `scale_around_center`: nó 100x100 `.scale(0.5)` → rect (25,25,50,50).
  - `nested_composes`: pai `.scale(0.5)`, filho `.translate(10,0)` → posição = escala do pai depois translate do filho; `paint_scale` do filho 0.5.
  - `hit_follows_visual`: botão `.translate(200,0)`; `hit_test` em (205,5) acerta o botão; em (5,5) não.
  - `text_size_scaled`: canvas de gravação recebe `TextStyle.size` = 14*0.5 com `.scale(0.5)`.
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** Implementar.
- [ ] **Step 4:** `cargo test -p higanbana-gpui` → PASS.
- [ ] **Step 5:** Commit `feat(gpui): translate e scale de subárvore`.

### Task 5: Seção "Animações" na galeria

**Files:**
- Create: `examples/gallery/sections/anim.rs`
- Modify: `examples/gallery/sections/mod.rs` (registrar seção), estado em `examples/gallery/main.rs` se preciso

**Interfaces:**
- Consumes: tudo das Tasks 1–4.

Conteúdo: card que entra deslizando (`presence` + `translate_y` + opacidade); botão que pulsa no hover (spring em `scale`); spinner (`animate_loop` + quadrados com delays escalonados, sem rotação); cor de fundo que cicla entre cores do tema (`animate_value::<Color>` com yoyo); botão "mostrar/ocultar" para presença.

- [ ] **Step 1:** Implementar seção.
- [ ] **Step 2:** `cargo build -p higanbana-gpui --example gallery --release` sem warnings.
- [ ] **Step 3:** Rodar galeria, conferir visualmente (screenshot via PrintWindow) e CPU ociosa 0% com seção parada (spinner fora da tela/oculto) — medir com Get-Process por 10s.
- [ ] **Step 4:** Commit `feat(gpui): seção de animações na galeria`.

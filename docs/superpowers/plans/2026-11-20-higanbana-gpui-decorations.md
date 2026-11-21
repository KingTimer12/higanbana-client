# higanbana-gpui Decorations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Janela sem barra nativa (`Decorations::Custom`) com barra desenhada pelo app, mantendo snap/sombra/animações do Windows; componente `TitleBar`; fullscreen.

**Architecture:** Core C mantém `WS_OVERLAPPEDWINDOW`, zera a área não-cliente em `WM_NCCALCSIZE` e responde `WM_NCHITTEST` com regiões que o app publica a cada frame (`hg_window_set_hit_regions`). Bordas de redimensionar calculadas no C. Botão maximizar devolve `HTMAXBUTTON` (snap layouts do Win11) e os eventos NC dele viram eventos de mouse normais. Comandos (min/max/fechar/fullscreen) via `hg_window_command`; estado via `hg_window_state` + evento `HG_EVENT_WINDOW_STATE`. No Rust, elementos marcam regiões (`.window_drag()`, `.window_control(..)`), o `Runner` coleta as regiões da árvore depois do layout e publica se mudaram.

**Tech Stack:** C11 (core), Rust (`higanbana-gpui`, `higanbana-gpui-sys` bindgen).

**Spec:** `docs/superpowers/specs/2026-11-20-higanbana-gpui-anim-decor-video-design.md` (seção 2)

## Global Constraints

- Branch `feat/higanbana-gpui`. Commits sem trailer de atribuição.
- `hg_window_desc` NÃO muda (evita quebrar literais): decorations via setter `hg_window_set_decorations`.
- Sem arquivo deus: código novo do C em `core/src/window/decor.c`; Rust em arquivos próprios.
- `Decorations::Native` = comportamento atual, byte a byte (nenhuma mensagem nova tratada).
- CPU ociosa 0%: publicar regiões só quando mudam; nada de timers.
- Todos os testes: `cargo test -p higanbana-gpui-sys` e `cargo test -p higanbana-gpui`.

## Review Focus

- Maximizada: conteúdo não pode vazar para fora do monitor (borda invisível compensada) — teste Task 1 (`maximized_client_fits_monitor`).
- Botões/abas dentro da área de arrastar continuam clicáveis (filhos interativos viram CLIENT) — teste Task 3.
- Overlay (Select aberto) sobre a barra recebe clique, não arrasta a janela — teste Task 3.
- Hover do botão maximizar funciona apesar de `HTMAXBUTTON` (NC mouse vira mouse move) — teste Task 1.
- Troca de backend para OpenGL recria a HWND: decorations e regiões precisam sobreviver — teste Task 1 (`recreate_keeps_custom_frame`).

---

### Task 1: Core C — decorations, hit test, comandos, estado

**Files:**
- Create: `crates/higanbana-gpui/core/src/window/decor.c`
- Modify: `core/include/hgui.h` (API pública), `core/src/window/window.h` (campos), `core/src/window/window_win32.c` (despachar mensagens para decor.c; recreate copia estado), `core/src/window/input.c` (nada se possível)
- Test: `crates/higanbana-gpui-sys/tests/decor.rs`

**Interfaces (hgui.h):**
```c
typedef enum hg_hit { HG_HIT_CLIENT = 0, HG_HIT_CAPTION, HG_HIT_MAXIMIZE } hg_hit;
typedef struct hg_hit_region { float x, y, w, h; hg_hit kind; } hg_hit_region; /* pixels lógicos */
typedef enum hg_window_cmd { HG_WINDOW_MINIMIZE = 0, HG_WINDOW_MAXIMIZE, HG_WINDOW_RESTORE,
    HG_WINDOW_TOGGLE_MAXIMIZE, HG_WINDOW_CLOSE, HG_WINDOW_ENTER_FULLSCREEN, HG_WINDOW_EXIT_FULLSCREEN } hg_window_cmd;
#define HG_WINDOW_MAXIMIZED  1u
#define HG_WINDOW_MINIMIZED  2u
#define HG_WINDOW_FULLSCREEN 4u
#define HG_WINDOW_FOCUSED    8u
/* novo hg_event_kind no fim: HG_EVENT_WINDOW_STATE (u.state.flags) */
hg_result hg_window_set_decorations(hg_window* win, int custom);
/* Copia as regiões; ordem = pintura (a última que contém o ponto vence). */
hg_result hg_window_set_hit_regions(hg_window* win, const hg_hit_region* regions, uint32_t count);
hg_result hg_window_command(hg_window* win, hg_window_cmd cmd);
uint32_t  hg_window_state(const hg_window* win);
```
- Borda de redimensionar: 6 px lógicos (só não maximizada/fullscreen e se `WS_THICKFRAME`).
- `WM_NCCALCSIZE` (custom, wp=TRUE): maximizada → recua `SM_CXFRAME+SM_CXPADDEDBORDER` (por DPI) em todos os lados; senão retorna 0 sem mexer.
- Região `HG_HIT_MAXIMIZE` → `HTMAXBUTTON`. `WM_NCMOUSEMOVE`/`WM_NCLBUTTONDOWN`/`WM_NCLBUTTONUP` com `HTMAXBUTTON` → empurra MOUSE_MOVE/DOWN/UP em coordenadas cliente e retorna 0 (sem o botão fantasma do sistema). `WM_NCMOUSELEAVE` → MOUSE_MOVE em (-1,-1).
- Fullscreen: guarda placement+estilo, troca para `WS_POPUP|WS_VISIBLE` no retângulo do monitor; sair restaura.
- Estado muda (`WM_SIZE` com SIZE_MAXIMIZED/RESTORED/MINIMIZED, fullscreen, `WM_ACTIVATE`) → empurra `HG_EVENT_WINDOW_STATE` só se flags mudaram.
- `hg__window_recreate_hwnd` preserva custom (estilo já é copiado; chama SWP_FRAMECHANGED) e regiões (vivem em `hg_window`).

- [ ] **Step 1: Testes em `tests/decor.rs`** (janela via `Fixture`, HWND via `hg__window_hwnd`, `SendMessageW(WM_NCHITTEST)` com coordenadas de tela):
  - `native_hit_test_unchanged`: sem custom, ponto no meio do cliente → `HTCLIENT`; acima do cliente → `HTCAPTION`.
  - `custom_client_covers_window`: após custom, `GetClientRect` == `GetWindowRect` (tamanhos).
  - `regions_map_to_hit_codes`: regiões caption(0,0,320,32) + maximize(240,0,40,32) + client(10,4,40,24): ponto (100,16)→HTCAPTION, (250,16)→HTMAXBUTTON, (20,16)→HTCLIENT, (160,120)→HTCLIENT, (160,2)→HTTOP, (1,120)→HTLEFT, (318,238)→HTBOTTOMRIGHT.
  - `maximize_command_sets_state_and_event`: `hg_window_command(MAXIMIZE)` → `hg_window_state & MAXIMIZED`, fila tem `HG_EVENT_WINDOW_STATE`; RESTORE limpa.
  - `maximized_client_fits_monitor`: custom + maximizar → retângulo cliente (em tela) dentro da área de trabalho do monitor (`MONITORINFO.rcWork`).
  - `max_button_nc_events_become_mouse`: `SendMessageW(WM_NCMOUSEMOVE, HTMAXBUTTON, pt)` → fila recebe `HG_EVENT_MOUSE_MOVE` com coords cliente lógicas.
  - `fullscreen_roundtrip`: ENTER → flag FULLSCREEN e janela cobre o monitor; EXIT → volta ao retângulo anterior.
  - `recreate_keeps_custom_frame`: custom + regiões → `hg__window_recreate_hwnd` → nova HWND ainda responde HTCAPTION em (100,16).
- [ ] **Step 2:** `cargo test -p higanbana-gpui-sys --test decor` → FAIL (símbolos ausentes).
- [ ] **Step 3:** Implementar `decor.c` + ganchos no `wndproc`.
- [ ] **Step 4:** `cargo test -p higanbana-gpui-sys` → PASS.
- [ ] **Step 5:** Commit `feat(gpui): decorations customizadas no core (hit test, comandos, fullscreen)`.

### Task 2: Rust gfx — wrappers

**Files:** Modify `crates/higanbana-gpui/src/gfx/window.rs`; Create `src/gfx/decor.rs`; Modify `src/gfx/event.rs` (ou onde `Event` vive) e `src/gfx/mod.rs`.

**Interfaces:**
- `pub enum Decorations { Native, Custom }` (default Native); `WindowOptions.decorations: Decorations`.
- `pub enum HitKind { Client, Caption, Maximize }`, `pub struct HitRegion { pub rect: Rect, pub kind: HitKind }`.
- `pub enum WindowCommand { Minimize, Maximize, Restore, ToggleMaximize, Close, EnterFullscreen, ExitFullscreen }`.
- `#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)] pub struct WindowState { pub maximized, minimized, fullscreen, focused: bool }`.
- `Window::set_hit_regions(&self, &[HitRegion])`, `Window::command(&self, WindowCommand)`, `Window::state(&self) -> WindowState`.
- `Event::WindowState(WindowState)`.

- [ ] **Step 1:** Teste unitário `state_from_flags` (flags 1|8 → maximized+focused) em `decor.rs`.
- [ ] **Step 2:** rodar → FAIL. **Step 3:** implementar (Window::new chama `hg_window_set_decorations` se Custom). **Step 4:** `cargo test -p higanbana-gpui` → PASS. **Step 5:** Commit `feat(gpui): API Rust de decorations`.

### Task 3: Regiões vindas da árvore + comandos nos handlers

**Files:** Create `src/tree/hit_regions.rs` (+ testes `src/tree/tests_hit_regions.rs`); Modify `src/element/*` (campo `window_hit: Option<HitKind>` no Element e Node), `src/style/styled.rs` ou builder do Element (`.window_drag()`, `.window_control(HitKind)`), `src/cx/event.rs` (`window_command`), `src/cx/render.rs` (`window_state()`), `src/app/run.rs`/`requests.rs`/`events.rs`.

**Interfaces:**
- `Element::window_drag(self) -> Self` (= `HitKind::Caption`), `Element::window_control(self, k: HitKind) -> Self`.
- `Tree::hit_regions(&self, out: &mut Vec<HitRegion>)`: pré-ordem (árvore principal, depois overlays). Nó marcado → região (rect ∩ clip). Descendente de região Caption com `on_click` ou `interact` → região Client. Toda raiz de overlay → Client (ou tela toda se `modal`).
- `EventCx::window_command(&mut self, c: WindowCommand)` (lista); Runner aplica após handler. `RenderCx::window_state() -> WindowState`. `Event::WindowState` atualiza estado do Runner e marca Rebuild.
- Runner publica regiões depois de cada layout se `Vec` mudou.

- [ ] **Step 1: Testes** em `tests_hit_regions.rs`:
  - `drag_region_and_interactive_children`: barra 400x32 `.window_drag()` com botão `on_click` 40x24 dentro → regiões `[Caption(barra), Client(botão)]`.
  - `maximize_control`: nó `.window_control(HitKind::Maximize)` → região Maximize.
  - `overlay_over_titlebar_is_client`: overlay aberto ancorado na barra → última região é Client com o rect do overlay.
  - `no_regions_without_marks`: árvore sem marcas → vazio.
  - em `cx` tests: `window_command_queued` (EventCx guarda comando).
- [ ] **Step 2:** FAIL. **Step 3:** implementar. **Step 4:** `cargo test -p higanbana-gpui` → PASS. **Step 5:** Commit `feat(gpui): regiões da janela a partir da árvore e comandos de janela`.

### Task 4: Componente `TitleBar` + galeria

**Files:** Create `src/components/window/mod.rs`, `src/components/window/title_bar.rs`, `src/components/window/tests.rs`; Modify `src/components/mod.rs`, `examples/gallery/main.rs` + `examples/gallery/shell.rs` (cabeçalho vira TitleBar, `Decorations::Custom`).

**Interfaces:**
- `TitleBar::new(title: impl Into<SharedString/String>) -> Self`; `.leading(Element<V>)` (ícone/logo), `.content(Element<V>)` (centro, ex.: abas/busca), `.trailing(Element<V>)` (antes dos botões); `.render(cx) -> Element<V>`.
- Altura 40 px; botões 46x40 (padrão Windows), glifos Segoe Fluent/MDL2 (`\u{E921}` min, `\u{E922}` max, `\u{E923}` restaurar, `\u{E8BB}` fechar); hover com cor animada (`animate_value`), fechar com hover `#C42B1C` e glifo branco. Maximizar usa `.window_control(Maximize)`; min/fechar são `on_click` → `cx.window_command`. Fullscreen: TitleBar não renderiza (retorna `div().hidden()`).

- [ ] **Step 1: Testes** (`tests.rs`, árvore sem GPU): `title_bar_regions` (Caption + Maximize + Clients dos botões min/fechar); `close_button_sends_command` (clique no fechar → EventCx com `WindowCommand::Close`); `restore_glyph_when_maximized`.
- [ ] **Step 2:** FAIL. **Step 3:** implementar + galeria. **Step 4:** testes PASS; build galeria release sem warnings; rodar galeria: screenshot, arrastar pela barra (SendInput) move a janela, botão maximizar maximiza, CPU ociosa 0%. **Step 5:** Commit `feat(gpui): componente TitleBar e galeria com decorations customizadas`.

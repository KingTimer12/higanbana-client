# Desktop GPUI — Plano 2: shell do app

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `higanbana-desktop` abre na lib `higanbana-gpui` (sem gpui-kit no binário) com janela, title bar, sidebar, busca (Ctrl K), downloads, aviso de atualização, notificações e login; telas do acervo viram placeholder até o Plano 4.

**Architecture:** Lib ganha o que o shell pede (caixa de correio entre threads, atalhos sem foco, botões voltar/avançar do mouse, `Kbd`, `Sidebar`, `Command`). No app, `AppState` deixa de ser `Entity` e vira campo da view raiz `Higanbana`; tarefas do gpui viram `cx.spawn(work, done)` com `core.block_on` no worker; `cx.emit` vira fila `events: Vec<AppEvent>` drenada pela raiz. Módulos ainda não portados (`views`, `video`, `reader`, `music`, `settings_dialog`, `components` exceto o que o shell usa) saem do `main.rs` com `// Plano 3/4` para o crate compilar e rodar.

**Tech Stack:** Rust, higanbana-gpui (Plano 1), higanbana-core (tokio via `Core::block_on`).

**Spec:** `docs/superpowers/specs/2026-11-21-desktop-gpui-migration-design.md`

## Global Constraints

- Branch `feat/desktop-gpui`; sem push; commits sem trailer de atribuição.
- Arquivos pequenos por responsabilidade; doc/comentários em português.
- Visual igual a olho nu ao app atual (master): valores de tamanho/cor/espaço copiados do kit em `~/.cargo/registry/src/*/gpui-component-0.9.0/src/<componente>` e do código atual em `crates/higanbana-desktop/src/chrome.rs` / `app.rs`.
- Motion: 160/240/420 ms, `ease_out_cubic` (valores de `crates/higanbana-desktop/src/motion.rs`).
- Componente genérico só na lib (com testes + seção na galeria); específico do app só no app.
- CPU ociosa ~0% com o app parado.
- Não varrer pastas da nuvem Anitsu em testes/scripts.

## Review Focus

1. Resposta de busca antiga chegando depois de uma nova → deve ser descartada (teste em Task 9).
2. Mailbox enviando depois da janela fechar → não pode travar nem entrar em pânico (teste em Task 1).
3. Ctrl K com input focado (ex.: campo da paleta) → não reabre/duplica a paleta (teste em Task 2).
4. Sidebar recolhida com item ativo → ícone ativo continua destacado e tooltip mostra o rótulo (teste em Task 5).
5. Command com teclas ↑/↓ sobre grupos vazios ou lista vazia → não sai do intervalo nem entra em pânico (teste em Task 6).

---

### Task 1: Mailbox (eventos de outras threads)

**Files:**
- Create: `crates/higanbana-gpui/src/cx/mailbox.rs`, `crates/higanbana-gpui/src/cx/tests_mailbox.rs`
- Modify: `src/cx/mod.rs` (exporta), `src/app/mod.rs`, `src/app/run.rs` (drena no `tick`, igual às tarefas)

**Interfaces:**
- Produces: `App::mailbox<V: View>(&mut self) -> Mailbox<V>`; `Mailbox<V>: Clone + Send` com `send(&self, f: impl FnOnce(&mut V, &mut EventCx) + Send + 'static) -> bool` (false se a janela já fechou). A fila guarda o `Waker` assim que a janela abre; mensagens enviadas antes disso ficam guardadas e rodam no primeiro tick.

- [ ] **Step 1: testes** em `tests_mailbox.rs`: `mensagem_roda_na_view` (envia `|v: &mut u32, _| *v += 1` de outra thread, drena → view == 1); `ordem_preservada` (3 envios → ordem 1,2,3); `envio_apos_fechar_devolve_false` (drop do receptor → `send` == false, sem pânico); `envio_antes_da_janela_fica_guardado`.
- [ ] **Step 2:** `cargo test -p higanbana-gpui mailbox` → FAIL (não compila).
- [ ] **Step 3:** implementar com `std::sync::mpsc` + `Arc<Mutex<Option<Waker>>>`; o runner chama `drain(&mut view) -> Option<EventCx>` no mesmo ponto onde drena `Tasks`.
- [ ] **Step 4:** `cargo test -p higanbana-gpui mailbox` → PASS; suíte da lib verde.
- [ ] **Step 5:** commit `feat(gpui): Mailbox para eventos de outras threads`.

### Task 2: Atalhos sem foco

**Files:** Modify `crates/higanbana-gpui/src/input/keyboard.rs`; Test `src/input/tests_focus.rs`

**Interfaces:**
- Produces: sem nada focado, `key_down` sobe a partir da **raiz** (nó 0), então `on_key_down` na raiz recebe Ctrl K / Alt ←/→. Com foco, cadeia atual (focado → raiz) não muda.

- [ ] **Step 1: testes** `tecla_sem_foco_chega_na_raiz` (raiz com `on_key_down`, nada focado, Ctrl K → handler chamado 1×); `tecla_com_foco_sobe_ate_raiz_uma_vez` (input focado + raiz: raiz chamada 1×, não 2×).
- [ ] **Step 2:** rodar → primeiro FAIL.
- [ ] **Step 3:** `key_target()` devolve `Some(0)` quando não há foco e a árvore não está vazia.
- [ ] **Step 4:** `cargo test -p higanbana-gpui` → PASS.
- [ ] **Step 5:** commit `feat(gpui): teclas sem foco vão para a raiz`.

### Task 3: Botões voltar/avançar do mouse

**Files:** Modify core C (`higanbana-gpui-sys`: `hgui.h`, evento de mouse Win32 `WM_XBUTTONUP`; nos outros backends/plataformas, botões 4/5 quando existirem), `src/input/` (tipo do botão), `src/element/interactive.rs`; Test `src/input/tests_*.rs` + `higanbana-gpui-sys/tests/` (padrão de `tests/cursor.rs`)

**Interfaces:**
- Produces: `pub enum NavDirection { Back, Forward }` (no prelude); `Element::on_navigate(f: impl Fn(&mut V, NavDirection, &mut EventCx) + 'static)` — dispara no soltar do XButton1/XButton2 sobre o elemento, subindo até o primeiro com handler.

- [ ] **Step 1: testes** `xbutton1_chama_on_navigate_back`, `xbutton2_forward`, `sem_handler_nao_quebra` (Rust, árvore sintética); no sys: `WM_XBUTTONUP` com `XBUTTON1` vira evento com botão 4.
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar ponta a ponta (C → FFI → `PointerEvent` → árvore).
- [ ] **Step 4:** `cargo test -p higanbana-gpui-sys -p higanbana-gpui` → PASS.
- [ ] **Step 5:** commit `feat(gpui): botões voltar/avançar do mouse`.

### Task 4: Kbd

**Files:** Create `crates/higanbana-gpui/src/components/basic/kbd.rs`; Test `src/components/basic/tests.rs`; galeria `examples/gallery/sections/basic.rs`

**Interfaces:**
- Produces: `Kbd::new("ctrl-k")` (Component) — mostra `Ctrl` `K` como no kit no Windows (`Ctrl+K`), visual de `gpui-component-0.9.0/src/kbd.rs` (borda, raio, padding, fonte xs, cor muted_foreground). `Kbd::label(s: &str) -> String` pública para teste.

- [ ] **Step 1: teste** `kbd_formata_atalho`: `label("ctrl-k") == "Ctrl+K"`, `label("alt-left") == "Alt+←"`, `label("shift-enter") == "Shift+Enter"`.
- [ ] **Step 2:** FAIL. **Step 3:** implementar. **Step 4:** PASS.
- [ ] **Step 5:** commit `feat(gpui): componente Kbd`.

### Task 5: Sidebar

**Files:** Create `crates/higanbana-gpui/src/components/nav/{mod,sidebar,sidebar_menu,tests}.rs`; Modify `src/components/mod.rs`; galeria `examples/gallery/sections/sidebar.rs`

**Interfaces:**
- Produces: `Sidebar::new(id).collapsed(bool).header(e).footer(e).group(SidebarGroup)`; `SidebarGroup::new(label).item(SidebarItem)`; `SidebarItem::new(id, label).icon(&'static str).active(bool).on_click(f)`; `SidebarToggle::new(id, collapsed).on_click(f)`. Recolhida = só ícones (modo `SidebarCollapsible::Icon` do kit), rótulo do grupo some, item ganha `Tooltip` com o rótulo. Larguras, alturas de item, cores (`sidebar`, `sidebar_foreground`, `sidebar_accent`, `sidebar_border`…) e animação de largura copiadas de `gpui-component-0.9.0/src/sidebar/`. Estado de tooltip: a lib gera ids a partir do id do item (sem estado novo na view além do `bool` recolhido); se o `Tooltip` exigir `TooltipState`, a Sidebar recebe `&SidebarState` + lens (padrão da lib).

- [ ] **Step 1: testes** `largura_expandida_e_recolhida` (valores do kit), `item_ativo_usa_cor_accent`, `recolhida_esconde_rotulos_mas_mantem_icones`, `recolhida_item_ativo_continua_destacado` (Review Focus 4), `clique_no_item_chama_handler`, `toggle_chama_handler`.
- [ ] **Step 2:** FAIL. **Step 3:** implementar. **Step 4:** `cargo test -p higanbana-gpui sidebar` PASS.
- [ ] **Step 5:** seção na galeria + captura conferida; commit `feat(gpui): componente Sidebar`.

### Task 6: Command (paleta)

**Files:** Create `crates/higanbana-gpui/src/components/overlay/{command,command_state,tests/command.rs}`; galeria `examples/gallery/sections/command.rs`

**Interfaces:**
- Consumes: `TextInputState`, `Spinner`, `Kbd`, `Dialog` (Plano 1/lib).
- Produces: `CommandState { input: TextInputState, selected: Option<(usize, usize)>, loading: bool }` com `CommandState::new()`, `set_loading(bool)`, `query() -> &str`; `Command::new(id, &CommandState, lens).placeholder(s).max_h(f32).group(CommandGroup).empty(e).on_query(f(&mut V, &str, &mut EventCx)).on_confirm(f(&mut V, (usize, usize), &mut EventCx))`; `CommandGroup::new(label).item(CommandItem)`; `CommandItem::new(id).child(e)`. Sem filtro próprio (o app filtra/busca). ↑/↓ andam pelos itens atravessando grupos, Enter confirma, item selecionado rola à vista (`scroll_into_view`). Visual de `gpui-component-0.9.0/src/command/` (sem borda, como `bordered(false)` no app). `Command::input_id(id)` público (o app foca ao abrir).

- [ ] **Step 1: testes** `setas_atravessam_grupos`, `setas_param_nas_pontas`, `setas_com_lista_vazia_nao_quebram` (Review Focus 5), `enter_confirma_selecionado`, `digitar_chama_on_query`, `loading_mostra_spinner`, `vazio_mostra_empty`.
- [ ] **Step 2:** FAIL. **Step 3:** implementar. **Step 4:** PASS.
- [ ] **Step 5:** galeria (paleta num `Dialog`, lista fixa filtrada pelo exemplo) + captura; commit `feat(gpui): componente Command`.

### Task 7: Notificação no visual do kit

**Files:** Modify `crates/higanbana-gpui/src/components/overlay/{toast,toaster}.rs` (+ testes existentes)

**Interfaces:**
- Produces: `Toasts::push_success(msg)` / `push_error(msg)` (se ainda não existirem com esse nome, ver API atual e só acrescentar) com ícone, cor, posição e tempo de `gpui-component-0.9.0/src/notification.rs`. Se o Toast atual já bate a olho nu, a task vira só os dois atalhos + teste — Ruling no ledger.

- [ ] **Step 1: teste** `push_success_usa_icone_e_cor_de_sucesso`, `push_error_usa_destructive`.
- [ ] **Step 2–4:** FAIL → implementar → PASS.
- [ ] **Step 5:** commit `feat(gpui): notificações de sucesso/erro como no kit`.

### Task 8: App compila na lib (main, tema, ícones, janela)

**Files:** Modify `crates/higanbana-desktop/Cargo.toml` (remove `gpui-kit`/`gpui`, adiciona `higanbana-gpui` com feature `video` desligada por ora), `src/main.rs`, `src/theme.rs`, `src/motion.rs`, `src/ui.rs`; tira do `main.rs` os módulos não portados (`// Plano 3: video` etc.)

**Interfaces:**
- Produces: `theme::load() -> Theme` (`Theme::from_json(include_str!("../themes/higanbana.json"))`, cai em `Theme::dark()` se Err — trata o Minor M-4 do Plano 1); `theme::on_media()`, `media_scrim(a)`, `media_track(a)`, `primary_ink(&Theme)` devolvendo `Color`; `motion::{FAST, BASE, SLOW, PAGE_SHIFT}` como `Duration`/`f32` e `motion::enter() -> Transition` com `Easing` equivalente a `ease_out_cubic` (se a lib não tiver esse easing, adiciona `Easing::EaseOutCubic` na lib com teste). `ui::{truncate(text) -> Element<V>, empty_state(icon, title, detail) -> Element<V>}`. Janela: 1280×800, mínimo 960×600, título "higanbana", `Decorations::Custom`, cantos Win11 (`round_corners` mantido), ícones do app via `App::icons` se `icons/` tiver extras além dos Lucide.

- [ ] **Step 1: testes** `tema_do_json_carrega` (primary/radius vêm do arquivo), `motion_valores` (160/240/420 ms).
- [ ] **Step 2:** FAIL. **Step 3:** implementar; `main` abre `App::new().theme(theme::load()).run(opts, Higanbana::new(core, mailbox))` — raiz provisória vazia até Task 10.
- [ ] **Step 4:** `cargo build -p higanbana-desktop` sem warnings; `cargo test -p higanbana-desktop` PASS.
- [ ] **Step 5:** commit `refactor(desktop): app sobe na higanbana-gpui (tema, motion, janela)`.

### Task 9: Estado do app sem Entity

**Files:** Split `crates/higanbana-desktop/src/state.rs` → `src/state/{mod,nav,session,downloads,lookup,images,updates,notify}.rs`; `src/image_cache.rs` passa a guardar `ImageSource`

**Interfaces:**
- Consumes: `EventCx::spawn`, `Mailbox` (Task 1).
- Produces: `AppState` (campo de `Higanbana`) com os mesmos métodos públicos de hoje, trocando `cx: &mut Context<Self>` por `cx: &mut EventCx` e lens `fn(&mut Higanbana) -> &mut AppState` onde o `done` precisa voltar ao estado. `events: Vec<AppEvent>` (`Play`, `Read`, `Notify(Tone, String)`, `Navigated`) drenado pela raiz. `Img::Ready(ImageSource)`. Eventos do núcleo (`Event::*`) chegam pela `Mailbox` criada em `main` e passada ao callback do `Core`.
- Trabalho assíncrono: `cx.spawn(move || core.block_on(fut), |v: &mut Higanbana, r, cx| ...)`; debounces/atrasos com `spawn_timer`.

- [ ] **Step 1: testes** (puros, sem rede): `voltar_e_avancar_seguem_historico`, `go_limpa_avancar`, `went_back_marca_direcao`, `resposta_antiga_de_lookup_descartada` (Review Focus 1 do lado do estado), `progresso_de_download_atualiza_item`, `limpar_concluidos`. Mover para cá os testes que já existiam em `state.rs`.
- [ ] **Step 2:** FAIL. **Step 3:** portar. **Step 4:** `cargo test -p higanbana-desktop` PASS.
- [ ] **Step 5:** commit `refactor(desktop): AppState sem Entity, dividido por responsabilidade`.

### Task 10: Chrome e raiz

**Files:** Delete `src/chrome.rs`, `src/app.rs`; Create `src/chrome/{mod,title_bar,search,downloads,sidebar,update}.rs`, `src/root.rs` (view `Higanbana`), `src/pages.rs` (placeholder por rota: nome da rota + `empty_state`, removido no Plano 4)

**Interfaces:**
- Consumes: tudo acima.
- Produces: `Higanbana: View` com `state`, `palette: SearchPalette` (`CommandState`, `query`, `groups`, `open: bool`), `toasts`, `login` (tela de login portada aqui: é parte do shell e pequena — ver `views/login.rs`). Layout exatamente o de `app.rs`/`chrome.rs` atuais: title bar (marca bold primary, versão xs muted, voltar/avançar ghost small com tooltip, busca outline small w-96 com ícone + texto + `Kbd("ctrl-k")`, botão de atualização primary, downloads em `Popover` TopRight com o painel w-96), sidebar com Acervo/grupos/Nuvem/rodapé (avatar, nome, "Via Discord", configurações — abre `empty_state` "Em migração" até o Plano 4 — e sair), página com fade + deslize `PAGE_SHIFT` no sentido da navegação, Ctrl K / Alt ←/→ na raiz, `on_navigate` na raiz, notificações via Toaster.
- Busca: debounce 250 ms com `spawn_timer`, `core.cloud().search` no worker, resposta descartada se `query` mudou, capas AniList dos 8 primeiros.

- [ ] **Step 1: testes** de unidade das partes puras: `kind_heading`/`KIND_ORDER` agrupam resultados (mover do arquivo antigo), `busca_descarta_resposta_antiga` (Review Focus 1), `ctrl_k_com_paleta_aberta_nao_reabre` (Review Focus 3), `sort_sections`, `is_library_section`.
- [ ] **Step 2:** FAIL. **Step 3:** implementar. **Step 4:** `cargo test -p higanbana-desktop` + `cargo build -p higanbana-desktop --release` sem warnings.
- [ ] **Step 5:** commit `feat(desktop): shell na higanbana-gpui (title bar, sidebar, busca, downloads, notificações)`.

### Task 11: Conferência visual e CPU

**Files:** nenhum de código (só correções que aparecerem, cada uma com teste)

- [ ] **Step 1:** capturas "antes" na master (worktree temporário em `../anitsu-master`, `cargo run -p higanbana-desktop --release`): login, shell com Início, sidebar recolhida, paleta aberta vazia, popover de downloads vazio. Sem navegar por pastas da nuvem além da raiz.
- [ ] **Step 2:** mesmas capturas na branch; comparar lado a lado. Diferença visível = corrigir na lib/app com teste, ou Ruling no ledger se for limitação (ex.: anel-sombra do Popover).
- [ ] **Step 3:** CPU com app parado no shell (Gerenciador de Tarefas, 30 s) → ~0%.
- [ ] **Step 4:** remover worktree temporário; commit das correções `fix(desktop): ajustes visuais do shell`.

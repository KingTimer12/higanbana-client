# Desktop GPUI — Plano 5: leitor, música, configurações e limpeza final

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** leitor de mangá, música (encarte do álbum + barra) e diálogo de configurações rodam na `higanbana-gpui`, iguais a olho nu aos da master; o código antigo fora do build (`reader/view.rs`, `music/*`, `settings_dialog.rs`, `updater.rs`) some e o desktop não depende mais de gpui/gpui-kit.

**Architecture:** Mesmo padrão do Plano 4: estado de cada tela é struct simples na raiz (`Higanbana.reader: Option<ReaderUi>`, `Higanbana.music: MusicUi`), sem `Entity`; trabalho assíncrono via `cx.spawn(work, done)`; a lógica do leitor (capítulos, páginas, avanço, faixa) fica em funções puras testáveis, separada do desenho. `reader/view.rs` (1.3k linhas) sai quebrado em `reader/{state,load,nav,keys}.rs` + `reader/view/{mod,margins,page,strip,end,thread,panels/*}.rs`. A lib ganha só o genérico que falta (duplo clique em qualquer elemento, borda tracejada).

**Tech Stack:** Rust, higanbana-gpui, higanbana-core.

**Spec:** `docs/superpowers/specs/2026-11-21-desktop-gpui-migration-design.md` (telas leitor, música, configurações; "remover gpui/gpui-kit do Cargo e código morto"; arquivo deus `reader/view.rs`). Referência visual/comportamento: arquivos antigos fora do build (`reader/view.rs`, `music/{album,bar,session}.rs`, `settings_dialog.rs`) e `git show master:crates/higanbana-desktop/src/app.rs` (como leitor/barra entram na janela).

## Global Constraints

- Branch `feat/desktop-gpui`; sem push; commits sem trailer de atribuição.
- Nenhum arquivo novo > ~350 linhas; doc/comentários em português.
- Visual igual a olho nu à master (margens de 5,5 rem, lombada com título vertical de até 28 letras, número da página 2,5 rem / 1,375 rem se > 3 caracteres, fio com estames só até 120 capítulos, barra de música 80 px de altura com laterais de 17,5 rem).
- Motion via `motion.rs`: painéis do leitor entram com fade QUICK; cartão de fim e barra de música com BASE. Reduce-motion = 0.
- CPU ociosa ~0%: o leitor parado não agenda frame; "mexeu o mouse" usa um único `spawn_timer` de 2,5 s (cancelado e reposto), nunca `animate_loop`. Tiques de posição da música (`only_clock`) redesenham só se a barra estiver visível; a tela do álbum não reconstrói por tique.
- RAM: páginas do leitor ficam num cache **próprio do leitor** (fora do LRU de capas), limitado a uma janela de páginas em volta da atual (atual −2 … atual +`PRELOAD`(3) no modo página; ±6 itens na faixa), decodificadas com teto de 2048 px no lado maior; páginas que saem da janela são soltas; ao fechar o leitor tudo é solto (`ReaderUi` dropado). Long strip via `VirtualList` (só monta o visível). Listas de arquivos/capítulos por `Arc`/`Rc`, nada de clonar `Vec<FileEntry>` por frame. Working set com leitor aberto em capítulo de 200 páginas ≤ ao da master.
- Não varrer a nuvem: o leitor abre só o arquivo pedido e o próximo (pré-carga); o álbum lista só a pasta aberta.

## Review Focus

1. Resposta de página/capítulo chegando depois de fechar o leitor ou trocar de capítulo → descartada sem pânico e sem pintar imagem errada (teste `pagina_atrasada_descartada`, Task 3).
2. Capítulo com 0 páginas, arquivo que falha ao abrir, página que falha → mensagem com "Tentar de novo", nunca esqueleto eterno (testes na Task 3 e Task 5).
3. Ler capítulo de 300 páginas indo até o fim → memória das páginas presa à janela, não cresce com o número de páginas lidas (teste `janela_solta_paginas_antigas`, Task 3).
4. Arquivo que se divide em vários capítulos (`expand_units`) no meio da faixa com emenda ligada → posição e progresso salvos no capítulo certo (teste `divisao_mantem_capitulo`, Task 2).
5. Pasta de música que não é álbum (sem faixa tocável) → cai na lista de arquivos, sem tela vazia (teste na Task 8).

---

### Task 1: Lib — duplo clique e borda tracejada

**Files:**
- Modify: `crates/higanbana-gpui/src/element/interactive.rs` (handler), despacho de clique em `input/dispatch.rs` (contagem de cliques por tempo/posição), `style/styled.rs` + pintura de borda (`paint/canvas.rs`), galeria
- Test: `crates/higanbana-gpui/src/element/tests_double_click.rs`, `crates/higanbana-gpui/src/paint/tests_dashed.rs`

**Interfaces:**
- Produces: `Element::on_double_click(self, f: impl Fn(&mut V, &MouseEvent, &mut EventCx) + 'static) -> Self` — dispara no 2º clique dentro de 500 ms e 4 px; o `on_click` continua disparando no 1º e no 2º. `Styled::border_dashed(self) -> Self` — borda pintada em traços de 4 px com vão de 4 px (só lados com largura > 0).

- [ ] **Step 1:** testes `duplo_clique_dispara` (dois cliques a 100 ms → 1 chamada), `cliques_lentos_nao_sao_duplos` (600 ms → 0), `cliques_longe_nao_sao_duplos` (10 px → 0), `borda_tracejada_tem_vaos` (linha de 40 px gera 5 traços).
- [ ] **Step 2:** `cargo test -p higanbana-gpui duplo tracejada` → FAIL (métodos não existem).
- [ ] **Step 3:** implementar; o `VideoSurface` passa a usar o `on_double_click` genérico se for trivial (senão fica como está).
- [ ] **Step 4:** `cargo test -p higanbana-gpui` → PASS; seção na galeria.
- [ ] **Step 5:** commit `feat(gpui): on_double_click e border_dashed`.

### Task 2: Leitor — estado e navegação (puro)

**Files:**
- Create: `crates/higanbana-desktop/src/reader/state.rs` (`ReaderUi`, `Opened`, `Panel`), `reader/nav.rs` (capítulos, página, avanço, faixa, divisão de arquivo)
- Modify: `reader/mod.rs` (mods novos; `ReadTarget` com `files: Arc<[FileEntry]>`)
- Test: `reader/tests_nav.rs`

**Interfaces:**
- Consumes: `higanbana_core::reader::{logic::*, store::*, api::{OpenResult, PageInfo}}` como em `reader/view.rs` antigo.
- Produces:
  - `pub struct ReaderUi { target, chapters: Rc<[Unit]>, expanded, opened: HashMap<String, Opened>, loading: HashSet<String>, cur_key, page: Option<usize>, prefs: TitlePrefs, settings: ReaderSettings, ratios: HashMap<String, f32>, strip: Vec<String>, pending_scroll: Option<(String, usize)>, end: bool, panel: Option<Panel>, stirred: bool, stir_timer: Option<TimerId>, filter: TextInputState, pages: PageCache }`
  - `ReaderUi::new(target: ReadTarget, data: ReaderData) -> Self` (sem IO; diz o que carregar via `wants()`)
  - `fn chapter(&self) -> usize`, `fn unit(&self) -> Option<Unit>`, `fn total(&self) -> usize`, `fn is_strip(&self) -> bool`, `fn spread(&self) -> Vec<usize>`, `fn has_next(&self) -> bool`
  - `fn next(&mut self) -> Step`, `fn prev(&mut self) -> Step`, `fn go_chapter(&mut self, i: usize, pg: Option<usize>)`, `fn set_mode(&mut self, m: ReadMode)`, `fn set_direction(&mut self, d: Direction)`
  - `fn on_opened(&mut self, key: &str, r: Result<OpenResult, String>, data: &ReaderData)` (inclui divisão via `expand_units` + `split_current`)
  - `fn wants(&self) -> Wants { open: Vec<Unit>, pages: Vec<(Unit, usize, usize)> }` — o que falta carregar para a posição atual (página: atual … +PRELOAD; perto do fim, o próximo capítulo)
  - `fn progress_to_save(&self) -> Option<(String, usize, usize)>` (página dupla conta a mais adiantada do par)

- [ ] **Step 1:** testes portando o comportamento do antigo: `avanca_e_volta_pagina`, `fim_do_capitulo_mostra_cartao` (`next` na última → `end = true`), `pagina_dupla_salva_a_mais_adiantada`, `pagina_none_vai_para_ultima_ao_abrir`, `divisao_mantem_capitulo` (Review Focus 4), `wants_pre_carrega_tres` e `wants_abre_proximo_perto_do_fim`, `tecla_d_inverte_direcao` (via `set_direction`).
- [ ] **Step 2:** `cargo test -p higanbana-desktop reader` → FAIL.
- [ ] **Step 3:** implementar portando a lógica de `reader/view.rs` (linhas 178–528) sem gpui.
- [ ] **Step 4:** `cargo test -p higanbana-desktop reader` → PASS.
- [ ] **Step 5:** commit `feat(desktop): estado e navegação do leitor sem gpui`.

### Task 3: Leitor — carga de capítulos e páginas (cache próprio)

**Files:**
- Create: `reader/load.rs` (`PageCache`, `open_chapter`, `load_page`, `pump`)
- Test: `reader/tests_load.rs`

**Interfaces:**
- Consumes: `ReaderUi::wants`, `on_opened`; `image_cache::decode`.
- Produces: `pub struct PageCache` com `get(&self, key) -> Option<&Img>`, `insert(key, Img, ratio)`, `retain_window(&mut self, keep: &HashSet<String>)`, `forget(&mut self, key)`; `pub fn pump(v: &mut Higanbana, cx: &mut EventCx)` — chamado depois de toda mudança de posição: dispara `cx.spawn` para cada `wants()` ainda não pedido, solta páginas fora da janela e grava progresso (`store.set_progress`/`set_summary`). Resultado checa se `v.reader` ainda existe e se a chave ainda é desejada antes de inserir.
- Decodificação com teto de 2048 px no lado maior (`decode(&bytes, 2048, 2048)`), proporção tirada da imagem decodificada.

- [ ] **Step 1:** testes `pagina_atrasada_descartada` (resposta depois de `v.reader = None` → nada muda, sem pânico), `janela_solta_paginas_antigas` (avançar 50 páginas → cache ≤ 6 entradas), `falha_vira_tentar_de_novo` (página com erro → `Img::Failed`; `forget` volta a pedir), `capitulo_sem_paginas_nao_trava` (OpenResult com 0 páginas → `end`, sem loop de pedidos).
- [ ] **Step 2:** `cargo test -p higanbana-desktop reader::` → FAIL.
- [ ] **Step 3:** implementar.
- [ ] **Step 4:** `cargo test -p higanbana-desktop` → PASS.
- [ ] **Step 5:** commit `feat(desktop): leitor carrega páginas numa janela com teto de memória`.

### Task 4: Leitor — tela (margens, página única/dupla, fim)

**Files:**
- Create: `reader/view/mod.rs` (raiz: título, lombada, área, margem direita, teclas, mouse), `reader/view/margins.rs`, `reader/view/page.rs` (`page_image`), `reader/view/end.rs` (cartão de fim / erro de abertura), `reader/keys.rs`
- Modify: `root.rs` (`pub reader: Option<ReaderUi>`; se aberto, a janela mostra só o leitor, como o player), `state/mod.rs` (`AppEvent::Read(ReadTarget)` + `AppState::read(target)`), `views/folder/mod.rs:117`, `views/home/continue_card.rs:40`, `views/title/header.rs:50` (trocam o "abre a pasta crua" por `read`)
- Test: `reader/tests_keys.rs`

**Interfaces:**
- Consumes: Tasks 1–3; `TitleBar` da lib (só fora de tela cheia), `Kbd`, `Skeleton`, `Spinner`, `Button`.
- Produces: `pub fn view(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana>`; `pub fn on_key(v: &mut Higanbana, ev: &KeyEvent, cx: &mut EventCx)`; `pub fn close(v: &mut Higanbana, cx: &mut EventCx)` (sai da tela cheia, solta `ReaderUi`, relê progresso para o "Continuar").
- Teclas iguais à master: Esc (fecha painel → sai da tela cheia → fecha), g, d, m, f, `[`, `]`, ↑/↓ no long strip (80 px), setas pela `key_action` respeitando a direção; ignoradas com Ctrl/Alt e quando o filtro do índice está focado.
- Clique na área: painel aberto → fecha; laterais (x < 0,4 / > 0,6 da área sem as margens) viram página respeitando direção; duplo clique → tela cheia (`on_double_click`).

- [ ] **Step 1:** testes `esc_fecha_painel_antes_de_sair`, `setas_respeitam_direcao`, `teclas_ignoradas_com_ctrl`, `clique_na_lateral_vira_pagina` (função pura `side_turn(x, w, margin, dir) -> Option<Turn>`).
- [ ] **Step 2:** `cargo test -p higanbana-desktop reader` → FAIL.
- [ ] **Step 3:** implementar; ligar `AppEvent::Read` e os três "Ler".
- [ ] **Step 4:** `cargo test -p higanbana-desktop` → PASS; abrir um capítulo real (um arquivo, não varrer) e comparar com a master: margens, lombada, número, cartão de fim, erro de abertura.
- [ ] **Step 5:** commit `feat(desktop): leitor de mangá na lib nova (página única e dupla)`.

### Task 5: Leitor — faixas (long e wide strip) e emenda

**Files:**
- Create: `reader/view/strip.rs`
- Modify: `reader/nav.rs` (`strip_items(&self) -> Vec<StripItem>`, `on_strip_top(&mut self, ix)`)
- Test: `reader/tests_strip.rs`

**Interfaces:**
- Produces: `enum StripItem { Page(Unit, usize, usize), Loading, End(usize), Seam(usize) }`; `ReaderUi::strip_items` (invertido no wide RTL); `ReaderUi::on_strip_top(ix)` atualiza capítulo/página pelo item no topo/centro e emenda o próximo capítulo quando faltam < 2 telas (só com `settings.seam`).
- Long strip: `VirtualList` com altura de cada página = largura (50 rem, limitada à área) / proporção (0,7 até saber); item visível via `visible_range`. Wide strip: `scroll_x` com páginas na altura da área; posição via `cx.scroll_offset`. `pending_scroll` resolvido com `scroll_to`/`item_offset` no frame seguinte.
- Só pede páginas a ±6 itens do visível (Task 3 solta o resto).

- [ ] **Step 1:** testes `itens_da_faixa_com_emenda`, `wide_rtl_inverte`, `topo_atualiza_pagina_e_salva`, `emenda_so_com_opcao_ligada`, `capitulo_falhou_na_faixa_mostra_erro` (Review Focus 2).
- [ ] **Step 2:** `cargo test -p higanbana-desktop strip` → FAIL.
- [ ] **Step 3:** implementar.
- [ ] **Step 4:** `cargo test -p higanbana-desktop` → PASS; long e wide num capítulo real; medir CPU parado (~0%) e working set no fim de um capítulo longo vs master.
- [ ] **Step 5:** commit `feat(desktop): leitor em long e wide strip com lista virtual`.

### Task 6: Leitor — painéis, fio e "mexeu o mouse"

**Files:**
- Create: `reader/view/thread.rs`, `reader/view/panels/{mod,index,pages,options}.rs`
- Test: `reader/tests_panels.rs`

**Interfaces:**
- Consumes: `Popover`-like cartão próprio (fundo `popover`, borda, `radius_lg`, sombra), `Input::kit().prefix("search")`, `RadioGroup`, `Switch`, `Kbd`, `Separator`, `border_dashed` (Task 1).
- Produces: `fn index_items(chapters, query) -> Vec<usize>` e `fn count_word(chapters) -> &'static str` puras ("volume(s)", "capítulo(s)", "arquivo(s)"); painéis Índice (lista com filtro, lida = ✓, atual realçada), Páginas (grade de 6 colunas calculada, sem grade na lib), Opções (modo, emendar, direção, estilo de progresso, atalhos).
- Progresso `Hidden`: `on_mouse_move` põe `stirred = true` e repõe um `spawn_timer(2500 ms)` (cancela o anterior); nada roda com o mouse parado.

- [ ] **Step 1:** testes `indice_filtra_por_nome_e_rotulo`, `palavra_do_contador`, `mexer_repoe_um_timer_so` (dois movimentos → um timer vivo), `fio_some_acima_de_120`.
- [ ] **Step 2:** `cargo test -p higanbana-desktop panels` → FAIL.
- [ ] **Step 3:** implementar.
- [ ] **Step 4:** `cargo test -p higanbana-desktop` → PASS; comparar painéis e fio com a master lado a lado.
- [ ] **Step 5:** commit `feat(desktop): painéis do leitor, fio de progresso e realce ao mexer o mouse`.

### Task 7: Música — sessão e barra

**Files:**
- Create: `music/session.rs` (reescrito sem `Entity`: `MusicUi`), `music/bar.rs` (reescrito)
- Modify: `root.rs` (`Event::MusicState` → `music.on_state`; barra embaixo do workspace com entrada BASE quando há álbum; `close_player` não muda), `main.rs` (`mod music;`)
- Test: `music/tests.rs` (mantém `so_o_relogio`, `troca_de_faixa_ou_pausa_nao_e_so_relogio`)

**Interfaces:**
- Produces: `pub struct MusicUi { player: Option<PlayerState>, metas: HashMap<String, TrackMeta>, seeking: Option<f32>, last_save: Instant }`; `MusicUi::on_state(&mut self, m: PlayerState, core: &Core) -> Redraw` (`Redraw::Bar` quando `only_clock`, `Redraw::All` senão; grava progresso a cada 5 s); `MusicUi::open(&mut self, core, album: &Album, index, at)`; `music::bar::bar(v, cx) -> Element<Higanbana>`.
- Barra: capa 48 px (abre o álbum), título/subtítulo truncados, aleatório/anterior/tocar/próxima/repetir (Off→All→One), tempo + `Slider` de posição (35 rem máx; desabilitado sem duração), "Volume igualado", mudo (ícone volume-x/1/2), `Slider` de volume 96 px. Dicas iguais às da master.
- Com `Redraw::Bar` o app só notifica (a lib reconstrói a árvore, mas a tela do álbum não refaz trabalho: listas em `Rc`).

- [ ] **Step 1:** testes `so_o_relogio` e `troca_de_faixa...` portados + `salva_a_cada_5s` + `repetir_cicla`.
- [ ] **Step 2:** `cargo test -p higanbana-desktop music` → FAIL.
- [ ] **Step 3:** implementar; teclas de mídia continuam pelo `attach_media_keys` do `main.rs`.
- [ ] **Step 4:** `cargo test -p higanbana-desktop` → PASS; tocar um álbum, CPU com música tocando e app parado ≤ master.
- [ ] **Step 5:** commit `feat(desktop): barra de música na lib nova`.

### Task 8: Música — encarte do álbum

**Files:**
- Create: `views/album/{mod,cover,tracks}.rs`
- Modify: `views/folder/mod.rs` (pasta com `looks_like_album` → álbum; `NotAlbum` → lista de arquivos), `state/` (`AlbumState` por caminho: carregado via `core.cloud().load_album`, solto ao sair da pasta como as listagens)
- Test: `views/album/tests.rs`

**Interfaces:**
- Consumes: `MusicUi::open/show`, `TabBar` (underline, "Faixas"/"Arquivos"), `ui/files.rs` (lista de arquivos), `img().zoom`.
- Produces: `pub fn page(v, cx, path: &str) -> Element<Higanbana>`; capa: pasta ou embutida na 1ª faixa (chave `album-cover:<path>` no LRU de capas), faixas por disco com número `pad`, faixa atual realçada (`list_active`), "Tocar"/"Continuar".

- [ ] **Step 1:** testes `nao_album_vira_lista` (Review Focus 5), `faixas_agrupadas_por_disco`, `album_solto_ao_sair`.
- [ ] **Step 2:** `cargo test -p higanbana-desktop album` → FAIL.
- [ ] **Step 3:** implementar.
- [ ] **Step 4:** `cargo test -p higanbana-desktop` → PASS; comparar com a master.
- [ ] **Step 5:** commit `feat(desktop): encarte do álbum na lib nova`.

### Task 9: Diálogo de configurações

**Files:**
- Create: `views/settings.rs` (substitui `settings_dialog.rs`)
- Modify: `root.rs` (troca o diálogo provisório pelo real)
- Test: em `views/settings.rs`

**Interfaces:**
- Consumes: `Dialog` (largura 32 rem), `RadioGroup::horizontal`, `Switch`; `video::{CACHES, BACK_CACHES}`.
- Produces: `pub fn dialog(v, cx) -> Element<Higanbana>`; `fn save(core: &Core, f: impl FnOnce(&mut Settings))` relê do núcleo antes de gravar (só `cache_mib`, `back_cache_mib`, `disk_cache`); `fn mib(n: u32) -> String` ("Nada" para 0).
- Textos iguais à master (títulos, dicas).

- [ ] **Step 1:** testes `mib_zero_e_nada`, `salvar_nao_apaga_outros_campos`.
- [ ] **Step 2:** `cargo test -p higanbana-desktop settings` → FAIL.
- [ ] **Step 3:** implementar.
- [ ] **Step 4:** `cargo test -p higanbana-desktop` → PASS.
- [ ] **Step 5:** commit `feat(desktop): diálogo de configurações na lib nova`.

### Task 10: Limpeza final e medição

**Files:**
- Delete: `reader/view.rs`, `settings_dialog.rs`, `updater.rs` (já substituído por `state/updates.rs`), `music/album.rs` antigo, comentários `// mod app; ...` do `main.rs`, `theme::ProductColors` se ficar sem uso, qualquer arquivo `.rs` fora do build (`grep` por módulos não declarados)
- Modify: `Cargo.toml` da workspace (tirar `gpui`/`gpui-kit` se nenhum crate usa)

- [ ] **Step 1:** `cargo tree -p higanbana-desktop -i gpui-kit` e `-i gpui` → "nothing to print"/erro de pacote inexistente; `rg "gpui_kit|gpui::" crates/higanbana-desktop/src` sem resultado (fora de `higanbana_gpui::`).
- [ ] **Step 2:** `cargo build --release -p higanbana-desktop` sem warnings novos; `cargo clippy -p higanbana-desktop -p higanbana-gpui` limpo.
- [ ] **Step 3:** `cargo test --workspace` → PASS.
- [ ] **Step 4:** medir CPU (1 núcleo) e working set, branch vs master, parado: leitor (página única, long strip no fim de capítulo longo), álbum com música tocando, diálogo aberto; tabela no ledger. Comparar visual lado a lado nessas mesmas situações.
- [ ] **Step 5:** commit `chore(desktop): remove gpui-kit e código antigo`.

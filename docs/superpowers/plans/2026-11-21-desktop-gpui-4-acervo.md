# Desktop GPUI — Plano 4: telas do acervo

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** as telas do acervo (home, catálogo + filtros, título, episódios, pasta, franquia, estante) rodam na `higanbana-gpui`, iguais a olho nu às da master; `pages.rs` deixa de mostrar o aviso provisório.

**Architecture:** A lib ganha o genérico que essas telas pedem (texto com `line_clamp`, polígono e imagem com zoom, lista virtual + aviso de visibilidade, `Carousel`, `Tag`, `Link`, `Breadcrumb`, `DropdownMenu`, variantes de `Tabs` do kit), cada um com testes e seção na galeria. No app, os helpers de `components.rs`/`interact.rs` viram módulos pequenos em `ui/`, `catalog_state.rs` vira campo de `AppState`, e cada tela vira uma pasta `views/<tela>/` com arquivos por região. Estado de cada tela é struct simples guardada na raiz (`Higanbana.pages`), sem `Entity`; trabalho assíncrono via `cx.spawn(work, done)` + `Post`, como no Plano 2/3.

**Tech Stack:** Rust, higanbana-gpui, higanbana-core.

**Spec:** `docs/superpowers/specs/2026-11-21-desktop-gpui-migration-design.md` (Fase 1 itens 8, 10, 11; Fase 3). Referência visual/comportamento: arquivos atuais fora do build (`views/*`, `components.rs`, `interact.rs`, `catalog_state.rs`, `series.rs`) e `git show master:crates/higanbana-desktop/src/app.rs` (rotas).

**Fora deste plano (Plano 5):** leitor de mangá, música (álbum + barra), diálogo de configurações, remoção de `updater.rs`/código morto restante e checagem final "sem gpui-kit no Cargo.lock do desktop".

## Global Constraints

- Branch `feat/desktop-gpui`; sem push; commits sem trailer de atribuição.
- Arquivos pequenos por responsabilidade (nenhum arquivo novo > ~350 linhas; `title.rs` 1.1k e `episodes.rs` 928 saem quebrados); doc/comentários em português.
- Visual igual a olho nu à master. Tokens do kit vêm do tema carregado (`theme::load`) e de `~/.cargo/registry/src/*/gpui-component-0.9.0/src/<componente>`.
- Motion: QUICK 160 ms, BASE 240 ms, RISE 420 ms, stagger 40 ms (máx 12), `ease_out_cubic`, via `motion.rs` (reduce-motion = 0). Mola do progresso do "Continuar": `Spring` da lib com rigidez 120, amortecimento 20, massa 1.
- Genérico na lib (testes + galeria); específico do app no app.
- CPU ociosa ~0% com app parado em qualquer tela (nada de `animate_loop` sem necessidade; carousel só agenda frame quando auto-avança).
- Não varrer pastas da nuvem Anitsu. Telas listam só a pasta aberta pelo usuário; o crawl de metadados do catálogo é o que a master já faz (mesmos limites de lote e cooldown de `catalog_state.rs`).

## Review Focus

1. Resposta de listagem/lookup chegando depois de o usuário já ter navegado para outra pasta → descartada, não pinta tela errada (teste na Task 7).
2. Lista virtual com 0 itens, com alturas variáveis e com `scroll_to_item` além do fim → sem pânico, offset limitado (teste na Task 3).
3. Carousel com 0 ou 1 item, e auto-avanço com janela sem foco/tela trocada → sem pânico e sem frame agendado depois de sair da tela (teste na Task 4).
4. Imagem que falha ao baixar → fundo neutro, nunca esqueleto eterno (teste na Task 6).
5. Voltar/avançar do mouse restaura posição de rolagem e filtros do catálogo (`CatalogPlace`) (teste na Task 9).

---

### Task 1: Texto com `line_clamp`

**Files:**
- Modify: `crates/higanbana-gpui/src/style/styled.rs`, layout de texto (onde `max_width` quebra linhas, `paint/canvas.rs` `text`), `examples/gallery` (seção Tipografia)
- Test: `crates/higanbana-gpui/src/text/tests_clamp.rs` (ou junto aos testes de texto existentes)

**Interfaces:**
- Produces: `Styled::line_clamp(self, lines: u32) -> Self` — corta em `lines` linhas, última termina em "…"; altura medida = `lines * line_height` no máximo.

- [ ] **Step 1:** testes `clamp_limita_altura` (texto de 5 linhas, `line_clamp(2)` → altura = 2 × line_height) e `clamp_poe_reticencias` (texto cortado termina em '…'); `line_clamp(1)` igual ao `truncate` atual.
- [ ] **Step 2:** `cargo test -p higanbana-gpui clamp` → FAIL (método não existe).
- [ ] **Step 3:** implementar.
- [ ] **Step 4:** `cargo test -p higanbana-gpui` → PASS.
- [ ] **Step 5:** commit `feat(gpui): line_clamp em texto`.

### Task 2: Polígono, imagem com zoom e cantos por canto

**Files:**
- Create: `crates/higanbana-gpui/src/element/polygon.rs`
- Modify: `paint/canvas.rs` (`fill_polygon`), backends via caminho comum (triangulação em leque → quads/triângulos que o core já desenha; se o core só tem quads, adicionar primitiva de triângulo no caminho comum), `element/mod.rs` (`img(..).zoom(k)` usando o `uv` que `gfx::draw::image` já aceita; `rounded_corners` vale para `img`)
- Test: `crates/higanbana-gpui/src/element/tests_polygon.rs`

**Interfaces:**
- Produces: `polygon<V>(points: Vec<(f32, f32)>, color: Color) -> Element<V>` (pontos relativos ao próprio retângulo, preenche, pinta absoluto sobre o pai); `Element::zoom(self, k: f32) -> Self` para `img` (amplia a partir do centro, recorta na própria área com os cantos do elemento).

- [ ] **Step 1:** testes: `poligono_vazio_ou_menos_de_3_pontos_nao_pinta`; `poligono_triangulo_gera_primitiva` (painter de teste conta 1 polígono/≥1 triângulo); `zoom_1_igual_cover` e `zoom_2_uv_central` (uv = 0.25..0.75 numa imagem quadrada em área quadrada).
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar.
- [ ] **Step 4:** `cargo test -p higanbana-gpui` → PASS; `cargo run -p higanbana-gpui --example gallery` mostra seção "Formas" com triângulo e imagem 1.15×.
- [ ] **Step 5:** commit `feat(gpui): polygon e img zoom`.

### Task 3: Lista virtual e aviso de visibilidade

**Files:**
- Create: `crates/higanbana-gpui/src/components/list/{mod.rs, virtual_list.rs, tests.rs}`, `crates/higanbana-gpui/src/element/visible.rs`
- Modify: `components/mod.rs`, galeria (seção Lista)

**Interfaces:**
- Produces:
  - `VirtualList::new(id, heights: Arc<[f32]>, render: impl Fn(&mut V, usize, &mut RenderCx<V>) -> Element<V>)` + `.gap(f32)`; só constrói itens cujo intervalo cruza viewport ± 1 tela; usa o scroll da lib (`ElementId` do container).
  - `RenderCx::scroll_to_item(id, index)` / `EventCx` equivalente: offset = soma das alturas anteriores, limitado a `scroll_max`.
  - `on_visible(margin: f32, f: impl FnOnce(&mut V, &mut EventCx) + 'static) -> Element<V>`: dispara uma vez quando o retângulo do elemento cruza a janela ampliada por `margin` (substitui `when_visible` do app).

- [ ] **Step 1:** testes: `vazia_nao_constroi_nada`; `so_constroi_visiveis` (1000 itens de 40 px, viewport 400 → ≤ ~30 construídos); `alturas_variaveis_offset_certo`; `scroll_to_item_alem_do_fim_limita`; `on_visible_dispara_uma_vez` e `on_visible_fora_nao_dispara`.
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar.
- [ ] **Step 4:** `cargo test -p higanbana-gpui` → PASS.
- [ ] **Step 5:** commit `feat(gpui): VirtualList e on_visible`.

### Task 4: Carousel

**Files:**
- Create: `crates/higanbana-gpui/src/components/carousel/{mod.rs, state.rs, tests.rs}`
- Modify: `components/mod.rs`, galeria (seção Carousel)

**Interfaces:**
- Produces: `CarouselState { index, len, .. }` com `next()`, `prev()`, `go(i)` (circular); `Carousel::new(id, lens)` com `.items(Vec<Element<V>>)`, `.basis(f32)` (fração por item, ex. 1/5), `.auto_advance(Duration)`, `.controls(bool)` (botões anterior/próximo outline redondos como `CarouselPrevious/Next` do kit), `.on_change(Fn(&mut V, usize, &mut EventCx))`. Deslize animado BASE + `ease_out_cubic`. Auto-avanço agenda um único timer por intervalo (sem frame contínuo) e para quando o carousel não é renderizado.

- [ ] **Step 1:** testes do estado: `next_circular`, `prev_do_zero_vai_pro_fim`, `len_0_nao_panica`, `len_1_next_fica_em_0`; do componente: `auto_avanco_troca_indice_apos_intervalo` (relógio de teste), `sem_render_nao_agenda` .
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar (valores visuais de `gpui-component-0.9.0/src/carousel`).
- [ ] **Step 4:** `cargo test -p higanbana-gpui` → PASS.
- [ ] **Step 5:** commit `feat(gpui): Carousel`.

### Task 5: Tag, Link, Breadcrumb, DropdownMenu, variantes de Tabs

**Files:**
- Create: `crates/higanbana-gpui/src/components/basic/{tag.rs, link.rs}`, `components/nav/breadcrumb.rs`, `components/overlay/dropdown_menu.rs`, testes ao lado (`tests_tag_link.rs`, `nav/tests_breadcrumb.rs`, `overlay/tests/dropdown_menu.rs`)
- Modify: `components/overlay/tabs.rs` (variantes), `components/basic/button.rs` (`.outline()` se ainda não existir com visual do kit), galeria

**Interfaces:**
- Produces:
  - `Tag::secondary(label)` / `Tag::outline(label)` (tamanho small do kit).
  - `Link::new(id, label).on_click(..)` (cor `link`/`primary`, sublinhado no hover).
  - `Breadcrumb::new().item(label, Option<on_click>)`; separador `chevron-right`, último item sem clique e cor `foreground`.
  - `DropdownMenu::new(id, trigger: Element<V>, lens).item(label, on_click).separator()`; abre ancorado `BottomLeft`/`BottomRight` usando `Popover`; fecha no clique fora/Esc/escolha.
  - `Tabs::underline()`, `Tabs::pill()`, `Tabs::segmented()` com o visual de `TabBar` do kit.

- [ ] **Step 1:** testes: `tag_secondary_cor_do_tema`; `link_clica`; `breadcrumb_ultimo_sem_clique`; `dropdown_fecha_ao_escolher` e `dropdown_esc_fecha`; `tabs_underline_ativo_tem_barra`.
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar.
- [ ] **Step 4:** `cargo test -p higanbana-gpui` → PASS.
- [ ] **Step 5:** um commit por componente, `feat(gpui): <componente>`.

### Task 6: Helpers do app (`ui/`)

**Files:**
- Create: `crates/higanbana-desktop/src/ui/{mod.rs, image.rs, poster.rs, cover.rs, breadcrumbs.rs, files.rs, download.rs, hover.rs, format.rs, tests.rs}` (o `ui.rs` atual vira `ui/mod.rs` mantendo `truncate`, `empty_state`, `APP_ICONS`)
- Delete: `components.rs`, `interact.rs` (conteúdo migrado; `Pointer`/`Button` próprio do `interact.rs` somem — o `Button` da lib já tem foco/teclado/tooltip)
- Reference: funções listadas em `components.rs` (when_visible → `on_visible` da lib, remote_image, rounded_image, hex_color, cover, in_library_mark, poster/PosterProps, columns, content_width, section_label/meta, nav_entries, folder_link, breadcrumbs, format_bytes, is_comic, stop_click, download_button, FileRows, file_list*, empty_folder, option_group/button, version_card, title_card, want_lookup) e `interact.rs` (HoverFade, hover_layer, hover_anim)

**Interfaces:**
- Consumes: Task 1–5.
- Produces: mesmas funções com assinatura da lib: `fn x(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, ..) -> Element<Higanbana>`; `remote_image(url, radius)` pede a imagem via `state.images` quando `on_visible(300)` dispara; estados Loading → esqueleto, Failed → fundo neutro, Ready → imagem com fade QUICK. `hover_fade(id, child)` = camada que anima opacidade 0→1 com `cx.animate` QUICK ao hover.

- [ ] **Step 1:** mover os testes puros existentes de `components.rs`/`interact.rs` para `ui/tests.rs` e acrescentar `imagem_falha_vira_neutro`, `imagem_carregando_vira_esqueleto`.
- [ ] **Step 2:** `cargo test -p higanbana-desktop ui::` → FAIL (módulos não existem).
- [ ] **Step 3:** implementar.
- [ ] **Step 4:** `cargo test -p higanbana-desktop` → PASS.
- [ ] **Step 5:** commit `feat(desktop): helpers de ui na lib nova`.

### Task 7: Estado do catálogo, listagens e roteador

**Files:**
- Create: `crates/higanbana-desktop/src/state/catalog.rs` (de `catalog_state.rs`), `src/views/mod.rs` reescrito (`Pages` com o estado de cada tela + `load_listing` via `cx.spawn` + `Post`), `src/views/route.rs` (rota → tela, a partir do `app.rs` da master), `src/views/tests_route.rs`
- Modify: `state/mod.rs` (campo `catalog`), `root.rs` (`pages: Pages`), `pages.rs` (chama o roteador), `main.rs` (`mod views; mod series;`)
- Delete: `catalog_state.rs`

**Interfaces:**
- Produces: `enum Route { Home, Catalog(MediaType), Title(String), Episodes(String), Folder(String), Franchise(String), Shelf }` com `Route::of(path: &str) -> Route` (mesma decisão do `app.rs` da master); `load_listing(v, path, done: impl FnOnce(&mut Higanbana, Result<Vec<FileEntry>, String>))` que descarta a resposta se `v.state.path()` mudou desde o pedido.

- [ ] **Step 1:** testes: `rota_de_cada_caminho` (um caso por variante, caminhos copiados da master); `listagem_antiga_descartada` (pede A, navega pra B, resposta A chega → estado de B intacto); testes de `crawl_wanted`/`next_crawl_batch`/`settle_crawl`/`may_retry` movidos.
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar; telas ainda não portadas continuam caindo no `empty_state` provisório.
- [ ] **Step 4:** `cargo test -p higanbana-desktop` → PASS.
- [ ] **Step 5:** commit `feat(desktop): rotas, listagem e estado do catálogo`.

### Task 8: Home (hero, continuar, descobrir)

**Files:**
- Create: `crates/higanbana-desktop/src/views/home/{mod.rs, greeting.rs, hero.rs, hero_slide.rs, continue_row.rs, discover.rs, tests.rs}`
- Delete: os arquivos antigos de `views/home/`

**Interfaces:**
- Consumes: `Carousel` (Task 4), `Tabs::underline` (Task 5), `ui::poster`/`remote_image` (Task 6), `Spring` da lib.
- Produces: `home::page(v, cx) -> Element<Higanbana>`; `HomeState` em `Pages`.

- [ ] **Step 1:** mover testes puros existentes e adicionar: `hero_avanca_sozinho_e_para_fora_da_tela` (com `Carousel` do Task 4), `continue_vazio_some`, `discover_troca_aba_reseta_indice`.
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar com os mesmos valores (gradientes do hero: fundo 0.95→0 de 0.2 a 0.9 e 0→0.75 de 0.45 a 1; brilho 0.35→0 até 0.7; drift do hero durante `AUTO_ADVANCE`; stagger nas entradas).
- [ ] **Step 4:** `cargo test -p higanbana-desktop` → PASS.
- [ ] **Step 5:** commit `feat(desktop): home na lib nova`.

### Task 9: Catálogo + filtros

**Files:**
- Create: `crates/higanbana-desktop/src/views/catalog/{mod.rs, header.rs, grid.rs, letters.rs, progress.rs, filters/{mod.rs, popover.rs, sort.rs, chips.rs}, tests.rs}`
- Delete: `views/catalog.rs`, `views/catalog_filters.rs`

**Interfaces:**
- Consumes: `VirtualList` (Task 3), `DropdownMenu` (Task 5), `Input` da lib, `state.catalog` (Task 7).
- Produces: `catalog::page`; `CatalogView` em `Pages`, com `save_catalog_place` ao sair e restauração ao voltar.

- [ ] **Step 1:** mover testes puros existentes; adicionar `voltar_restaura_rolagem_e_filtros` e `busca_filtra_sem_rede` (filtro local sobre o índice já carregado).
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar.
- [ ] **Step 4:** `cargo test -p higanbana-desktop` → PASS.
- [ ] **Step 5:** commit `feat(desktop): catálogo e filtros na lib nova`.

### Task 10: Título

**Files:**
- Create: `crates/higanbana-desktop/src/views/title/{mod.rs, backdrop.rs, header.rs, info.rs, versions.rs, related.rs, tabs.rs, data.rs, tests.rs}`
- Delete: `views/title.rs`

**Interfaces:**
- Consumes: `Tabs::underline`, `Tag`, `Link`, `Avatar` (lib), `line_clamp` (Task 1), `ui::version_card`/`title_card`.
- Produces: `title::page`; `TitleView` em `Pages`. Funções de dados (`summary`, `episodes`, `display_name` etc., hoje recebendo `&App`) passam a receber `&AppState` e ficam em `data.rs`.

- [ ] **Step 1:** mover testes puros; adicionar `sinopse_corta_em_5_linhas` (`line_clamp(5)`), `aba_sem_conteudo_nao_aparece`.
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar (gradientes do backdrop: 0.3→1 vertical e 0.85→0 de 0.25 a 0.9 horizontal).
- [ ] **Step 4:** `cargo test -p higanbana-desktop` → PASS.
- [ ] **Step 5:** commit `feat(desktop): tela de título na lib nova`.

### Task 11: Episódios

**Files:**
- Create: `crates/higanbana-desktop/src/views/episodes/{mod.rs, grid_card.rs, list_card.rs, still.rs, shapes.rs, actions.rs, labels.rs, tests.rs}`
- Delete: `views/episodes.rs`

**Interfaces:**
- Consumes: `polygon` e `img().zoom()` (Task 2), `on_visible` (Task 3), `ui::download_button`, `Button` da lib (Tab/Enter já nativos).
- Produces: `episodes::page`. `cut_triangle`, `bottom_strip`, `petal_corners`, `scale_about_center`, `kicker`, `head_number`, `list_columns` viram funções puras em `shapes.rs`/`labels.rs` sobre `f32`.

- [ ] **Step 1:** mover os testes puros existentes (ajustando para `f32`); adicionar `clicar_download_nao_toca` e `assistir_com_enter_toca` .
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar (pétala animada no hover, corte diagonal, faixa de progresso recortada nos cantos, sangria da imagem com `BLEED_ZOOM`/`BLEED_ALPHA`, stagger RISE).
- [ ] **Step 4:** `cargo test -p higanbana-desktop` → PASS.
- [ ] **Step 5:** commit `feat(desktop): episódios na lib nova`.

### Task 12: Pasta, franquia, estante

**Files:**
- Create: `crates/higanbana-desktop/src/views/{folder/{mod.rs, rows.rs}, franchise/mod.rs, shelf/{mod.rs, search.rs}}` + `tests.rs` em cada
- Delete: `views/folder.rs`, `views/franchise.rs`, `views/shelf.rs`

**Interfaces:**
- Consumes: `VirtualList` + `scroll_to_item` (Task 3), `Breadcrumb` (Task 5), `Tabs` (Task 5), `ui::FileRows` (Task 6).
- Produces: `folder::page`, `franchise::page`, `shelf::page`.

- [ ] **Step 1:** testes: `pasta_vazia_mostra_empty_folder`, `pasta_grande_virtualiza` (2.000 arquivos → só visíveis construídos), `estante_busca_filtra`, `franquia_troca_aba`.
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar.
- [ ] **Step 4:** `cargo test -p higanbana-desktop` → PASS.
- [ ] **Step 5:** commit `feat(desktop): pasta, franquia e estante na lib nova`.

### Task 13: Comparação com a master e CPU

**Files:** nenhum no repositório; PNGs em `<scratchpad>/acervo-{antes,depois}/`.

- [ ] **Step 1:** worktree temporária da master (`git worktree add ../hb-master master`), copiar `.env` e a libmpv; capturar 1280×800: home (hero + continuar + descobrir), catálogo de anime com um filtro aberto, um título (o mesmo aberto à mão), episódios grade e lista com hover num card, uma pasta, uma franquia, estante. Abrir cada tela clicando — sem scripts que listem pastas.
- [ ] **Step 2:** mesmas capturas no branch; comparar lado a lado; cada diferença visível vira correção com teste (RED→GREEN) quando houver lógica, ou ajuste de valor copiado do kit.
- [ ] **Step 3:** CPU com o app parado 30 s em cada tela (Process Explorer / `Get-Process`): esperado ~0%. Home com hero auto-avançando: reportar média.
- [ ] **Step 4:** `cargo test --workspace` → PASS; `cargo clippy -p higanbana-desktop -p higanbana-gpui` sem warnings novos. Remover worktree. Commit final das correções `fix(desktop): ajustes visuais do acervo`.

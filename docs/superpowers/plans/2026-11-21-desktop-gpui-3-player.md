# Desktop GPUI — Plano 3: player de vídeo

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** o player de vídeo do `higanbana-desktop` roda na `higanbana-gpui` (vídeo desenhado pela lib via render API sw), visualmente idêntico ao da master, com todo o comportamento (atalhos, auto-hide de controles e cursor, pular abertura, próximo episódio com contagem, miniatura no hover, capítulos, menus, progresso a cada 5 s, re-tentativas).

**Architecture:** A lib ganha o genérico: `Player` configurável (opções, `start`, `set_property`, capítulos, cache à frente, buffering, primeiro quadro, erro de fim de arquivo), `Player::headless` para miniaturas, e as peças do `VideoPlayer` separadas (`VideoSurface`, `AutoHide`, `Seekbar`) — o `VideoPlayer` atual passa a ser montado com elas. O app monta o layout exato do `video/view.rs` da master com essas peças, em arquivos por região. Lógica do app (progresso, skip, próximo, re-tentativas, miniaturas) fica num `PlayerSession` puro e testável; `surface.rs` nativo some.

**Tech Stack:** Rust, higanbana-gpui (`feature = "video"`, libmpv dinâmica), higanbana-core (`core.player()`, `player::logic`).

**Spec:** `docs/superpowers/specs/2026-11-21-desktop-gpui-migration-design.md` (Fase 2 e Verificação). Referência do visual/comportamento: `git show master:crates/higanbana-desktop/src/video/view.rs` (abaixo "V:<linha>"), `thumbs.rs`, `theme.rs`, `motion.rs`.

## Global Constraints

- Branch `feat/desktop-gpui`; sem push; commits sem trailer de atribuição.
- Arquivos pequenos por responsabilidade (`view.rs` de 1.5k linhas NÃO volta como um arquivo); doc/comentários em português.
- Visual igual a olho nu ao player da master. Tokens do kit (`radius`, `radius_lg`, `popover`, `border`, `muted_foreground`, `primary`, `list_active`, `list_hover`, `danger_foreground`, `success_foreground`, shadows, tamanhos de Button) vêm do tema já carregado (`theme::load`) e de `~/.cargo/registry/src/*/gpui-component-0.9.0`.
- Cores de mídia: texto branco; `on_media_muted` = branco 0.72; scrim = preto α; trilha = branco α; `primary_ink` = token `red_light`.
- Motion: QUICK 160 ms, BASE 240 ms, `ease_out_cubic`, via `motion::transition` (reduce-motion = 0).
- Genérico na lib (com testes + seção na galeria/exemplo `video_player`); específico do app no app.
- CPU: player pausado = 0% (sem frame novo, sem render). 1080p tocando: medir e reportar.
- Não varrer pastas da nuvem Anitsu. Para testar, abrir **um** episódio escolhido à mão.

## Review Focus

1. Arquivo some/cai a rede no meio → re-tentativas em 2/4/8/16/30 s na mesma posição, 5ª falha = tela "Não foi possível tocar" (teste em Task 5).
2. EOF com próximo episódio e contagem já disparada → avança uma vez só (`advanced_from`) (teste em Task 5).
3. Fechar o player durante geração de miniatura / com mpv ainda carregando → sem pânico, sem thread presa, progresso salvo (teste em Task 6 e Task 10).
4. Arrastar a seekbar e soltar fora da janela → busca no último ponto e encerra o arraste (teste em Task 3).
5. Duplo clique no vídeo → tela cheia sem mudar o estado de pausa (teste em Task 2).

---

### Task 0: Capturas "antes" na master

**Files:** nenhum no repositório; PNGs em `<scratchpad>/player-antes/`.

- [ ] **Step 1:** worktree temporária da master (`git worktree add ../hb-master master`), copiar `.env`, `cargo run --release -p higanbana-desktop`.
- [ ] **Step 2:** abrir **um** episódio à mão (Continuar assistindo ou título → episódio). Capturar a janela em 1280×800 nas situações: controles visíveis tocando; pausado; hover na barra (com miniatura); menu Legenda e áudio; menu Ajustes; menu Episódios; botão Pular abertura (se houver); cartão Próximo episódio com contagem (buscar perto do fim); carregando ("Abrindo …").
- [ ] **Step 3:** anotar o caminho do arquivo aberto (dir + nome) para repetir na Task 11. Remover a worktree.
- Sem commit.

### Task 1: Player configurável e propriedades novas (lib)

**Files:** Modify `crates/higanbana-gpui/src/video/{player.rs,state.rs,mod.rs}`; Create `src/video/options.rs`; Test `src/video/tests_options.rs`

**Interfaces:**
- Produces:
  - `PlayerOptions { extra: Vec<(String, String)> }` com `PlayerOptions::default()` = `OPTIONS` atuais; `.set(k, v) -> Self` (sobrescreve chave igual). `Player::with_options(waker, PlayerOptions) -> Result<Player, String>`; `Player::new(waker)` = `with_options(waker, default)`.
  - `Player::load_at(&self, path: &str, start: Option<f64>)` (`loadfile <p> replace -1 start=<s>` só se `Some(s) && s > 1.0`, formatado `{:.1}`); `Player::set_property(&self, name: &str, value: &str)`; `Player::seek` passa a ser `absolute+exact`.
  - `PlayerState` ganha `chapters: Vec<Chapter>` (`Chapter { time: f64, title: String }`, via `chapter-list` node), `cache_ahead: f64` (`demuxer-cache-time`), `buffering: bool` (`paused-for-cache`), `first_frame: bool` (`vo-configured`), `end_error: u32` (contador de `END_FILE` com reason Error; nunca zera sozinho), `eof: bool`.
- [ ] **Step 1: testes** `opcoes_padrao_iguais_as_atuais`; `set_sobrescreve_chave`; `load_at_sem_start_ate_1s` (detached: `load_at("a", Some(0.5))` → comando sem `start=`; `Some(12.34)` → `start=12.3`); `capitulos_do_node` (aplica node fake → 2 capítulos na ordem); `fim_com_erro_incrementa`.
- [ ] **Step 2:** `cargo test -p higanbana-gpui --features video options` → FAIL.
- [ ] **Step 3:** implementar; `OBSERVED` ganha `chapter-list` (NODE), `demuxer-cache-time` (DOUBLE), `paused-for-cache` (FLAG), `vo-configured` (FLAG).
- [ ] **Step 4:** testes → PASS; suíte da lib verde (`--features video`).
- [ ] **Step 5:** commit `feat(gpui): Player com opções, start, capítulos e cache à frente`.

### Task 2: Peças do VideoPlayer — VideoSurface e AutoHide (lib)

**Files:** Create `crates/higanbana-gpui/src/components/video/{surface.rs,auto_hide.rs}`; Modify `components/video/{mod.rs,player_view.rs}` (passa a usar as peças, sem mudar visual); Test `components/video/tests_pieces.rs`

**Interfaces:**
- Produces:
  - `AutoHide { after: Duration, last: Instant, idle: bool }`; `AutoHide::new(Duration)`; `poke(&mut self, now) -> bool` (true se estava oculto); `tick(&mut self, now) -> Option<Duration>` (quanto falta; `idle = true` ao vencer); `hidden(&self, hold: bool) -> bool` (`idle && !hold`). O app passa `hold = pausado || menu || buffering || arraste`.
  - `VideoSurface::new(id, &mut VideoState-ou-&mut Player, fit)` com `.on_click(f)` (clique simples, adiado pela janela de duplo clique) e `.on_double_click(f)`; duplo clique NÃO dispara o simples. `.cursor_hidden(bool)` aplica `CursorStyle::None` no elemento.
- [ ] **Step 1: testes** `auto_hide_vence_em_2500ms`; `poke_devolve_true_quando_oculto`; `hold_segura_visivel`; `duplo_clique_nao_dispara_simples` (Review Focus 5); `clique_simples_dispara_apos_janela`; `cursor_none_quando_oculto`.
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** extrair do `VideoState` (`pending_click`, `last_activity`) para as peças; `VideoPlayer` continua passando nos testes antigos de `components/video/tests.rs`.
- [ ] **Step 4:** `cargo test -p higanbana-gpui --features video` → PASS.
- [ ] **Step 5:** commit `refactor(gpui): VideoPlayer montado com VideoSurface e AutoHide`.

### Task 3: Seekbar (lib)

**Files:** Create `crates/higanbana-gpui/src/components/video/seekbar.rs`; Test `components/video/tests_seekbar.rs`; Modify `examples/video_player.rs` (usa a Seekbar) e galeria (seção "Seekbar" com valores fixos)

**Interfaces:**
- Produces: `SeekbarState { drag: Option<f64>, hover: Option<f32> }` (frações 0..1 / x local). `Seekbar::new(id, &SeekbarState, lens)` com `.duration(f64)`, `.position(f64)`, `.buffered(f64)` (segundos absolutos), `.marks(Vec<f64>)`, `.played_color(Color)`, `.hover_content(impl Fn(f64 /*segundos*/, f32 /*x*/, f32 /*largura*/) -> Element<V>)`, `.on_seek(Fn(&mut V, f64, &mut EventCx))`, `.on_drag(Fn(&mut V, bool, &mut EventCx))`.
- Valores (V:1122-1200): área 20 px de altura, itens centralizados; trilha 4 px `rounded_full` branco 0.2; buffer branco 0.35 até `min(1,(buffered)/dur)`; tocado `played_color` até drag-ou-posição; marcas 2 px preto 0.8 altura cheia, só `t > 0`; knob 14 px branco, `shadow_md`, centro na fração (`ml -7`), só se `dur > 0`. Conteúdo de hover absoluto a `bottom 24`.
- [ ] **Step 1: testes** `fracao_do_clique` (clique em x=50 de 200 com dur 100 → `on_seek(25.0)`); `arraste_solto_fora_busca_ultimo_ponto` (Review Focus 4: down, move para x=-30, up fora → `on_seek(0.0)`, `drag == None`); `sem_duracao_sem_knob`; `marca_em_zero_ignorada`; `buffer_limitado_a_1`; `hover_some_ao_sair`.
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar com `on_mouse_down` + `on_mouse_move` (captura) + `on_mouse_up`, usando `PointerEvent::local`/`bounds`.
- [ ] **Step 4:** testes → PASS; `cargo run --example video_player --features video` mostra a barra.
- [ ] **Step 5:** commit `feat(gpui): Seekbar com buffer, capítulos, knob e hover`.

### Task 4: Player headless para miniaturas (lib)

**Files:** Modify `crates/higanbana-gpui/src/video/player.rs`; Test `src/video/tests_headless.rs` (`#[ignore]` quando a libmpv não carrega)

**Interfaces:**
- Produces: `Player::headless(PlayerOptions) -> Result<Player, String>` (sem thread de render, sem `vo=libmpv`; o app passa `vo=null`); `Player::command_blocking(&self, args: &[&str]) -> Result<(), String>` (`mpv_command`, bloqueia; chamar fora da UI). `Send` para uso em thread própria.
- [ ] **Step 1: teste** `headless_tira_screenshot` (gera vídeo de 2 s com `lavfi://color=red:s=64x36:d=2` via `loadfile`, `seek 1 absolute+keyframes`, `screenshot-to-file <tmp>.jpg video` → arquivo existe e > 0 bytes).
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat(gpui): Player headless com comando bloqueante`.

### Task 5: PlayerSession — lógica do app, sem UI

**Files:** Create `crates/higanbana-desktop/src/video/{mod.rs,target.rs,session.rs,options.rs,tests_session.rs}`; Modify `main.rs` (reativa `mod video`), apagar `video/{surface.rs,mpv.rs,view.rs}` antigos

**Interfaces:**
- Consumes: Task 1 (`PlayerState` novo, `PlayerOptions`); core `player::logic` (`classify_chapters`, `estimate_ending`, `is_watched`, `pick_tracks`, `next_episode`, `episode_label`, retry 2/4/8/16/30), `core.player()` (`data`, `stream_url`, `preview_url`, `save_progress`, `save_tracks`, `save_settings`, `aniskip`).
- Produces:
  - `PlayTarget { dir, file, files, mal_id, is_anime, anime_title }` (igual V:67); `CACHES = [50,100,150,300]`, `BACK_CACHES = [0,32,64,256]`.
  - `options::mpv(settings) -> PlayerOptions` com V:114-156 menos o que era da superfície (`wid`, `vo=gpu-next`, `force-window`): cache em disco + `demuxer-cache-dir=%TEMP%/higanbana-cache` (limpa ao abrir), `vd-lavc-threads`, `hwdec=auto-safe` → **`auto-copy`** (sw render precisa de cópia), `hwdec-extra-frames=2`, `keep-open`, `osd-level=0`, `cache=yes`, `demuxer-max-bytes`, `demuxer-max-back-bytes`, `cache-secs=60`, `demuxer-readahead-secs=60`, `audio-buffer=0.2`, `sub-auto=no`, override `HIGANBANA_MPV`.
  - `PlayerSession` (puro: recebe `PlayerState` + `now`, devolve `Vec<Action>`): `Action::{Load{start}, Seek(f64), SetTrack{prop, value}, SaveProgress(Progress), Next, Fatal, ResolveSkip}`. Métodos: `on_state(&mut self, &PlayerState, now) -> Vec<Action>`, `skip_intro()`, `cancel_next()`, `go_next()`, `retry()`, `countdown() -> Option<(f64 /*restante*/, f64 /*total*/)>`, `show_skip(pos) -> bool`, `show_next(pos, dur) -> bool`.
  - Regras (V:389-772): progresso em memória a cada `time-pos`, persistido no máx. a cada 5 s, forçado em pausa/troca/fechar; retomar da posição salva se não assistido; skip por capítulos, senão aniskip (anime + mal_id + número); botão skip quando `op.start ≤ pos < op.end−1`; próximo quando há próximo, `dur>0`, não cancelado e `pos ≥ ed.start` (ou `dur − countdown_sec`); contagem pausa com o vídeo; 0 → `Next`; EOF com auto_next → `Next` uma vez (`advanced_from`); erro de fim → re-tentar na mesma posição após 2/4/8/16/30 s, 5ª falha → `Fatal`.
- [ ] **Step 1: testes** (com `PlayerState` montado à mão): `salva_no_max_a_cada_5s`; `pausa_forca_salvar`; `retoma_se_nao_assistido`; `skip_por_capitulo_abertura`; `botao_skip_some_1s_antes_do_fim`; `contagem_pausa_com_video`; `contagem_zero_avanca`; `eof_avanca_uma_vez` (Review Focus 2); `cancelar_impede_eof`; `retentativas_2_4_8_16_30_depois_fatal` (Review Focus 1); `opcoes_sem_superficie` (sem `wid`/`gpu-next`).
- [ ] **Step 2:** `cargo test -p higanbana-desktop session` → FAIL.
- [ ] **Step 3:** implementar portando a lógica de V:389-772 (sem gpui).
- [ ] **Step 4:** PASS; `cargo build -p higanbana-desktop` sem warnings novos.
- [ ] **Step 5:** commit `feat(desktop): PlayerSession (progresso, pular abertura, próximo, re-tentativas)`.

### Task 6: Miniaturas

**Files:** Create `crates/higanbana-desktop/src/video/thumbs.rs` (porta do `thumbs.rs` da master sobre `Player::headless`); Test `video/tests_thumbs.rs`

**Interfaces:**
- Consumes: Task 4.
- Produces: `Thumbs::start(url, waker-ou-Post) -> Thumbs`; `want(&mut self, t: f64)`; `get(&self, t: f64) -> Option<ImageSource>` (JPEG decodificado por `image_cache` para RGBA); `Drop` encerra a thread sem bloquear a UI e apaga a pasta.
- Valores (thumbs.rs da master): opções `vo=null, pause, keep-open, audio=no, sid=no, hwdec=no, hr-seek=no, vd-lavc-threads=1, vd-lavc-skiploopfilter=all, vd-lavc-fast, demuxer-lavf-probesize=524288, demuxer-lavf-analyzeduration=0.5, vf=scale=w=320:h=-2, cache=no, demuxer-readahead-secs=0, demuxer-max-bytes=4MiB, screenshot-jpeg-quality=80, config=no, load-scripts=no, ytdl=no, input-default-bindings=no`; balde `floor(t/10)`; `seek <b*10> absolute+keyframes` + `screenshot-to-file`; um pedido por vez, último vence; ocioso pré-busca ±3 vizinhos, mais perto primeiro; pasta `%TEMP%/higanbana-thumbs/{pid}-{seq}`.
- [ ] **Step 1: testes** (fila pura, sem mpv): `balde_de_10s`; `ultimo_pedido_vence`; `prefetch_mais_perto_primeiro`; `drop_durante_geracao_nao_trava` (Review Focus 3: worker fake que dorme 500 ms; `drop` volta em < 50 ms).
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar (fila separada do worker para testar sem mpv).
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat(desktop): miniaturas da barra sobre Player headless`.

### Task 7: Tela do player — superfície, barra de cima e estados

**Files:** Create `crates/higanbana-desktop/src/video/view/{mod.rs,top_bar.rs,status.rs,style.rs}`; Test `video/view/tests.rs`

**Interfaces:**
- Consumes: Tasks 1–3, 5.
- Produces: `PlayerView` (campo `Option<PlayerView>` em `Higanbana`) com `PlayerView::open(core, target, post, waker) -> PlayerView`; `fn view(v: &mut Higanbana, cx) -> Element<Higanbana>`. `style.rs`: `on_media()`, `on_media_muted()`, `scrim_down(a)`, `scrim_up(a)`, `media_card(cx)`, `menu_heading(text, cx)`.
- Valores (V:1281-1460):
  - raiz relativa, tamanho cheio, coluna, texto branco;
  - fundo do tema em cima do vídeo com opacidade 1 até `first_frame && tocando && !broken`, depois some em BASE;
  - barra de cima (no fluxo): altura 48, gap 8, `pl 8`, `pr 16`, `scrim_down(0.75)`; Button ghost `arrow-left` dica "Voltar (Esc)"; título `text_sm` truncado `max_w 70%` semibold + " · {ep}" em `on_media_muted`; fade QUICK ao ocultar;
  - área do vídeo `flex_1 min_h 0 relative`; estados por cima: buffering (spinner grande `on_media_muted`, gap 12, e "Abrindo {ep}…" antes do 1º quadro); broken ("O motor de vídeo (mpv) não abriu." + erro xs); fatal (`circle-alert` danger, "Não foi possível tocar este arquivo" semibold, "Pode ser a conexão, a sessão expirada ou um arquivo indisponível na nuvem." xs muted `max_w 288` centralizado, Button primary small `refresh-cw` "Tentar de novo"); sem rede (linha gap 12, `wifi-off` + "Sem conexão com a nuvem. Tentando de novo…");
  - `media_card`: coluna gap 8, px 20, py 16, `radius_lg`, borda 1 `border`, fundo `popover`, `shadow_lg`. Posição conferir na captura da Task 0.
- [ ] **Step 1: testes** de render (Player detached + estado à mão): `fundo_cobre_ate_primeiro_quadro`; `fatal_mostra_tentar_de_novo` (clique chama `retry`); `barra_some_quando_oculto` (opacidade → 0 após QUICK); `titulo_com_episodio`.
- [ ] **Step 2:** rodar → FAIL.
- [ ] **Step 3:** implementar; ícones novos (`arrow-left`, `circle-alert`, `refresh-cw`, `wifi-off`) entram na lib/`APP_ICONS` se faltarem.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat(desktop): player — superfície, barra de cima e estados`.

### Task 8: Controles e seekbar com prévia

**Files:** Create `crates/higanbana-desktop/src/video/view/{controls.rs,seek.rs}`; Test em `video/view/tests.rs`

**Interfaces:**
- Consumes: Task 3 (`Seekbar`), Task 6 (`Thumbs`).
- Valores (V:1122-1250):
  - bloco: coluna gap 4, px 20, pt 40, pb 12, `scrim_up(0.85)`, fade QUICK;
  - seekbar com `played_color = primary`, marcas = capítulos; hover sem miniatura: rótulo a `left x−24`, px 8, py 2, `radius`, `popover`, xs medium; com miniatura: coluna `left clamp(x−88, 0, w−176)`, gap 4, p 4, `radius_lg`, `popover`, `shadow_md`, imagem 176×99 `Cover` `radius` (placeholder preto 0.6) + tempo com pb 2;
  - linha gap 4: play/pause ("Tocar (Espaço)"/"Pausar (Espaço)"), próximo `skip-forward` "Próximo episódio (N)" desabilitado sem próximo, mudo (`volume-x` se mudo ou 0, `volume-1` <50, `volume-2`; "Tirar do mudo (M)"/"Mudo (M)"), Slider 96 px 0..100 passo 1 (0 se mudo), tempo `ml 8` sm muted "{pos} / {dur}", rótulo central `flex_1 px 16` centralizado sm medium truncado, 3 menus (Task 9), tela cheia `maximize`/`minimize` "Tela cheia (F)"/"Sair da tela cheia (F)". Todos Button ghost com `tip`.
- [ ] **Step 1: testes** `icone_volume_por_nivel`; `slider_zero_quando_mudo`; `proximo_desabilitado_sem_proximo`; `previa_limitada_nas_bordas` (x=10 → left 0; x=w → left w−176); `tempo_formatado`.
- [ ] **Step 2:** rodar → FAIL. **Step 3:** implementar. **Step 4:** PASS.
- [ ] **Step 5:** commit `feat(desktop): player — controles e seekbar com prévia`.

### Task 9: Menus, cantos e atalhos

**Files:** Create `crates/higanbana-desktop/src/video/view/{menus/tracks.rs,menus/settings.rs,menus/episodes.rs,menus/mod.rs,corner.rs,keys.rs}`; Test em `video/view/tests.rs`

**Interfaces:**
- Consumes: lib `Popover` (âncora BottomRight), `RadioGroup`, `Switch`, `Progress(XSmall)`; Task 5.
- Valores (V:956-1121, 1251-1280, 1461-1482, 853-882):
  - tracks (`captions`, "Legenda e áudio", w 288): "Legenda" (rádios "Desligada" + faixas), divisor, "Áudio" só se >1 faixa; rótulo "{nome} ({LANG})"; escolha salva `TrackPref`;
  - settings (`settings-2`, "Ajustes", w 320, gap 12): textos verbatim do inventário — "Ir para o próximo episódio sozinho", "Tempo da contagem" 5 s/10 s/15 s (só se ligado), "Mostrar “Pular abertura”", divisor, "Legenda preferida"/"Áudio preferido" (Português/Inglês/Japonês/Coreano/Espanhol = por/eng/jpn/kor/spa, horizontais), divisor, "Miniatura ao passar o mouse na barra", "Processador (vale no próximo episódio)" Automático/4 núcleos/2 núcleos/1 núcleo (0/4/2/1), "Buffer à frente" 50/100/150/300 MB (aplica `demuxer-max-bytes` ao vivo);
  - episódios (`list-video`, "Episódios", w 384, max_h 384, rola): linha gap 12 px 8 py 8 `radius`; atual `list_active`, demais hover `list_hover`; número w 32 lg semibold centralizado (`primary_ink` se atual, senão muted; "·" sem número); "Episódio N" sm truncado; Progress XSmall se parcial; `check` small `success_foreground` se assistido;
  - cabeçalho de menu: pb 4, xs medium muted;
  - canto: absoluto `right 20`, `bottom 24` oculto / `96` visível, coluna `items_end` gap 12; "Pular abertura" (default, `chevrons-right`, dica "Pular abertura (S)") quando skip e sem cartão; cartão próximo (w 320, fade BASE, consome clique): "Próximo episódio em {ceil} s"/"Próximo episódio" xs muted, "Episódio {n}" semibold truncado, Progress XSmall `(1−rest/total)×100` na contagem, linha gap 8: primary `play` "Assistir agora" flex_1 + ghost "Cancelar";
  - atalhos (ignorar com Ctrl/Alt): espaço, ←/→ ∓5 s, ↑/↓ volume ±5 (tira mudo se >0), M, F, N, S, Esc (fecha menu → sai da tela cheia → fecha player); todo atalho tratado para propagação e dá `poke`; clique no vídeo fecha menu aberto, senão pausa/toca; duplo clique = tela cheia.
- [ ] **Step 1: testes** `esc_fecha_menu_depois_tela_cheia_depois_player`; `setas_buscam_5s`; `volume_sobe_tira_mudo`; `ctrl_ignora`; `audio_so_com_mais_de_uma_faixa`; `contagem_some_sem_auto_next`; `clique_fecha_menu_sem_pausar`; `episodio_atual_destacado`.
- [ ] **Step 2:** rodar → FAIL. **Step 3:** implementar. **Step 4:** PASS.
- [ ] **Step 5:** commit `feat(desktop): player — menus, pular abertura, próximo episódio e atalhos`.

### Task 10: Abrir e fechar o player no app

**Files:** Modify `crates/higanbana-desktop/src/{root.rs,state/mod.rs,main.rs}`; Test `video/tests_open.rs`

**Interfaces:**
- Produces: `AppEvent::Play(PlayTarget)` → `Higanbana::play(target, cx)`: pausa a música (se o Plano 4 ainda não portou, no-op com comentário), cria `PlayerView`; enquanto aberto, a raiz renderiza **só** o player (sem barra/sidebar/fundo); cursor oculto e auto-hide via `AutoHide` (2500 ms, `hold` = pausado || menu || buffering || arraste); ao ficar ocioso tocando, larga as miniaturas. Fechar: sai da tela cheia, salva progresso forçado, `player = None`, `refresh_player_data`. Dev: `HIGANBANA_PLAY=<dir>|<arquivo>` abre direto (para as capturas, sem precisar das telas do Plano 4).
- [ ] **Step 1: testes** `fechar_salva_progresso` (Review Focus 3: sessão com posição 42 → fechar → `SaveProgress` forçado); `raiz_so_player_quando_aberto`; `fechar_durante_carregamento_sem_panico` (Player detached em `Loading`).
- [ ] **Step 2:** rodar → FAIL. **Step 3:** implementar. **Step 4:** `cargo test -p higanbana-desktop` PASS; `cargo build --release -p higanbana-desktop` sem warnings.
- [ ] **Step 5:** commit `feat(desktop): abrir e fechar o player`.

### Task 11: Conferência visual e CPU

**Files:** nenhum (ajustes que surgirem viram commits `fix(desktop): …`).

- [ ] **Step 1:** `HIGANBANA_PLAY=<o mesmo arquivo da Task 0>` em release; capturar as mesmas situações da Task 0 em 1280×800.
- [ ] **Step 2:** comparar lado a lado; diferença visível (posição, tamanho, cor, ícone, motion) → corrigir e recapturar. Tolerância: antialiasing.
- [ ] **Step 3:** CPU: pausado 10 s → ~0%; tocando o episódio (anotar resolução) → medir média em 30 s e reportar.
- [ ] **Step 4:** `cargo test -p higanbana-gpui --features video` e `cargo test -p higanbana-desktop` verdes.
- [ ] **Step 5:** commit dos ajustes (se houver).

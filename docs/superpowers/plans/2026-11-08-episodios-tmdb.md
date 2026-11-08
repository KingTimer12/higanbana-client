# Episódios com TMDB — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lista de episódios de anime com miniatura, título, nota e duração vindos do TMDB, em grade (A, padrão) ou lista (B), mais uma trait de fonte de notas.

**Architecture:** Novo módulo `tmdb` no `higanbana-core` (cliente com ritmo, tipos, casamento puro obra/arquivo→episódio, serviço com cache em disco), exposto por `Core::tmdb()`. O desktop pede os episódios da obra ao abrir a página (sob demanda, como `want_lookup`) e desenha a aba Episódios num arquivo de view novo. Notas passam por uma trait `RatingSource` com implementações AniList e TMDB; preferências (fonte de notas, layout) num `settings.json` novo.

**Tech Stack:** Rust, tokio, reqwest 0.12 (workspace), crate `tmdb_client` 1.8 (opcional, ver Task 1), GPUI + gpui-kit.

**Spec:** `docs/superpowers/specs/2026-11-08-episodios-tmdb-design.md` (ler inteiro antes de qualquer tarefa).

## Global Constraints

- Branch `feat/catalogo-animes`; não criar branch.
- NUNCA rodar `cargo fmt` (arquivos CRLF; reformata o crate). Estilo à mão, manter CRLF.
- Um commit por tarefa (ou por mudança coerente), mensagem em português, SEM `Co-Authored-By` nem qualquer linha de atribuição.
- Antes de cada commit: `cargo test -p higanbana-core` e `cargo test -p higanbana-desktop` verdes; `cargo build -p higanbana-desktop` sem warnings novos.
- User-Agent `concat!("higanbana-client/", env!("CARGO_PKG_VERSION"))`; nunca e-mail.
- Credenciais `TMDB_TOKEN` (Bearer v4, preferido) e `TMDB_KEY` (v3) no `.env`, embutidas mascaradas com XOR como `ANITSU_SUPABASE_ANON_KEY` em `crates/higanbana-core/build.rs`; a falta delas NÃO quebra o build: recurso desligado.
- TMDB só é consultado ao abrir a página de uma obra; nada de varrer o catálogo.
- Na dúvida, não ligar arquivo a episódio (anti-objetivo: episódio errado).
- Sem TMDB (sem token, erro, obra não casada) a aba Episódios funciona igual, com miniatura neutra.
- Idioma: títulos/sinopses pt-BR; vazio ou genérico ("Episódio N"/"Episode N") → en-US; senão "Episódio N".
- Tolerância de data de exibição: 7 dias.
- UI segue as skills `gpui-kit` e `gpui-kit-design-guides`.

## Review Focus

1. Split cour / "Part 2": obra AniList começa no meio de uma temporada TMDB → arquivo "01" deve cair no episódio da data de início, não em S1E1 (teste na Task 2).
2. Numeração absoluta (One Piece: arquivo `0500`, TMDB em arcos) → converter para (temporada, episódio) certos; número além do total → sem ligação (Task 2).
3. Pasta de OVA/SPECIAL sem episódio com data compatível na temporada 0 → nenhum arquivo ligado (Task 2).
4. TMDB responde 429 / está fora / sem token → página abre, cards neutros, nada de laço de pedidos (Tasks 1, 3 e 5).
5. Pasta com subpastas, legendas e vídeos sem número junto de vídeos com número → todos os vídeos viram cards, o resto vai para a lista compacta embaixo, nada some (Task 6).

---

### Task 1: Credenciais e cliente TMDB (core)

**Files:**
- Modify: `crates/higanbana-core/build.rs` (embutir `TMDB_TOKEN`/`TMDB_KEY` opcionais)
- Modify: `crates/higanbana-core/Cargo.toml`, `Cargo.toml` (workspace) se usar `tmdb_client`
- Modify: `.env.example` (linhas `TMDB_TOKEN=` e `TMDB_KEY=` com comentário)
- Create: `crates/higanbana-core/src/tmdb/mod.rs`, `tmdb/client.rs`, `tmdb/types.rs`, `tmdb/credentials.rs`
- Modify: `crates/higanbana-core/src/lib.rs` (`pub mod tmdb;`)

**Interfaces:**
- Produces:
  - `tmdb::credentials::token() -> Option<String>` (Bearer; `None` quando não embutido) e `api_key() -> Option<String>`.
  - `tmdb::TmdbError` (thiserror): `NotConfigured`, `RateLimited`, `NotFound`, `Http(String)`, `Decode(String)`; `pub type TmdbResult<T> = Result<T, TmdbError>`.
  - Tipos (serde, `Clone`, `Debug`, `Serialize`, `Deserialize`) em `tmdb/types.rs`:
    - `TvHit { id: u64, name: String, original_name: String, first_air_date: Option<String> }`
    - `SeasonInfo { season_number: u32, episode_count: u32, air_date: Option<String> }`
    - `TvShow { id: u64, name: String, vote_average: f32, vote_count: u32, seasons: Vec<SeasonInfo> }`
    - `Episode { season_number: u32, episode_number: u32, name: String, overview: String, air_date: Option<String>, still_path: Option<String>, vote_average: f32, vote_count: u32, runtime: Option<u32> }`
  - `tmdb::client::TmdbClient` com `new() -> Self`, `configured(&self) -> bool`, `async fn search_tv(&self, query: &str, lang: &str) -> TmdbResult<Vec<TvHit>>`, `async fn tv(&self, id: u64, lang: &str) -> TmdbResult<TvShow>`, `async fn season(&self, id: u64, season: u32, lang: &str) -> TmdbResult<Vec<Episode>>`.
  - `tmdb::still_url(path: &str, size: &str) -> String` → `https://image.tmdb.org/t/p/{size}{path}`.

- [ ] **Step 1:** Em `build.rs`, nova função `embed_optional(var, masked_env, mask_env)` reaproveitando `read_dotenv`/`random_bytes`/`hex` (generalizar `read_dotenv` para receber o nome da variável). Quando ausente, emitir as duas `rustc-env` vazias (string vazia), sem panic. `cargo:rerun-if-env-changed` para as duas variáveis.
- [ ] **Step 2: Testes que falham** em `credentials.rs`: `unmask(masked_hex, mask_hex) -> Option<String>` pura: `unmask("", "") == None`; máscara de "abc" com máscara `[1,2,3]` volta "abc"; tamanhos diferentes → `None`.
- [ ] **Step 3:** `cargo test -p higanbana-core tmdb::credentials` → FAIL (função não existe). Implementar; `token()`/`api_key()` usam `option_env!`/`env!` das variáveis do build. Rodar → PASS.
- [ ] **Step 4: Testes que falham** em `client.rs` para funções puras de ritmo: `retry_after(header: Option<&str>) -> Duration` (número → segundos + 1 s; ausente/HTTP-date → 10 s); e desserialização de JSON real reduzido para `TvShow`, `Vec<Episode>` (campo `episodes`), `Vec<TvHit>` (campo `results`), aceitando `still_path: null`, `air_date: null`/`""`.
- [ ] **Step 5:** Implementar `TmdbClient`: uma requisição por vez (`tokio::sync::Mutex<()>` segurado na chamada), intervalo mínimo 100 ms, em 429 espera `retry_after` e tenta de novo até 3 vezes, depois `RateLimited`; timeouts connect 5 s / total 10 s; Bearer `token()`, senão `api_key` como query `api_key=`; sem nenhum → `NotConfigured` sem rede. Usar a crate `tmdb_client` se ela der essas garantias (async, timeouts, cabeçalho); se não, `reqwest` direto — registrar a escolha e o motivo no relatório.
- [ ] **Step 6:** Teste ao vivo `#[ignore]` `live_frieren`: `search_tv("Sousou no Frieren","pt-BR")` contém id 209867; `season(209867,1,"pt-BR")` tem 38 episódios com `still_path`.
- [ ] **Step 7:** `.env.example`: `TMDB_TOKEN=` e `TMDB_KEY=` com comentário "opcional: miniaturas e títulos dos episódios".
- [ ] **Step 8:** Testes + build (Global Constraints). Commit `feat(tmdb): credenciais opcionais e cliente com ritmo`.

### Task 2: Casamento puro obra/arquivo → episódio (core)

**Files:**
- Create: `crates/higanbana-core/src/tmdb/matching.rs`
- Modify: `crates/higanbana-core/src/player/logic.rs` (nova `episode_season`)

**Interfaces:**
- Consumes: tipos da Task 1.
- Produces:
  - `player::logic::episode_season(file_name: &str) -> Option<u32>` (temporada de `S02E03`; `None` sem padrão SxxEyy).
  - `matching::Ymd { y: i32, m: u32, d: u32 }` com `Ymd::parse(&str) -> Option<Ymd>` ("2023-09-29") e `days(&self) -> i64` (dias desde época; algoritmo civil-from-days, sem crate nova).
  - `matching::WorkKind { Series, Special }` (ONA/TV/TV_SHORT → Series; OVA/SPECIAL → Special; MOVIE → fora, o chamador não pede).
  - `matching::WorkQuery { titles: Vec<String>, year: Option<i32>, start: Option<Ymd>, kind: WorkKind }`
  - `matching::pick_show(hits: &[TvHit], q: &WorkQuery) -> Option<u64>`
  - `matching::Anchor { season: u32, offset: u32 }` (arquivo n → episódio `offset + n` da `season`), `Serialize/Deserialize/Clone/Copy/PartialEq/Debug`.
  - `matching::pick_anchor(show: &TvShow, seasons: &[(u32, Vec<Episode>)], q: &WorkQuery) -> Option<Anchor>`
  - `matching::map_file(anchor: Anchor, show: &TvShow, file_season: Option<u32>, number: u32) -> Option<(u32, u32)>`
  - `matching::is_generic_title(s: &str) -> bool` e `matching::pick_text(pt: &str, en: &str, n: u32) -> String`.

Regras (do spec, seção "Como ligar"):
- `pick_show`: candidato cujo `name`/`original_name` normalizado (minúsculas, só alfanuméricos) iguala ou contém algum de `q.titles` normalizado; entre esses, o de ano de estreia == `q.year`; senão o de estreia mais recente ≤ `q.year` (parte posterior → série-mãe); sem ano → primeiro que casa pelo título. Nenhum título casando → `None`.
- `pick_anchor` (Series): temporadas regulares (`season_number > 0`) em ordem. Se `q.start` existe: primeiro episódio cuja `air_date` está a ≤ 7 dias de `q.start` → `Anchor { season, offset: episode_number - 1 }`. Sem `q.start` e só uma temporada regular → `Anchor{season:1, offset:0}`. Senão `None`.
- `pick_anchor` (Special): só temporada 0; episódio a ≤ 7 dias de `q.start` → âncora nele; sem data → `None`.
- `map_file`: se `file_season` é `Some(s)` e `s` existe em `show.seasons` → `(s, number)` se `number <= episode_count` de `s`. Senão `ep = anchor.offset + number`; se `ep <= episode_count` da `anchor.season` → `(anchor.season, ep)`. Senão, se `anchor.season > 0` e `anchor.offset == 0`, tratar `number` como absoluto: somar `episode_count` das temporadas regulares a partir da 1 até achar a que contém `number` → `(temporada, número - soma_anteriores)`. Nada disso → `None`. Também: se `anchor.offset > 0` e `number > anchor.offset` e `number <= episode_count` da âncora → `(anchor.season, number)` (arquivos numerados pela temporada, ex. 13–24).
  - Ordem das regras para o caso offset: primeiro testar `number > offset && number <= count` (numeração da temporada), depois `offset + number`. Documentar no código.

- [ ] **Step 1: Testes que falham** (`matching.rs` `#[cfg(test)]`), com dados mínimos montados à mão:
  - `ymd_parses_and_counts`: `Ymd::parse("2023-09-29")` ok; `"2023-10-06".days() - "2023-09-29".days() == 7`; `Ymd::parse("")` e `"x"` → `None`.
  - `show_by_title_and_year`: hits Frieren(2023) e outro; `pick_show` com `titles=["Sousou no Frieren","Frieren"]`, ano 2023 → 209867.
  - `later_part_picks_parent`: série estreou 2022, obra 2023, mesmo título → id da série.
  - `no_title_no_show`: título sem relação → `None`.
  - `single_season_anchor`: 1 temporada, sem start → `Anchor{season:1,offset:0}`.
  - `split_cour_anchor`: S1 com 24 eps, ep 13 air_date 2022-10-02, `q.start` 2022-10-01 → `Anchor{season:1,offset:12}`; `map_file(.., None, 1) == Some((1,13))`; `map_file(.., None, 13) == Some((1,13))`; `map_file(.., None, 25) == None`.
  - `start_mismatch_no_anchor`: nenhuma data a ≤ 7 dias e várias temporadas → `None`.
  - `absolute_numbering`: temporadas 1(61), 2(16), 3(14); âncora {1,0}; `map_file(.., None, 70) == Some((2,9))`; `map_file(.., None, 92) == None`.
  - `explicit_season_wins`: `map_file(Anchor{1,0}, show, Some(2), 3) == Some((2,3))`; `Some(9)` inexistente → regra normal.
  - `special_anchor_season_zero`: temporada 0 com ep 5 em 2015-03-01, `kind: Special`, start 2015-03-03 → `Anchor{season:0,offset:4}`; sem episódio próximo → `None`.
  - `titles_fallback`: `pick_text("Episódio 3","Love wins",3) == "Love wins"`; `pick_text("","",3) == "Episódio 3"`; `pick_text("O fim da aventura","x",1) == "O fim da aventura"`; `is_generic_title("Episode 12")` true.
  - Em `player/logic.rs`: `episode_season("[G] X S02E03 [1080p].mkv") == Some(2)`; `episode_season("X - 03.mkv") == None`.
- [ ] **Step 2:** `cargo test -p higanbana-core matching` → FAIL (compilação). 
- [ ] **Step 3:** Implementar as funções acima.
- [ ] **Step 4:** `cargo test -p higanbana-core` → PASS.
- [ ] **Step 5:** Commit `feat(tmdb): casamento de obra e arquivos com episódios`.

### Task 3: Serviço TMDB com cache e `Core::tmdb()` (core)

**Files:**
- Create: `crates/higanbana-core/src/tmdb/service.rs`
- Modify: `crates/higanbana-core/src/core.rs` (campo `tmdb: Tmdb`, `pub fn tmdb(&self) -> &Tmdb`)

**Interfaces:**
- Consumes: Tasks 1 e 2; `anilist::cache::DiskCache` (`open/get/insert/flush`).
- Produces:
  - `tmdb::WorkEpisodes { show_id: u64, show_vote: (f32, u32), anchor: Anchor, show: TvShow, episodes: Vec<EpisodeView> }` (`Clone`, `Debug`, serde).
  - `tmdb::EpisodeView { season: u32, number: u32, title: String, overview: String, air_date: Option<String>, still_path: Option<String>, vote_average: f32, vote_count: u32, runtime: Option<u32> }` — título/sinopse já resolvidos por `pick_text` (pt-BR → en-US → "Episódio N").
  - `tmdb::Tmdb::new(cache_dir: &Path) -> Self`, `configured(&self) -> bool`, `async fn work_episodes(&self, q: &WorkQuery) -> TmdbResult<Option<WorkEpisodes>>` (`Ok(None)` = TMDB não tem a obra).
  - `impl WorkEpisodes { pub fn link(&self, file_name: &str) -> Option<&EpisodeView> }` (usa `episode_number` + `episode_season` + `map_file`).
  - `tmdb::work_key(q: &WorkQuery) -> String` (chave de cache e do mapa do desktop).

- [ ] **Step 1: Testes que falham:** `link_uses_anchor` (WorkEpisodes montado à mão com âncora {1,12} e episódios 13–24; `link("[G] X - 01 [1080p].mkv")` → episódio 13; `link("ncop.mkv")` → `None`); `cache_key_is_stable` (`work_key(&WorkQuery)` = títulos normalizados + ano + kind, mesma entrada → mesma chave).
- [ ] **Step 2:** Rodar → FAIL. Implementar `work_episodes`: chave → cache `tmdb-works.json` (7 dias; guarda `Option<(show_id, Anchor)>`, inclusive `None`); senão `search_tv` para cada título (pt-BR) até `pick_show`, `tv(id)`, temporadas necessárias via cache `tmdb-seasons.json` (24 h, chave `"{id}:{season}:{lang}"`) — Series: regulares (para âncora/absoluto); Special: só 0. Buscar pt-BR e en-US da temporada e montar `EpisodeView` com `pick_text`. Erro de rede não vai para o cache.
- [ ] **Step 3:** Ligar em `Core::new` (`Tmdb::new(&paths.cache)`).
- [ ] **Step 4:** Teste ao vivo `#[ignore]` `live_happiness_charge`: WorkQuery `titles=["HappinessCharge Precure!"]`, ano 2014, Series → `Some`, 49 episódios, título do ep 1 em inglês (sem pt-BR).
- [ ] **Step 5:** Testes + build. Commit `feat(tmdb): serviço de episódios com cache`.

### Task 4: Fonte de notas e preferências (core)

**Files:**
- Create: `crates/higanbana-core/src/ratings.rs`, `crates/higanbana-core/src/settings.rs`
- Modify: `crates/higanbana-core/src/lib.rs`, `core.rs` (`settings: Settings`, `pub fn settings(&self) -> &Settings`)

**Interfaces:**
- Produces:
  - `ratings::Score { value: f32 /* 0–10 */, votes: Option<u32> }` e `Score::label(&self) -> String` ("8,5", vírgula decimal, uma casa).
  - `pub trait RatingSource: Send + Sync { fn work(&self, anilist: Option<&MediaSummary>, tmdb: Option<&WorkEpisodes>) -> Option<Score>; fn episode(&self, ep: &EpisodeView) -> Option<Score>; }`
  - `ratings::AniListRatings` (work = `average_score/10`; episode = mesma regra TMDB, pois o AniList não tem) e `ratings::TmdbRatings` (work = `show_vote` quando `votes > 0`; episode = `vote_average` quando `vote_count > 0`).
  - `ratings::RatingProvider { AniList, Tmdb }` (serde) e `ratings::source(p: RatingProvider) -> &'static dyn RatingSource`.
  - `settings::EpisodeLayout { Grid, List }` (serde, padrão `Grid`); `settings::Settings` em `settings.json` (`paths.data`): `open(PathBuf)`, `rating_provider() -> RatingProvider` (padrão AniList), `set_rating_provider`, `episode_layout() -> EpisodeLayout`, `set_episode_layout`; salvar atômico como `prefs.rs`; campos com `#[serde(default)]`.

- [ ] **Step 1: Testes que falham:** `anilist_work_score` (average 85 → 8.5, label "8,5"); `tmdb_episode_needs_votes` (vote_count 0 → `None`; 7.94/68 → "7,9"); `settings_roundtrip` (arquivo temporário: grava `List`, reabre → `List`; arquivo ausente → `Grid` e `AniList`).
- [ ] **Step 2:** Rodar → FAIL; implementar; rodar → PASS.
- [ ] **Step 3:** Testes + build. Commit `feat(core): fonte de notas e preferências de episódios`.

### Task 5: Pedido dos episódios no desktop

**Files:**
- Modify: `crates/higanbana-desktop/src/state.rs` (mapa de episódios por obra, `want_episodes`)
- Modify: `crates/higanbana-desktop/src/views/title.rs` (montar `WorkQuery` e pedir ao abrir)

**Interfaces:**
- Consumes: `Core::tmdb()`, `WorkQuery`, `WorkKind`, `WorkEpisodes`, `Core::settings()`.
- Produces:
  - `AppState::episodes(&self, key: &str) -> Option<Option<Arc<WorkEpisodes>>>` (`None` = buscando/não pedido; `Some(None)` = sem TMDB).
  - `AppState::want_episodes(&mut self, key: String, q: WorkQuery, cx)`: pede uma vez por chave; falha → `Some(None)` nesta sessão (sem repetir em laço); sem token → `Some(None)` sem rede.
  - `AppState::episode_layout()` / `set_episode_layout(EpisodeLayout, cx)` (persistem via `Settings`) e `rating_source() -> &'static dyn RatingSource`.
  - `title::work_query(detail: &MediaSummary, folder_name: &str) -> Option<WorkQuery>` pura: títulos = romaji, english, native, synonyms, título da pasta (`library::parse_work_name`); ano = `year()`; start = `start_date` completo (ano/mês/dia; sem dia → dia 1; sem mês → `None`); kind por `format` (MOVIE → `None`).

- [ ] **Step 1: Testes que falham** de `work_query`: MOVIE → `None`; OVA → `Special`; TV com startDate 2023-09-29 → `start == Ymd{2023,9,29}` e títulos incluem o da pasta.
- [ ] **Step 2:** Rodar → FAIL; implementar `work_query` e o estado; em `TitleView::sync` (quando o detalhe chega e a obra é anime) chamar `want_episodes` com a chave `work_key` (Task 3) — usar o detalhe completo (`MediaDetail.summary`, que tem `startDate { year month day }`).
- [ ] **Step 3:** A nota da obra no cabeçalho (`title.rs` ~509, hoje `Nota {s}%`) passa a vir de `rating_source().work(..)`, exibida como "★ 8,5"; com AniList como padrão o valor é o mesmo de hoje em escala 0–10.
- [ ] **Step 4:** Testes + build. Commit `feat(desktop): pede os episódios do TMDB ao abrir a obra`.

### Task 6: Aba Episódios em grade/lista + README

**Files:**
- Create: `crates/higanbana-desktop/src/views/episodes.rs` (e `views/mod.rs`)
- Modify: `crates/higanbana-desktop/src/views/title.rs:653-669` (anime usa a nova view; mangá continua `file_list`)
- Modify: `crates/higanbana-desktop/src/components.rs` só se precisar expor helpers já existentes (`download_button`, `hover_fade`, `truncate`, `when_visible`)
- Modify: `README.md`

**Interfaces:**
- Consumes: Task 5 (`episodes`, `episode_layout`, `set_episode_layout`, `rating_source`), `WorkEpisodes::link`, `tmdb::still_url`, `AppState::image`, `player::logic::{video_files, episode_number}`, `components::{file_list, download_button, when_visible}`.
- Produces: `episodes::episode_list(dir: &str, files: &[FileEntry], eps: Option<&WorkEpisodes>, layout: EpisodeLayout, on_play: FileAction, cx: &mut App) -> Div` e `episodes::card_text(file: &FileEntry, ep: Option<&EpisodeView>) -> (String /*rótulo "E1"*/, String /*título*/)` pura.

Visual (referência aprovada: `.superpowers/brainstorm/107-1794158077/content/episodios-layout.html`, opções A e B):
- **Grid (A):** colunas responsivas (~4 em 72 rem); miniatura 16:9 (`still_url(.., "w300")`), duração no canto inferior direito, barra de progresso `primary` embaixo, check/"Visto" quando assistido, ▶ no hover; embaixo "E{n}", título (2 linhas), "★ 8,5" quando houver; botão de download pequeno no card.
- **List (B):** linha com miniatura ~170 px (`w300`), "E{n} · data" (formato de data do `anilist::text` se existir), título, nota, sinopse 2–3 linhas, download à direita.
- Sem dados do TMDB para o arquivo: miniatura neutra (gradiente/ícone `FilePlay` do tema), título `pick_text`-like: "Episódio N" ou nome do arquivo.
- Miniatura só carrega quando visível (`when_visible`, como as capas).
- Botão de ícone grade↔lista alinhado à direita, na linha de opções acima da lista; alterna e persiste. Aparece sempre que há vídeos.
- Subpastas e arquivos não-vídeo: `file_list` com só esses itens, abaixo.
- Enquanto `episodes()` é `None`: cards com esqueleto na miniatura (não bloquear a lista).
- Clique no card = `on_play`.

- [ ] **Step 1: Testes que falham** de `card_text`: com episódio → ("E1", "O fim da aventura"); sem episódio e com número → ("E5", "Episódio 5"); sem número → ("", nome do arquivo).
- [ ] **Step 2:** Rodar → FAIL; implementar `card_text`, depois a view e a troca em `title.rs`.
- [ ] **Step 3:** README: seção curta "Episódios (TMDB)" — o que faz, como pôr `TMDB_TOKEN`/`TMDB_KEY` no `.env` (copiar de `.env.example`), que sem token o app funciona sem miniaturas, e o botão de layout.
- [ ] **Step 4:** Testes + build. Commit `feat(desktop): episódios em grade ou lista com miniaturas do TMDB` (README pode ir em commit próprio `docs: README com a configuração do TMDB`).

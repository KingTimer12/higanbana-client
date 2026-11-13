# Capas do fanart.tv — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Capa de anime vem do pôster da temporada no fanart.tv (reduzido, WebP em disco), com reserva no AniList, no desktop e no mobile; cache de imagens do desktop ganha limite LRU.

**Architecture:** Novo módulo `higanbana_core::fanart`: mapa compacto AniList/MAL → alvo (TVDB+temporada ou filme TMDB) construído da lista Fribb; cliente do fanart; escolha do pôster; redução + WebP num diretório de capas. `Core::cover`/`Core::cover_cached` devolvem o caminho local ou `None` (= usar AniList). Desktop e mobile pedem a capa quando o card aparece e trocam ao chegar.

**Tech Stack:** Rust 2024, tokio, reqwest, serde, `image` 0.25, `webp` (libwebp via `cc`), gpui-kit (desktop), NSH + TSX (mobile).

**Spec:** `docs/superpowers/specs/2026-11-12-capas-fanart-design.md`

## Global Constraints

- Chave: variável `FANART_TV` (env ou `.env` na raiz), embutida mascarada por `build.rs`; vazia → recurso desligado, **zero rede** (nem a lista Fribb).
- Base do fanart: `https://webservice.fanart.tv`; rotas `/v3/tv/{tvdb_id}?api_key=…` e `/v3/movies/{tmdb_id}?api_key=…`.
- Lista: `https://raw.githubusercontent.com/Fribb/anime-lists/master/anime-list-full.json`, renovada a cada 7 dias; falha mantém a anterior.
- Cache de URL escolhida: achado 7 dias, "não tem" 1 dia; erro de rede nunca vai para cache.
- Pôster: largura 480 px, WebP com perda qualidade 85, em `paths.cache/covers/`, teto 300 MB (apaga mais antigos por mtime na abertura).
- No máximo 4 requisições simultâneas ao fanart.
- LRU do desktop: orçamento 384 MB de bytes decodificados estimados (largura × altura × 4).
- Banner e capa de mangá: sem mudança (AniList).
- Nada toca a nuvem da Anitsu (conta já bloqueada por 429 antes).
- Commits sem trailer de coautoria; branch `feat/capas-fanart`.

## Review Focus

1. Revisitar uma tela já vista: a capa do fanart aparece direto, sem piscar a do AniList (`cover_cached`) — teste na Task 6.
2. Fanart/GitHub fora do ar ou sem rede: capa do AniList, sem cache de "não tem" e sem martelar a cada frame — testes nas Tasks 3 e 6, e `covers_failed` na Task 8.
3. Arquivo `.webp` truncado/corrompido no disco (app fechado no meio da gravação): grava em `.tmp` + rename; arquivo inválido é apagado e volta ao AniList — teste na Task 5.
4. Mídia de mangá ou chave ausente nunca gera requisição — teste na Task 6.
5. Lista Fribb com entradas estranhas (sem tvdb, temporada 0, temporada ausente, ids repetidos) não derruba a carga — teste na Task 2.

---

### Task 1: Chave `FANART_TV` embutida

**Files:**
- Create: `crates/higanbana-core/src/secret.rs` (move `unmask`/`unhex` + testes de `tmdb/credentials.rs`)
- Modify: `crates/higanbana-core/src/tmdb/credentials.rs` (usa `crate::secret::unmask`)
- Modify: `crates/higanbana-core/build.rs` (`embed_optional("FANART_TV", "FANART_TV_MASKED", "FANART_TV_MASK")`)
- Create: `crates/higanbana-core/src/fanart/mod.rs`, `crates/higanbana-core/src/fanart/credentials.rs`
- Modify: `crates/higanbana-core/src/lib.rs` (`mod secret; pub mod fanart;`)
- Modify: `README.md` (seção da chave `FANART_TV` ao lado da do TMDB)

**Interfaces:**
- Produces: `pub(crate) fn secret::unmask(masked_hex: &str, mask_hex: &str) -> Option<String>`; `pub fn fanart::credentials::api_key() -> Option<String>`; em `fanart/mod.rs`:
  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum FanartError { NotConfigured, NotFound, Http(String), Decode(String), Image(String) }
  pub type FanartResult<T> = Result<T, FanartError>;
  ```
  (mensagens em português no `#[error]`, como `TmdbError`).

- [ ] **Step 1:** Mover `unmask`/`unhex` e os 4 testes para `secret.rs`; `tmdb/credentials.rs` passa a chamar `crate::secret::unmask`.
- [ ] **Step 2:** Adicionar `embed_optional` do `FANART_TV` no `build.rs` e `fanart::credentials::api_key()` no mesmo molde do TMDB.
- [ ] **Step 3:** Run: `cargo test -p higanbana-core secret` → Expected: 4 testes PASS. `cargo check -p higanbana-core` → sem erro, com e sem `FANART_TV` no `.env`.
- [ ] **Step 4:** README: parágrafo "Capas (fanart.tv, opcional)" explicando `FANART_TV` e que sem ela as capas vêm do AniList.
- [ ] **Step 5: Commit** — `feat(fanart): chave FANART_TV embutida no build`

---

### Task 2: Mapa AniList/MAL → alvo (lista Fribb)

**Files:**
- Create: `crates/higanbana-core/src/fanart/mapping.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  pub enum Target {
      /// `series_poster`: aceita `tvposter` quando não há `seasonposter` (só temporada 1).
      Season { tvdb: u32, season: u32, series_poster: bool },
      Movie { tmdb: u32 },
  }
  #[derive(Default, Serialize, Deserialize)]
  pub struct AnimeMap { by_anilist: HashMap<i64, Target>, by_mal: HashMap<i64, Target> }
  impl AnimeMap {
      pub fn from_fribb(json: &[u8]) -> FanartResult<Self>;
      pub fn target(&self, anilist_id: i64, mal_id: Option<i64>) -> Option<Target>;
  }
  ```
  `target`: `anilist_id > 0` → `by_anilist`; senão (ou não achou) → `by_mal` com `mal_id` (ou `-anilist_id` quando negativo).

- [ ] **Step 1: Conferir o formato** — baixar **uma vez** a lista (`curl -s <URL> -o <scratchpad>/fribb.json`) e anotar nomes/tipos reais: `anilist_id`, `mal_id`, `thetvdb_id`, `themoviedb_id` (número ou string?), `type`, e a temporada (`season.tvdb`? valor para numeração absoluta?). Modelar o `RawEntry` (só esses campos, todos `Option`, `#[serde(default)]`) pelo que achar; ids que vierem como string aceitam ambos via `deserialize_with`.
- [ ] **Step 2: Testes que falham** em `mapping.rs`, com JSON montado no teste:
  ```rust
  #[test] fn movie_maps_to_tmdb()            // type "MOVIE", themoviedb_id 123 → Movie{tmdb:123}
  #[test] fn season_one_accepts_series_poster() // season 1 → Season{season:1, series_poster:true}
  #[test] fn later_season_no_series_poster() // season 2 → series_poster:false
  #[test] fn season_zero_is_skipped()        // season 0 → target None
  #[test] fn split_season_only_first_part()  // ids 100 e 200 em (tvdb 5, season 2) → 100 Some, 200 None
  #[test] fn absolute_single_entry_is_season_one() // tvdb com uma só entrada e temporada ausente/absoluta → Season{season:1, series_poster:true}
  #[test] fn absolute_with_siblings_is_skipped()   // temporada ausente e outra entrada no mesmo tvdb → None
  #[test] fn mal_only_by_negative_id()       // target(-77, None) acha por mal_id 77
  #[test] fn junk_entries_ignored()          // sem tvdb nem tmdb, ids nulos, entradas duplicadas → sem pânico, from_fribb Ok
  ```
- [ ] **Step 3:** Run: `cargo test -p higanbana-core fanart::mapping` → Expected: FAIL (não compila).
- [ ] **Step 4: Implementar `from_fribb`**. Regras, nesta ordem, por entrada:
  1. `type == "MOVIE"` e tem `themoviedb_id` → `Movie`.
  2. Sem `thetvdb_id` → ignora.
  3. Temporada 0 → ignora.
  4. Temporada ausente/absoluta → `Season{season:1, series_poster:true}` só se for a única entrada com aquele tvdb; senão ignora.
  5. Grupo `(tvdb, season)` com várias entradas → só o menor `anilist_id` do grupo recebe o alvo (as outras ignoradas, inclusive no `by_mal`).
  6. `series_poster = season == 1`.
- [ ] **Step 5:** Run: `cargo test -p higanbana-core fanart::mapping` → Expected: PASS.
- [ ] **Step 6: Commit** — `feat(fanart): mapa AniList/MAL para TVDB e TMDB pela lista Fribb`

---

### Task 3: Escolha do pôster e cliente do fanart

**Files:**
- Create: `crates/higanbana-core/src/fanart/pick.rs`, `crates/higanbana-core/src/fanart/client.rs`

**Interfaces:**
- Consumes: `Target` (Task 2), `credentials::api_key()`, `FanartError` (Task 1).
- Produces:
  ```rust
  // pick.rs
  #[derive(Deserialize, Default)] pub struct Art { pub url: String, #[serde(default)] pub lang: String, #[serde(default)] pub likes: String /* número em string na API */, #[serde(default)] pub season: Option<String> }
  #[derive(Deserialize, Default)] pub struct TvImages { #[serde(default)] pub tvposter: Vec<Art>, #[serde(default)] pub seasonposter: Vec<Art> }
  #[derive(Deserialize, Default)] pub struct MovieImages { #[serde(default)] pub movieposter: Vec<Art> }
  pub fn pick_tv(images: &TvImages, season: u32, series_poster: bool) -> Option<String>;
  pub fn pick_movie(images: &MovieImages) -> Option<String>;
  // client.rs
  pub struct FanartClient;
  impl FanartClient {
      pub fn new() -> Self;
      pub fn configured(&self) -> bool;
      pub async fn tv(&self, tvdb: u32) -> FanartResult<TvImages>;     // 404 → Ok(TvImages::default())
      pub async fn movie(&self, tmdb: u32) -> FanartResult<MovieImages>;
      pub async fn download(&self, url: &str) -> FanartResult<Vec<u8>>; // só https
  }
  ```
  O builder confirma os tipos de `likes`/`season` numa resposta real (uma chamada manual com a chave do `.env`).

- [ ] **Step 1: Testes que falham** em `pick.rs`:
  ```rust
  #[test] fn picks_matching_season()          // seasonposter season "2" e "1"; pick_tv(_,2,false) → url da "2"
  #[test] fn no_season_falls_to_none_after_s1() // só tvposter; pick_tv(_,2,false) → None
  #[test] fn season_one_uses_tvposter()       // só tvposter; pick_tv(_,1,true) → Some
  #[test] fn lang_order_en_then_textless_then_ja() // ja(likes 9), "00"(likes 1), en(likes 0) → en; sem en → "00"
  #[test] fn likes_break_ties()               // dois en, likes "3" e "7" → o de 7
  #[test] fn movie_picks_best_poster()
  ```
  Ordem de idioma: `en` > `00` ou vazio > `ja` > outros; empate por `likes` (parse `u32`, inválido = 0).
- [ ] **Step 2:** Run: `cargo test -p higanbana-core fanart::pick` → FAIL; implementar; → PASS.
- [ ] **Step 3: Cliente** — `reqwest` como `TmdbClient` (user-agent `higanbana-client/<versão>`, connect 5 s, total 10 s para JSON e 20 s para imagem), `tokio::sync::Semaphore` de 4 permissões segurado em cada requisição. Sem chave: `NotConfigured` sem rede. 404 em `tv`/`movie` → padrão vazio; outro status ≠ 2xx → `Http`.
- [ ] **Step 4: Teste** — `#[tokio::test] async fn unconfigured_never_hits_network()`: cliente construído com `api_key: None` (construtor interno `with_key(Option<String>)`) → `tv(1)` dá `NotConfigured`.
- [ ] **Step 5:** Run: `cargo test -p higanbana-core fanart` → PASS.
- [ ] **Step 6: Commit** — `feat(fanart): cliente e escolha do pôster por temporada e idioma`

---

### Task 4: Redução e WebP

**Files:**
- Modify: `Cargo.toml` (workspace: `webp = "0.3"`), `crates/higanbana-core/Cargo.toml` (`image = { workspace = true, default-features = false, features = ["jpeg", "png", "webp"] }`, `webp = { workspace = true, optional = true }`, feature `webp-lossy = ["dep:webp"]` em `default`)
- Modify: `mobile/Cargo.toml` (ativa `webp-lossy` explicitamente no `higanbana-core` — ver Step 5)
- Create: `crates/higanbana-core/src/fanart/encode.rs`

**Interfaces:**
- Produces:
  ```rust
  pub const COVER_WIDTH: u32 = 480;
  pub const COVER_QUALITY: f32 = 85.0;
  /// Reduz para `COVER_WIDTH` (sem ampliar) e codifica. Extensão do arquivo resultante.
  pub fn shrink(bytes: &[u8]) -> FanartResult<(Vec<u8>, &'static str)>; // ("webp") ou ("jpg") sem a feature
  ```

- [ ] **Step 1: Testes que falham**:
  ```rust
  #[test] fn shrinks_to_480_wide()   // PNG 1000×1500 gerado com image::RgbImage → saída decodifica com 480×720
  #[test] fn never_upscales()        // 300×450 → saída 300×450
  #[test] fn output_is_webp()        // bytes começam com "RIFF" e [8..12] == "WEBP" (com a feature)
  #[test] fn garbage_is_error()      // shrink(b"nada") → Err(FanartError::Image(_))
  ```
- [ ] **Step 2:** Run: `cargo test -p higanbana-core fanart::encode` → FAIL; implementar com `image::load_from_memory` + `resize` (`FilterType::Lanczos3`) + `webp::Encoder::from_image(...).encode(COVER_QUALITY)`; sem `webp-lossy`, `JpegEncoder::new_with_quality(_, 85)`; → PASS.
- [ ] **Step 3:** Run: `cargo build -p higanbana-desktop` → compila (libwebp via `cc` no Windows).
- [ ] **Step 4:** Run: `cargo test -p higanbana-core --no-default-features fanart::encode` → PASS no caminho JPEG (`output_is_webp` com `#[cfg(feature = "webp-lossy")]`).
- [ ] **Step 5: Android** — `mobile/Cargo.toml`: `higanbana-core = { …, default-features = false, features = ["webp-lossy"] }`; rodar `scripts/mobile.sh verify` (e o build Android que o README do mobile indica). Se a libwebp não compilar no NDK, tirar `webp-lossy` do mobile e anotar no commit; o mobile grava JPEG.
- [ ] **Step 6: Commit** — `feat(fanart): reduz pôster para 480px e grava WebP`

---

### Task 5: Pasta de capas no disco

**Files:**
- Create: `crates/higanbana-core/src/fanart/store.rs`

**Interfaces:**
- Produces:
  ```rust
  pub const COVERS_MAX_BYTES: u64 = 300 * 1024 * 1024;
  pub struct CoverStore { dir: PathBuf }
  impl CoverStore {
      pub fn open(dir: PathBuf) -> Self;                 // cria a pasta e chama prune(COVERS_MAX_BYTES)
      pub fn path_for(&self, url: &str) -> Option<PathBuf>; // arquivo existente para a URL (sha256 → 16 hex + .webp|.jpg)
      pub fn save(&self, url: &str, bytes: &[u8], ext: &str) -> std::io::Result<PathBuf>; // .tmp + rename
      pub fn discard(&self, path: &Path);               // apaga arquivo inválido
      fn prune(&self, max: u64);                         // apaga por mtime mais antiga até caber
  }
  ```

- [ ] **Step 1: Testes que falham** (pasta única em `std::env::temp_dir()` com nome aleatório por teste, apagada no fim):
  ```rust
  #[test] fn save_then_path_for()      // save("https://x/a.jpg", b"..", "webp") → path_for mesma URL == Some(path)
  #[test] fn different_urls_different_files()
  #[test] fn tmp_leftover_not_returned() // só existe "<hash>.webp.tmp" → path_for None
  #[test] fn prune_drops_oldest()      // 3 arquivos de 10 bytes com mtimes crescentes, prune(20) → sobra o mais novo e o do meio
  ```
  mtime via `std::fs::File::set_modified` (estável no Rust 2024).
- [ ] **Step 2:** Run: `cargo test -p higanbana-core fanart::store` → FAIL; implementar; → PASS.
- [ ] **Step 3: Commit** — `feat(fanart): pasta de capas com teto de 300 MB`

---

### Task 6: Serviço `Fanart` e `Core::cover`

**Files:**
- Create: `crates/higanbana-core/src/fanart/service.rs`
- Modify: `crates/higanbana-core/src/fanart/mod.rs` (exports), `crates/higanbana-core/src/core.rs` (campo `fanart: Fanart`, `Fanart::new(&paths.cache)`)

**Interfaces:**
- Consumes: Tasks 2–5.
- Produces:
  ```rust
  pub struct Fanart { /* client, map: tokio::sync::RwLock<Option<Arc<AnimeMap>>>, hits: DiskCache<String>, misses: DiskCache<bool>, store: CoverStore, … */ }
  impl Fanart {
      pub fn new(cache_dir: &Path) -> Self; // map em cache_dir/"fanart-map.json", hits "fanart-hits.json" (7d), misses "fanart-miss.json" (1d), store cache_dir/"covers"
      /// Caminho local do pôster, sem rede. `None` = ainda não tem (ou usar AniList).
      pub fn cover_cached(&self, m: &MediaSummary) -> Option<PathBuf>;
      /// Resolve mapa → fanart → arquivo. `Ok(None)` = usar AniList.
      pub async fn cover(&self, m: &MediaSummary) -> FanartResult<Option<PathBuf>>;
      /// Apaga um arquivo de capa inválido (repassa a `CoverStore::discard`).
      pub fn discard(&self, path: &Path);
  }
  // Core
  pub fn fanart(&self) -> &Fanart;
  ```
  Chave das caches por alvo: `tv:{tvdb}:{season}:{series_poster}` / `movie:{tmdb}`.

- [ ] **Step 1: Mapa** — carregar `fanart-map.json` (`{ at: i64, map: AnimeMap }`); vazio ou com mais de 7 dias → baixar a lista Fribb, `from_fribb`, gravar (`.tmp` + rename). Falha no download com arquivo antigo → usa o antigo; sem arquivo → erro, e nova tentativa só depois de 10 min (guardar `Instant` da falha). Sem chave → nunca baixa.
- [ ] **Step 2: Testes que falham** (construtor interno `with_parts(client, map, cache_dir)` para injetar mapa e cliente sem chave):
  ```rust
  #[tokio::test] async fn manga_returns_none_without_network()     // media_type Manga → Ok(None)
  #[tokio::test] async fn unconfigured_returns_none()              // sem chave → Ok(None), mapa nunca baixado
  #[tokio::test] async fn unmapped_returns_none()                  // id fora do mapa → Ok(None)
  #[test]        fn cached_hit_with_file_is_returned_sync()        // hits["tv:5:1:true"]=url e arquivo salvo no store → cover_cached Some(path)
  #[test]        fn cached_hit_without_file_is_none()
  ```
- [ ] **Step 3: `cover`** — mangá → `Ok(None)`; sem chave → `Ok(None)`; alvo do mapa; `misses` → `Ok(None)`; `hits` + arquivo → caminho; `hits` sem arquivo → baixar de novo; sem hit → `client.tv/movie` + `pick_*`; nada escolhido → `misses.insert` e `Ok(None)`; senão `hits.insert`, `download`, `shrink`, `store.save`. Se `shrink` falhar → `misses.insert` e `Ok(None)`. Erro de rede → `Err` sem tocar nas caches. `flush` das caches no fim, como o `Tmdb`.
- [ ] **Step 4:** Run: `cargo test -p higanbana-core fanart` → PASS. `cargo check -p higanbana-desktop` → OK.
- [ ] **Step 5: Commit** — `feat(fanart): serviço de capas no núcleo`

---

### Task 7: LRU do cache de imagens do desktop

**Files:**
- Create: `crates/higanbana-desktop/src/image_cache.rs`
- Modify: `crates/higanbana-desktop/src/state.rs:109,161,316-360` (`images: ImageCache<Img>`), `crates/higanbana-desktop/src/main.rs` (ou onde estão os `mod`)

**Interfaces:**
- Produces:
  ```rust
  pub const IMAGE_BUDGET: u64 = 384 * 1024 * 1024;
  pub struct ImageCache<V: Clone> { /* HashMap<String, (V, u64 weight, Cell<u64> last)>, Cell<u64> clock, total */ }
  impl<V: Clone> ImageCache<V> {
      pub fn new(budget: u64) -> Self;
      pub fn get(&self, key: &str) -> Option<V>;        // marca uso (Cell), por isso &self
      pub fn insert(&mut self, key: String, value: V, weight: u64) -> Vec<V>; // devolve os descartados
  }
  /// largura × altura × 4 lidos do cabeçalho; sem cabeçalho legível, bytes.len() × 4.
  pub fn decoded_weight(bytes: &[u8]) -> u64;
  ```
  `Img::Loading`/`Img::Failed` entram com peso 0. Descarte por menor `last`, nunca o recém-inserido.

- [ ] **Step 1: Testes que falham**:
  ```rust
  #[test] fn evicts_least_recently_used()   // budget 100; a(40), b(40); get("a"); c(40) → b sai, a e c ficam
  #[test] fn zero_weight_never_evicts()
  #[test] fn new_entry_survives_even_if_over_budget() // budget 10, insert x(50) → x fica
  #[test] fn weight_from_header()           // PNG 10×20 gerado → 800
  ```
- [ ] **Step 2:** Run: `cargo test -p higanbana-desktop image_cache` → FAIL; implementar; → PASS.
- [ ] **Step 3: Ligar no `AppState`** — `image()`, `put_image()`, `cached_image()`, `mark_image_loading()`, `fail_image()` usam o `ImageCache`; peso por `decoded_weight` nos `Ready`.
- [ ] **Step 4: Liberar textura** — procurar no gpui do registro (`~/.cargo/registry/src/*/gpui*`) como um `Arc<Image>` é cacheado (`use_asset`, `remove_asset`, `drop_image`). Se o gpui guarda o decodificado por conta própria, chamar a remoção correspondente para cada valor devolvido por `insert`. Anotar no commit o que foi achado.
- [ ] **Step 5:** Run: `cargo test -p higanbana-desktop` → PASS; `cargo run -p higanbana-desktop`, rolar o catálogo e ver no Gerenciador de Tarefas a memória estabilizar.
- [ ] **Step 6: Commit** — `feat(desktop): limite LRU de 384 MB no cache de imagens`

---

### Task 8: Capas do fanart no desktop

**Files:**
- Modify: `crates/higanbana-desktop/src/state.rs` (campos `covers`, `covers_failed`; métodos abaixo; `image()` aceita chave `file:`)
- Modify: `crates/higanbana-desktop/src/components.rs:110-128` (`cover`)
- Modify: `crates/higanbana-desktop/src/chrome.rs:152-154`, `crates/higanbana-desktop/src/views/login.rs:36,88`

**Interfaces:**
- Consumes: `Core::fanart()`, `Fanart::cover_cached`, `Fanart::cover` (Task 6).
- Produces:
  ```rust
  impl AppState {
      /// Chave de imagem da capa: fanart local (`file:<caminho>`) se já existe, senão a URL do AniList.
      /// Pede o fanart uma vez por id de mídia na sessão.
      pub fn cover_key(&mut self, m: &MediaSummary, small: bool, cx: &mut Context<Self>) -> Option<String>;
  }
  ```
  `covers: HashMap<i64, Option<Option<PathBuf>>>` (`None` = buscando); `covers_failed: HashSet<i64>` (erro de rede: não pede de novo nesta tela até a próxima navegação — limpar em `AppEvent::Navigated`).

- [ ] **Step 1:** `image()` — chave começando com `file:` lê o arquivo com `tokio::fs::read` no runtime do núcleo; arquivo que não decodifica → `Img::Failed`, `core.fanart().discard(caminho)` e a entrada do id em `covers` vira `Some(None)` (volta ao AniList).
- [ ] **Step 2:** `cover_key` — 1) `covers[id] == Some(Some(p))` → `file:p`; 2) não visto: `cover_cached` síncrono; achou → grava e devolve; senão marca `None`, `core.spawn(fanart.cover(m))`, ao voltar grava `Some(res)` e `cx.notify()`; erro → `covers_failed`. 3) senão → `m.cover()` ou `m.cover_small()` conforme `small`.
- [ ] **Step 3:** `components::cover` usa `cover_key(m, false)`; `chrome.rs` usa `cover_key(m, true)`; `login.rs` mostra `cover_key(m, true)` (o filtro de `cover_small().is_some()` continua).
- [ ] **Step 4: Verificação manual** — `cargo run -p higanbana-desktop` com `FANART_TV` no `.env`: estante e catálogo trocam para pôsteres nítidos; uma 2ª temporada sem seasonposter mantém a do AniList; fechar e abrir de novo → pôsteres aparecem sem piscar; sem `FANART_TV` → tudo como antes.
- [ ] **Step 5: Commit** — `feat(desktop): capas do fanart.tv com reserva no AniList`

---

### Task 9: Capas do fanart no mobile

**Files:**
- Modify: `mobile/src/lib.rs` (método `Catalog::cover`)
- Modify: `mobile/lib/backend.ts`, `mobile/lib/native.ts`, `mobile/fixture/fake.ts`, `mobile/store.ts`
- Modify: `mobile/components/Poster.tsx:66`, `mobile/pages/Title.tsx:134`
- Test: `mobile/App.test.tsx`

**Interfaces:**
- Consumes: `Fanart::cover_cached`, `Fanart::cover` (Task 6).
- Produces:
  ```rust
  /// URI do pôster local do fanart (`file://…`), ou `null` (usar o AniList).
  pub async fn cover(media: JsonValue) -> Result<Option<String>, String>;
  ```
  ```ts
  // backend.ts (interface do backend)
  cover(media: Media): Promise<string | null>;
  // store.ts
  export const coverOf = (media: Media | null | undefined): string | null; // fanart se já chegou, senão mediaCover(media)
  export function wantCover(media: Media): void;                           // uma vez por id, fila como wantLookup
  ```

- [ ] **Step 1: Arquivo local no `Image` do NSH** — procurar no `nsh-base` do registro como `Image src` resolve `file://`/caminho absoluto. Se aceitar, devolver `file://<caminho>`. Se não, parar e expor o arquivo pelo servidor HTTP local do núcleo (`player/proxy.rs`) numa rota `/cover/<nome>` restrita à pasta `covers/`, devolvendo `http://127.0.0.1:<porta>/cover/<nome>`; anotar a escolha no commit.
- [ ] **Step 2: Teste que falha** em `App.test.tsx`: na fixture, mídia id 1 tem capa do fanart `"file:///fanart/1.webp"`; o pôster dela na tela da temporada renderiza `Image` com esse `src`; mídia sem capa do fanart renderiza a URL `coverImage.large`.
- [ ] **Step 3:** Run: `cd mobile; nsh test` → FAIL.
- [ ] **Step 4:** Implementar `Catalog::cover` (`cover_cached` primeiro, senão `cover().await`, erro → `Ok(None)`), `native.ts`, fixture (`cover` devolve o arquivo falso para id 1, `null` para os outros), `store.ts` (`coverOf`/`wantCover`, `effect(() => wantCover(media))` no `Poster` quando há `media`), `Poster.tsx` e `Title.tsx` usam `coverOf`.
- [ ] **Step 5:** Run: `scripts/mobile.sh verify` → check + build + test PASS.
- [ ] **Step 6: Verificação manual** — `cd mobile; nsh run` (núcleo real) com `FANART_TV`: pôsteres nítidos na temporada e no catálogo.
- [ ] **Step 7: Commit** — `feat(mobile): capas do fanart.tv com reserva no AniList`

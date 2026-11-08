# Catálogo de animes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transformar a seção Animes num catálogo A–Z contínuo com busca e filtros, organizar franquias/temporadas em abas por formato, mostrar versões como cartões legíveis e completar capas pelo MyAnimeList.

**Architecture:** Lógica pura (nomes de pasta, rótulo de versão, filtros) fica em `higanbana-core` com testes unitários. Rede nova (índice de letras, Jikan, limitador da nuvem) também no core, com cache em disco. As telas GPUI em `higanbana-desktop` consomem isso: `views/catalog.rs` (novo), `views/franchise.rs` (novo) e `views/title.rs` (página da obra).

**Tech Stack:** Rust 2024, GPUI + `gpui-kit`, tokio, reqwest, serde_json, regex. Testes: `cargo test -p higanbana-core` e `cargo test -p higanbana-desktop`.

**Spec:** `docs/superpowers/specs/2026-11-08-catalogo-animes-design.md`

## Global Constraints

- Branch `feat/catalogo-animes`. **Um commit por tarefa**, mensagem Conventional Commits em português (ex.: `feat(core): …`). **Nunca** adicionar trailer `Co-Authored-By` nem outra atribuição.
- Escopo só Animes: mangás continuam usando `Place::Shelf` como hoje.
- Nada pode varrer o acervo em profundidade; a nuvem dá **HTTP 429 depois de ~50 listagens seguidas**.
- Índice de letras: uma listagem por vez, **400 ms** entre listagens, recuo exponencial em 429 começando em **5 s** (teto 60 s); cache em disco **TTL 12 h**.
- Indexação começa quando a seção Animes é aberta e **pausa enquanto há vídeo tocando**.
- Jikan: `https://api.jikan.moe/v4`, no máximo **1 requisição por segundo**, respeitar 429. User-Agent genérico `higanbana-client/<versão>` (nunca e-mail).
- Ids de obras só da MAL: **`-mal_id`** (negativo).
- UI segue os Coding Guides e Design Guides do gpui-kit (skills `gpui-kit` e `gpui-kit-design-guides`: ler antes de tarefas de UI) e o tema `themes/higanbana.json`; textos da interface em português do Brasil.
- Mockups aprovados (referência visual): `.superpowers/brainstorm/1407-1794116529/content/` — `catalogo-layout.html` opção **B**, `franquia-layout.html` opção **B**, `versoes-layout.html` opção **A**.
- Antes de cada commit: `cargo build -p higanbana-desktop` sem erros e os testes do crate tocado passando.

## Review Focus

1. Nome de pasta com espaço duplo ou espaço antes da `/` (`[FILME] /`, `Precure  [TV]`): título limpo, ano e tipo corretos — teste em Task 1.
2. Obra com arquivos soltos **e** subpastas de release ao mesmo tempo: nenhuma é escondida, os soltos viram a versão "Arquivos da pasta" — teste em Task 2.
3. Filtro ativo enquanto metadados ainda não chegaram: obra sem metadados conta como "desconhecida", não some sem aviso (contador visível) — teste em Task 7.
4. 429 da nuvem no meio do índice: o índice recua e retoma, sem perder as letras já carregadas — teste em Task 3.
5. Cache antigo de matches com `null` (obras que o AniList não achou antes): precisam passar pelo fallback MAL, não ficar 7 dias sem capa — teste em Task 5.

---

### Task 1: Nome de obra (ano com decimal, ordem, tipo)

**Files:**
- Modify: `crates/higanbana-core/src/library.rs`

**Interfaces:**
- Produces:
  - `pub enum WorkKind { Tv, Movie, Ova, Ona, Special }` (derive `Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize`).
  - `pub struct WorkName { pub year: Option<u32>, pub order: u32, pub title: String, pub kind: Option<WorkKind> }`.
  - `pub fn parse_work_name(name: &str) -> WorkName`.
  - `pub fn is_dated(name: &str) -> bool` — começa com `[AAAA]` ou `[AAAA.N]`.
  - `pub fn is_franchise(dirs: &[&str]) -> bool` — pelo menos 2 subpastas e mais da metade com `is_dated`.
  - `WorkKind::from_anilist(format: &str) -> Option<WorkKind>` (`TV`/`TV_SHORT`→Tv, `MOVIE`→Movie, `OVA`→Ova, `ONA`→Ona, `SPECIAL`/`MUSIC`→Special).
  - `folder_year` passa a aceitar `[2005.1]`.

- [ ] **Step 1: Write the failing tests** (em `library::tests`)

```rust
#[test]
fn work_names() {
    let w = parse_work_name("[2005.1] Futari wa Precure Max Heart Movie 1 [FILME]");
    assert_eq!((w.year, w.order, w.kind), (Some(2005), 1, Some(WorkKind::Movie)));
    assert_eq!(w.title, "Futari wa Precure Max Heart Movie 1");
    let w = parse_work_name("[2004] Futari wa Precure  [TV]");
    assert_eq!((w.year, w.order, w.kind, w.title.as_str()), (Some(2004), 0, Some(WorkKind::Tv), "Futari wa Precure"));
    assert_eq!(parse_work_name("[2010.1] Heartcatch Precure! Movie Hana no Miyako [FILME] ").kind, Some(WorkKind::Movie));
    assert_eq!(parse_work_name("[2008.3] Precure All Stars GoGo Dream Live! [SPECIAL]").kind, Some(WorkKind::Special));
    assert_eq!(parse_work_name("[2011.3] X [OVA]").kind, Some(WorkKind::Ova));
    assert_eq!(parse_work_name("[2026] Meitantei Precure!").kind, None);
    assert_eq!(parse_work_name("Sem ano").year, None);
    assert_eq!(folder_year("[2005.1] X"), Some(2005));
    assert!(is_dated("[2005.0] X") && is_dated("[2004] X") && !is_dated("X (2004)"));
    assert!(is_franchise(&["[2004] A", "[2005.0] B", "Extras"]));
    assert!(!is_franchise(&["[2004] A"]));
    assert!(!is_franchise(&["[Aenianos] A", "B 1080p"]));
}
```

Também aceitar `[ESPECIAL]`→Special e `[ONA]`→Ona, sem diferenciar maiúsculas.

- [ ] **Step 2: Run** `cargo test -p higanbana-core library::tests` → FAIL (símbolos inexistentes).
- [ ] **Step 3: Implement** em `library.rs`. Ajustar a regex `YEAR` para `[\[(]((?:19|20)\d\d)(?:\.\d+)?[\])]`. `title` = `folder_title` do nome (já remove colchetes e normaliza espaços).
- [ ] **Step 4: Run** `cargo test -p higanbana-core` → PASS (inclusive os testes antigos de `folder_year`/`folder_title`).
- [ ] **Step 5: Commit** `feat(core): nome de obra com ano decimal, ordem e tipo`

---

### Task 2: Rótulo de versão e pastas de extras

**Files:**
- Modify: `crates/higanbana-core/src/library.rs`

**Interfaces:**
- Consumes: `folder_title`, `norm` (já existem).
- Produces:
  - `pub struct VersionLabel { pub name: String, pub group: Option<String>, pub resolution: Option<String>, pub source: Option<String>, pub audio: Option<String> }`.
  - `pub fn version_label(folder: &str, work_title: &str) -> VersionLabel`.
  - `pub fn resolution_rank(label: &VersionLabel) -> u32` — altura em pixels (`1080p`→1080, sem resolução→0).
  - `pub fn is_extra_folder(name: &str) -> bool` — `Extras`, `Especiais`, `Bônus`/`Bonus`, `Legendas`, `NCOP`, `NCED`, `OST`, `Scans` (compara via `norm`, também quando o nome começa com uma dessas palavras, ex. `NCOP e NCED`).
  - `pub const LOOSE_FILES_VERSION: &str = "Arquivos da pasta";`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn version_labels() {
    let v = version_label("[Aenianos]_Pretty_Cure_Splash_Star [640x480]", "Futari wa Precure Splash Star");
    assert_eq!((v.group.as_deref(), v.resolution.as_deref()), (Some("Aenianos"), Some("480p")));
    assert_eq!(v.name, "Aenianos");
    let v = version_label("PreCure.Splash.Star.S01.1080p.CR.WEB-DL.AAC2.0.H.264-TupaC", "Futari wa Precure Splash Star");
    assert_eq!((v.group.as_deref(), v.resolution.as_deref(), v.source.as_deref()), (Some("TupaC"), Some("1080p"), Some("CR")));
    let v = version_label("[DragsterPS] Glitter Force S01 e S02 [1080p] [Multi-Audio] [Multi-Subs]", "Smile Precure!");
    assert_eq!(v.name, "Glitter Force");
    assert_eq!((v.audio.as_deref(), v.resolution.as_deref()), (Some("Multi-Audio"), Some("1080p")));
    let v = version_label("[Aenianos] Smile Precure [1280x720][hi10p]", "Smile Precure!");
    assert_eq!((v.name.as_str(), v.resolution.as_deref()), ("Aenianos", Some("720p")));
    assert_eq!(version_label("Yes Precure 5 - 01 ~ 49 [1080p CR WEB-DL AVC AAC][MultiSub]", "Yes! Precure 5").source.as_deref(), Some("CR"));
    assert!(resolution_rank(&version_label("x [1080p]", "x")) > resolution_rank(&version_label("x [640x480]", "x")));
    assert!(is_extra_folder("Extras") && is_extra_folder("NCOP e NCED") && is_extra_folder("bônus"));
    assert!(!is_extra_folder("[Aenianos] Smile Precure"));
}
```

- [ ] **Step 2: Run** `cargo test -p higanbana-core library::tests::version_labels` → FAIL.
- [ ] **Step 3: Implement.** Regras:
  - **Grupo:** `[X]` no início, ou `-X` no fim depois do último `-` sem espaço.
  - **Resolução:** `\d{3,4}p`, ou `WxH` → `{H}p`.
  - **Fonte:** primeira entre `CR`, `NF`, `AMZN`, `BD`, `WEB-DL`, `WEB`, como palavra.
  - **Áudio:** `Dual`, `Multi-Audio`, `Dublado`, `Dub`.
  - **`name`:** pegar o nome sem colchetes, trocar `_`/`.` por espaço e tirar marcas técnicas (`S\d+`, `e S\d+`, resolução, codecs, faixas `01 ~ 49`). Se o que sobrou, normalizado, não estiver contido no `norm(work_title)` nem vice-versa, e tiver ao menos 3 letras, usar esse título ("Glitter Force"). Senão, usar o grupo. Se nada disso existir, usar `folder_title(folder)`.
- [ ] **Step 4: Run** `cargo test -p higanbana-core` → PASS.
- [ ] **Step 5: Commit** `feat(core): rótulo legível de versão e pastas de extras`

---

### Task 3: Índice de letras de Animes (com recuo em 429 e pausa)

**Files:**
- Create: `crates/higanbana-core/src/anime_index.rs`
- Modify: `crates/higanbana-core/src/lib.rs` (`pub mod anime_index;`), `crates/higanbana-core/src/core.rs` (campo + `pub fn anime_index(&self) -> &AnimeIndex`, arquivo `paths.cache.join("anime-index.json")`), `crates/higanbana-core/src/catalog.rs` (`Catalog::letter` passa a usar o índice).

**Interfaces:**
- Consumes: `Cloud::list`, `HttpError::Status { status: 429, .. }`.
- Produces:
  - `pub const LETTERS: [&str; 27]` = `"0-9"`, `"Letra A"` … `"Letra Z"`.
  - `pub struct IndexEntry { pub letter: String, pub name: String, pub path: String }` (`Clone, Serialize, Deserialize`).
  - `pub struct AnimeIndex`:
    - `open(file: PathBuf) -> Self`;
    - `loaded(&self, letter: &str) -> Option<Arc<Vec<IndexEntry>>>` (sem rede);
    - `async letter(&self, cloud: &Cloud, letter: &str) -> Result<Arc<Vec<IndexEntry>>, String>` (cache ou listagem, com recuo em 429);
    - `async fill_all(&self, cloud: &Cloud)` (percorre `LETTERS` em ordem, 400 ms entre listagens de rede, espera enquanto pausado);
    - `set_paused(&self, paused: bool)`;
    - `collections(&self) -> Vec<String>` (pastas da raiz `Animes` que não são letras, preenchido por `async root(&self, cloud)`);
    - `progress(&self) -> (usize, usize)` (letras carregadas, 27).
  - `pub fn backoff(attempt: u32) -> Duration` — 5 s, 10 s, 20 s, 40 s, 60 s, 60 s…
  - Entradas só de diretórios, em `natural_cmp`. Persistência como `{ at, letters: { letra: [IndexEntry] } }`, descartada após 12 h.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn backoff_grows_and_caps() {
    assert_eq!(backoff(0), Duration::from_secs(5));
    assert_eq!(backoff(1), Duration::from_secs(10));
    assert_eq!(backoff(4), Duration::from_secs(60));
    assert_eq!(backoff(9), Duration::from_secs(60));
}

#[test]
fn letters_in_order() {
    assert_eq!(LETTERS[0], "0-9");
    assert_eq!(LETTERS[1], "Letra A");
    assert_eq!(LETTERS[26], "Letra Z");
}

#[test]
fn persists_and_expires() {
    let dir = std::env::temp_dir().join(format!("idx-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("anime-index.json");
    let idx = AnimeIndex::open(file.clone());
    idx.store("Letra P", vec![IndexEntry { letter: "Letra P".into(), name: "Precure".into(), path: "Animes/Letra P/Precure".into() }]);
    assert_eq!(AnimeIndex::open(file.clone()).loaded("Letra P").unwrap()[0].name, "Precure");
    AnimeIndex::write_at(&file, 0); // força `at` antigo
    assert!(AnimeIndex::open(file).loaded("Letra P").is_none());
}
```

(`store` e `write_at` são `pub(crate)` só para teste/uso interno.)

- [ ] **Step 2: Run** `cargo test -p higanbana-core anime_index` → FAIL.
- [ ] **Step 3: Implement.** Em 429, dormir `backoff(tentativa)` e tentar de novo (no máximo 6 tentativas). Outros erros sobem como `String`. Não guardar letra que falhou. Pausa: `AtomicBool` + `tokio::sync::Notify`. `Catalog::letter` vira um adaptador que chama `anime_index.letter` e devolve os nomes. Para isso, `Catalog::check` recebe `&AnimeIndex`; ajustar `Core::check_catalog`.
- [ ] **Step 4: Run** `cargo test -p higanbana-core` → PASS; `cargo build -p higanbana-desktop` → OK.
- [ ] **Step 5: Commit** `feat(core): índice de letras de Animes com cache e recuo em 429`

---

### Task 4: Jikan (MAL) e campos novos do resumo

**Files:**
- Create: `crates/higanbana-core/src/anilist/jikan.rs`
- Modify: `crates/higanbana-core/src/anilist/mod.rs`, `crates/higanbana-core/src/anilist/queries.rs`, `crates/higanbana-core/src/anilist/types.rs`

**Interfaces:**
- Produces:
  - `SUMMARY_FRAGMENT` ganha `season idMal`.
  - `MediaSummary` ganha `#[serde(default)] pub season: Option<String>`, `#[serde(default)] pub id_mal: Option<i64>`.
  - `impl MediaSummary { pub fn is_mal_only(&self) -> bool { self.id < 0 } }`.
  - `pub fn idmal_batch_query(n: usize) -> String` em `queries.rs`: aliases `m{i}: Media(idMal: $i{i}, type: ANIME) { ...Summary }` com variáveis `$i{i}: Int`.
  - `jikan.rs`:
    - `pub struct JikanClient` com `new()`, `async search(&self, q: &str) -> AniListResult<Vec<Value>>` (`GET /anime?q=&limit=6&sfw=false`) e `async anime(&self, mal_id: i64) -> AniListResult<Value>` (`GET /anime/{id}`). Ambos devolvem itens já no formato AniList.
    - `pub fn to_summary_shape(item: &Value) -> Value`: AniList Summary shape, com `id = -mal_id`, `idMal = mal_id`, `type: "ANIME"`.
    - `pub fn to_detail_shape(item: &Value) -> Value`: o mesmo, mais `description` (de `synopsis`), `characters/staff/relations: { edges: [] }`, `studios: { nodes: [...] }` e `siteUrl` (url da MAL).
  - Mapas: `type` Jikan `TV`→`TV`, `Movie`→`MOVIE`, `OVA`→`OVA`, `ONA`→`ONA`, `Special`/`TV Special`→`SPECIAL`, `Music`→`MUSIC`. `season` `winter`→`WINTER` etc. `images.webp.large_image_url` (ou `jpg`) → `coverImage.extraLarge`/`large`. `title`→`romaji`, `title_english`→`english`, `title_japanese`→`native`, `title_synonyms`→`synonyms`. `year` (ou `aired.prop.from.year`) → `seasonYear`/`startDate.year`. `score` → `averageScore` (×10, arredondado). `genres[].name` + `themes[].name` → `genres`. `episodes`.

- [ ] **Step 1: Write the failing tests** (fixture JSON inline com um item Jikan realista de "Fushigiboshi no Futago Hime", mal_id 1531)

```rust
#[test]
fn jikan_item_to_anilist_shape() {
    let s = to_summary_shape(&fixture());
    let m: MediaSummary = serde_json::from_value(s).unwrap();
    assert_eq!(m.id, -1531);
    assert_eq!(m.id_mal, Some(1531));
    assert!(m.is_mal_only());
    assert_eq!(m.format.as_deref(), Some("TV"));
    assert_eq!(m.season.as_deref(), Some("SPRING"));
    assert_eq!(m.year(), Some(2005));
    assert_eq!(m.average_score, Some(72));
    assert!(m.genres.contains(&"Fantasy".to_string()));
    assert!(m.cover().unwrap().starts_with("https://"));
}

#[test]
fn detail_shape_has_empty_people() {
    let d: crate::anilist::types::MediaDetail = serde_json::from_value(to_detail_shape(&fixture())).unwrap();
    assert!(d.characters.edges.is_empty() && d.staff.edges.is_empty() && d.relations.edges.is_empty());
}

#[test]
fn builds_idmal_query() {
    let q = idmal_batch_query(2);
    assert!(q.starts_with("query ($i0: Int, $i1: Int)"));
    assert!(q.contains("m1: Media(idMal: $i1, type: ANIME)"));
}
```

(Se `MediaDetail` exigir outros campos, a fixture de detalhe inclui o mínimo para desserializar; ajustar `to_detail_shape` até passar.)

- [ ] **Step 2: Run** `cargo test -p higanbana-core anilist` → FAIL.
- [ ] **Step 3: Implement.** O cliente espaça as chamadas em ≥1 s (`Mutex<Instant>`). Em 429, espera 2 s e tenta de novo, no máximo 3 vezes. User-Agent igual ao do `AniListClient`.
- [ ] **Step 4: Run** `cargo test -p higanbana-core` → PASS.
- [ ] **Step 5: Commit** `feat(core): cliente Jikan e resumo com temporada e idMal`

---

### Task 5: Fallback MAL no `AniList::lookup` e detalhe de obras só-MAL

**Files:**
- Modify: `crates/higanbana-core/src/anilist/service.rs`, `crates/higanbana-core/src/anilist/matcher.rs` (só se precisar expor algo)

**Interfaces:**
- Consumes: Task 4 (`JikanClient`, `to_summary_shape`, `to_detail_shape`, `idmal_batch_query`).
- Produces:
  - `AniList::lookup` com a mesma assinatura, agora com fallback.
  - `AniList::media(id)` aceita id negativo (busca no cache de detalhes ou em `JikanClient::anime(-id)` + `to_detail_shape`).
  - Cache de matches passa a `anilist-matches-v2.json`: os `null` antigos não valem mais.

- [ ] **Step 1: Write the failing tests.** A parte de rede não é testável sem mock, então extrair a decisão pura:

```rust
/// Para cada busca sem match no AniList: o melhor candidato Jikan (formato AniList) ou None.
pub(crate) fn pick_mal(candidates: &[Value], wanted: &ParsedName) -> Option<Value>;
/// Junta resultado do AniList-por-idMal com o candidato Jikan: AniList vence; senão Jikan.
pub(crate) fn merge_mal(anilist: Option<Value>, jikan: Value) -> Value;

#[test]
fn mal_fallback_choice() {
    let jk = json!({"id": -1531, "idMal": 1531, "title": {"romaji": "Fushigiboshi no☆Futago Hime"}, "synonyms": [], "seasonYear": 2005});
    let wanted = matcher::parse_folder_name("Fushigiboshi no Futago Hime");
    assert_eq!(pick_mal(&[jk.clone()], &wanted).unwrap()["id"], -1531);
    assert_eq!(merge_mal(None, jk.clone())["id"], -1531);
    assert_eq!(merge_mal(Some(json!({"id": 777})), jk)["id"], 777);
}

#[test]
fn match_cache_is_versioned() {
    assert!(MATCHES_FILE.ends_with("-v2.json"));
}
```

E em `matcher::tests`: `parse_folder_name("[2005.1] Futari wa Precure Max Heart Movie 1 [FILME]")` == `p("Futari wa Precure Max Heart Movie 1", Some(2005))`. Hoje o ano com decimal se perde; corrigir em `parse_folder_name`.

- [ ] **Step 2: Run** `cargo test -p higanbana-core service` → FAIL.
- [ ] **Step 3: Implement.** Em `lookup`:
  1. Depois do lote AniList, para cada chave com resultado `None`, sequencialmente: `jikan.search(parsed.title)` e depois `pick_mal`.
  2. Junta os achados num único `idmal_batch_query` ao AniList (até 10 por vez), aplica `merge_mal` e grava no cache de matches.
  3. Quando o resultado é só-MAL, grava também `to_detail_shape` em `details` com a chave `id` (negativa).
  4. Falha da Jikan conta como "erro de rede" (não guarda `None`).
- [ ] **Step 4: Run** `cargo test -p higanbana-core` → PASS.
- [ ] **Manual check:** rodar `cargo run -p higanbana-core --example tree -- 1 "Animes/Letra F"`, achar uma obra sem capa no app antes da mudança e conferir que agora casa. Anotar no relatório qual foi.
- [ ] **Step 5: Commit** `feat(core): capas e metadados pelo MyAnimeList quando o AniList não acha`

---

### Task 6: Preferência de versão por obra

**Files:**
- Create: `crates/higanbana-core/src/prefs.rs`
- Modify: `crates/higanbana-core/src/lib.rs`, `crates/higanbana-core/src/core.rs` (`pub fn prefs(&self) -> &Prefs`, arquivo `paths.data.join("prefs.json")`)

**Interfaces:**
- Produces: `pub struct Prefs` com `open(file: PathBuf) -> Self`, `version(&self, work_path: &str) -> Option<String>`, `set_version(&self, work_path: &str, folder: &str)` (grava em disco na hora).

- [ ] **Step 1: Test** `prefs_roundtrip`: abrir em pasta temporária, `set_version("Animes/Letra P/Precure/[2012.0] Smile Precure! [TV]", "[DragsterPS] Glitter Force …")`, reabrir e ler o mesmo valor. Obra desconhecida devolve `None`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement** (`Mutex<HashMap<String,String>>` + JSON). **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(core): lembrar versão escolhida por obra`

---

### Task 7: Filtros e ordenação do catálogo (lógica pura)

**Files:**
- Create: `crates/higanbana-core/src/catalog_filter.rs`
- Modify: `crates/higanbana-core/src/lib.rs`

**Interfaces:**
- Consumes: `WorkKind` (Task 1), `MediaSummary.season/genres/format/average_score` (Task 4).
- Produces:
  - `pub enum Season { Winter, Spring, Summer, Fall }` com `from_anilist(&str)`.
  - `#[derive(Default, Clone, PartialEq)] pub struct Filters { pub query: String, pub kinds: Vec<WorkKind>, pub genres: Vec<String>, pub years: Option<(i32, i32)>, pub seasons: Vec<Season>, pub dubbed: bool }` com `needs_metadata(&self) -> bool` (qualquer filtro além de `query`/`dubbed`) e `is_empty()`.
  - `pub enum Sort { Az, Year, Score }`.
  - `pub enum Verdict { Pass, Fail, Unknown }`.
  - `pub fn judge(name: &str, meta: Option<&MediaSummary>, inner: &[MediaSummary], f: &Filters) -> Verdict`:
    - `query` compara `norm` do nome da pasta (contém).
    - `dubbed` via `is_dubbed(name)`.
    - O resto exige metadados. Sem `meta` e com `inner` vazio, devolve `Unknown`. Com `inner` não vazio (franquia já aberta), `Pass` se `meta` ou **qualquer** item de `inner` passar.
  - `pub fn is_dubbed(name: &str) -> bool` (`DUAL`, `Dublado`, `Multi-Audio`, sem diferenciar maiúsculas).
  - `pub fn sort_key_year(meta: Option<&MediaSummary>) -> i32` (sem ano vai para o fim) e `pub fn sort_key_score(meta: Option<&MediaSummary>) -> i64`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn judges_entries() {
    let tv2010 = summary(json!({"id":1,"format":"TV","seasonYear":2010,"season":"WINTER","genres":["Mahou Shoujo","Comedy"]}));
    let f = Filters { genres: vec!["Mahou Shoujo".into()], kinds: vec![WorkKind::Tv], ..Default::default() };
    assert_eq!(judge("Heartcatch Precure!", Some(&tv2010), &[], &f), Verdict::Pass);
    assert_eq!(judge("X", None, &[], &f), Verdict::Unknown);
    let movie = summary(json!({"id":2,"format":"MOVIE","seasonYear":2005,"genres":["Action"]}));
    assert_eq!(judge("Precure", Some(&movie), &[tv2010.clone()], &f), Verdict::Pass);
    assert_eq!(judge("Precure", Some(&movie), &[], &f), Verdict::Fail);
    let q = Filters { query: "heart catch".into(), ..Default::default() };
    assert_eq!(judge("[2010] Heartcatch Precure!", None, &[], &q), Verdict::Pass);
    let y = Filters { years: Some((2009, 2011)), seasons: vec![Season::Winter], ..Default::default() };
    assert_eq!(judge("x", Some(&tv2010), &[], &y), Verdict::Pass);
    assert!(is_dubbed("Naruto [DUAL]") && !is_dubbed("Naruto"));
    assert!(!Filters::default().needs_metadata());
    assert!(f.needs_metadata());
}
```

(`summary(v)` = helper de teste que desserializa `MediaSummary`.)

- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** `cargo test -p higanbana-core` → PASS.
- [ ] **Step 5: Commit** `feat(core): filtros e ordenação do catálogo`

---

### Task 8: Estado do catálogo no app (índice, metadados em segundo plano, pausa no vídeo)

**Files:**
- Create: `crates/higanbana-desktop/src/catalog_state.rs`
- Modify: `crates/higanbana-desktop/src/main.rs` (registrar módulo), `crates/higanbana-desktop/src/state.rs`, `crates/higanbana-desktop/src/app.rs` (`play`/fechar player chamam `set_playing`)

**Interfaces:**
- Consumes: `AnimeIndex` (Task 3), `AppState::want_lookup`/`lookup` (existentes).
- Produces (métodos em `AppState`, implementados em `catalog_state.rs` via `impl AppState`):
  - `start_anime_index(&mut self, cx)`: idempotente. Dispara `root()` + `fill_all()` no runtime do core e notifica a cada letra (poll do `progress()` a cada 500 ms enquanto incompleto).
  - `anime_letter(&self, letter: &str) -> Option<Arc<Vec<IndexEntry>>>` (sem rede).
  - `want_anime_letter(&mut self, letter: &str, cx)`: prioriza essa letra.
  - `anime_collections(&self) -> Vec<String>`.
  - `index_progress(&self) -> (usize, usize)` (letras).
  - `start_metadata_crawl(&mut self, cx)`: depois que uma letra chega, enfileira os nomes dela em `want_lookup` aos poucos, até **40 nomes a cada 3 s**. Não roda enquanto `playing`.
  - `metadata_progress(&self) -> (usize, usize)`: lookups resolvidos / total de nomes no índice.
  - `set_playing(&mut self, playing: bool, cx)`: guarda a flag e chama `core.anime_index().set_paused(playing)`.
  - `inner_works: HashMap<String, Vec<String>>` com `set_inner_works(title_path, names)` e `inner_summaries(&self, title_path) -> Vec<MediaSummary>` (para `judge`).

- [ ] **Step 1: Test** (puro, em `catalog_state.rs`) `crawl_batches`: a função `next_crawl_batch(pending: &mut Vec<String>, max: usize) -> Vec<String>` tira até `max` em ordem e não repete. Rodar `cargo test -p higanbana-desktop catalog_state` → FAIL.
- [ ] **Step 2: Implement** os métodos acima. `AppView::play` chama `s.set_playing(true)` e o fechamento (`PlayerEvent::Close`) chama `s.set_playing(false)`.
- [ ] **Step 3: Run** `cargo test -p higanbana-desktop` → PASS; `cargo build -p higanbana-desktop` → OK.
- [ ] **Step 4: Commit** `feat(desktop): estado do catálogo com índice e metadados em segundo plano`

---

### Task 9: Tela do catálogo — rota, grade A–Z contínua, índice lateral e busca

**Files:**
- Create: `crates/higanbana-desktop/src/views/catalog.rs`
- Modify: `crates/higanbana-core/src/library.rs` (`Place::Catalog { letter: Option<String> }` e `Place::Raw { path }` em `classify`), `crates/higanbana-desktop/src/views/mod.rs`, `crates/higanbana-desktop/src/app.rs` (rota)

**Interfaces:**
- Consumes: Task 8 (métodos de `AppState`), `poster`/`PosterProps`, `when_visible`, `columns`, `content_width`, `want_lookup`.
- Produces:
  - `classify`:
    - `"Animes"` → `Place::Catalog { letter: None }`;
    - `"Animes/Letra P"` e `"Animes/0-9"` → `Place::Catalog { letter: Some("Letra P") }`;
    - `"Animes/[Colecao Curtas]"` continua `Shelf`;
    - `"raw:<caminho>"` → `Place::Raw { path }` (abre `FolderView` no caminho cru).
  - `pub fn raw_path(path: &str) -> String` → `format!("raw:{path}")`.
  - `CatalogView::new(letter: Option<String>, window, cx)`.

- [ ] **Step 1: Tests** em `library::tests` (`classifies_catalog`): os casos acima, incluindo que `"Animes/Letra P/Precure"` continua `Place::Title`. Ajustar o teste antigo `classifies_places`, que esperava `Shelf` para `Animes/Letra B`. Rodar → FAIL.
- [ ] **Step 2: Implement `classify`** e rodar `cargo test -p higanbana-core` → PASS.
- [ ] **Step 3: Implement `CatalogView`** (layout do mockup catalogo B, sem os filtros, que vêm na Task 10):
  - Barra: campo de busca (`Input` + `InputState`) e "Ordenar" ainda desabilitado.
  - Grade de pôsteres por letra, na ordem `LETTERS`, com cabeçalho "F · 3".
  - Mostra as letras carregadas em sequência a partir da primeira. Um `when_visible` no fim da grade chama `want_anime_letter` da próxima letra (rolagem infinita). Cada card pede `want_lookup` quando visível (como `ShelfView`).
  - Índice lateral fixo `# A … Z`: clicar faz `want_anime_letter` e rola até o cabeçalho, usando `ScrollHandle` no contêiner da tela.
  - `letter: Some(...)` abre já rolado até a letra.
  - Clique no card vai para `entry.path`.
  - Busca por nome: com texto, filtra **todas** as letras já indexadas e oculta os cabeçalhos vazios; mostra "Indexando nomes… N/27" se o índice estiver incompleto.
  - `CatalogView::new` chama `start_anime_index`.
  - Desempenho: só renderizar cards de letras até a última pedida. Se a rolagem ficar pesada com mais de 1500 cards, virtualizar por linhas (decisão do implementador, registrar no relatório).
- [ ] **Step 4: Manual check:** `cargo run -p higanbana-desktop`.
  - Clicar em Animes mostra 0-9 e, rolando, A, B…
  - O índice lateral pula para P.
  - A busca "precure" acha a franquia.
  - Breadcrumb/voltar funcionam.
  - Nenhum 429 nos logs.
- [ ] **Step 5: Commit** `feat(desktop): catálogo de animes A–Z contínuo com busca`

---

### Task 10: Barra de filtros, ordenação, progresso, coleções e busca na nuvem

**Files:**
- Modify: `crates/higanbana-desktop/src/views/catalog.rs` (se passar de ~600 linhas, extrair `crates/higanbana-desktop/src/views/catalog_filters.rs` com a barra)

**Interfaces:**
- Consumes: `Filters`, `Sort`, `judge`, `Verdict`, `sort_key_*`, `Season` (Task 7); `metadata_progress`, `inner_summaries`, `anime_collections` (Task 8); `Cloud::search`.
- Produces: nada para tarefas seguintes.

- [ ] **Step 1: Implement**, segundo o mockup catalogo B:
  - **Botões de filtro** que abrem menus (`Popover`/`DropdownMenu` do gpui-kit):
    - Formato: TV, Filme, OVA, ONA, Especial.
    - Gênero: os gêneros presentes nos metadados já carregados, em ordem alfabética, multi.
    - Ano: dois campos numéricos "de"/"até".
    - Temporada: Inverno, Primavera, Verão, Outono.
    - Dublado: alternar.
  - O botão mostra a contagem ("Gênero · 1").
  - **Chips removíveis** para cada filtro ativo e "Limpar".
  - **Ordenar:** A–Z, Ano, Nota. Fora de A–Z não há cabeçalhos de letra: a grade é uma só, usando só as letras já indexadas.
  - **Filtro com metadados:** obra com `Verdict::Unknown` não aparece. A linha de status mostra "N obras · metadados X / Y" enquanto `metadata_progress` estiver incompleto, e só "N obras" depois.
  - **Com filtro ativo**, a rolagem infinita para de depender de visibilidade: as letras continuam chegando em segundo plano via `fill_all`.
  - **Coleções:** menu "Coleções" com `anime_collections()`; cada item navega para `Animes/<coleção>`.
  - **Busca na nuvem:** com texto de ≥3 letras, debounce de 400 ms, chama `Cloud::search` e mostra os resultados `SearchKind::Anime` cujo caminho tem profundidade > 3 (obras dentro de franquias) num grupo "Dentro de franquias" acima da grade. O card leva ao caminho do resultado.
  - **Estado vazio:** "Nenhuma obra com esses filtros" + botão "Limpar filtros".
- [ ] **Step 2: Manual check.**
  - Filtrar por Gênero Mahou Shoujo + TV mostra o contador subindo enquanto indexa.
  - Remover o chip restaura a lista.
  - Ordenar por Ano funciona.
  - Buscar "heartcatch" mostra a obra dentro de Precure.
  - Coleções abre `[Colecao Curtas]`.
- [ ] **Step 3: Run** `cargo build -p higanbana-desktop` e `cargo test -p higanbana-desktop` → OK.
- [ ] **Step 4: Commit** `feat(desktop): filtros, ordenação e busca dentro de franquias no catálogo`

---

### Task 11: Página da franquia (abas por formato)

**Files:**
- Create: `crates/higanbana-desktop/src/views/franchise.rs`
- Modify: `crates/higanbana-desktop/src/views/title.rs`, `crates/higanbana-desktop/src/views/mod.rs`

**Interfaces:**
- Consumes: `parse_work_name`, `is_franchise`, `WorkKind`, `WorkKind::from_anilist`, `is_extra_folder` (Tasks 1–2); `set_inner_works` (Task 8); `raw_path` (Task 9).
- Produces:
  - `pub fn franchise_tabs(works: &[(String, WorkName, Option<MediaSummary>)]) -> Vec<(FranchiseTab, Vec<usize>)>`, com `pub enum FranchiseTab { Tv, Movies, Specials, Others }`:
    - Tv = `WorkKind::Tv`/`Ona`;
    - Movies = `Movie`;
    - Specials = `Ova`/`Special`;
    - Others = sem ano.
    - O formato do AniList/MAL (`from_anilist`) vence a tag do nome.
    - Ordem por `(year, order)`.
    - Só abas não vazias.
  - `FranchiseView::new(title_path: String, works: Vec<FileEntry>, cx)`.

- [ ] **Step 1: Test** `franchise_tabs_by_kind` em `franchise.rs`. Montar 4 obras:
  - `[2004] A [TV]`;
  - `[2005.1] B [FILME]`;
  - `[2019.0] Star Twinkle Precure [FILME]` com `meta.format = "TV"`;
  - `Extras sem ano`.

  Esperar: Tv = [A, Star Twinkle]; Movies = [B]; Others = [Extras]; sem aba Specials. Rodar `cargo test -p higanbana-desktop franchise` → FAIL.
- [ ] **Step 2: Implement.** `TitleView`: quando `rest` está vazio e `is_franchise(nomes das subpastas)`, renderiza `FranchiseView` e não a página de obra.
  - **Herói:** capa da franquia (lookup do nome da pasta), nome e "Franquia · 2004–2026".
  - **`TabBar`** "Séries TV · 22 / Filmes · 26 / Especiais · 3 / Outros". Se houver uma aba só, sem `TabBar`.
  - **Grade de pôsteres:** cada card faz `want_lookup` do nome **completo** da subpasta (o matcher usa o ano) e mostra ano + nº de episódios do resumo.
  - **Clique:** vai para `title_path/<subpasta>`.
  - **Link "Ver pasta":** abre `raw_path(title_path)`.
  - Chama `set_inner_works(title_path, nomes)`.
  - Remover o grupo "Temporada" (option buttons) da página de obra. A obra dentro de franquia mostra um link de volta "← <franquia>" acima do título.
- [ ] **Step 3: Manual check.**
  - Abrir Precure mostra as abas com contagens.
  - Star Twinkle aparece em Séries TV.
  - Cada obra tem capa própria (ou placeholder).
  - Clicar em Heartcatch abre os episódios.
  - O link de volta funciona.
  - A franquia custa uma listagem.
- [ ] **Step 4: Run** `cargo test -p higanbana-desktop` → PASS.
- [ ] **Step 5: Commit** `feat(desktop): página de franquia com abas por formato`

---

### Task 12: Página da obra — cartões de versão, extras e versão lembrada

**Files:**
- Modify: `crates/higanbana-desktop/src/views/title.rs`, `crates/higanbana-desktop/src/components.rs` (novo `version_card`)

**Interfaces:**
- Consumes: `version_label`, `resolution_rank`, `is_extra_folder`, `LOOSE_FILES_VERSION` (Task 2); `core.prefs()` (Task 6); `raw_path` (Task 9); `MediaSummary::is_mal_only` (Task 4).
- Produces:
  - `pub fn version_card(id: impl Into<ElementId>, label: &VersionLabel, episodes: Option<usize>, selected: bool, cx: &App) -> Stateful<Div>` em `components.rs`.
  - Em `title.rs`, `pub(crate) fn split_work_dirs(dirs: Vec<FileEntry>) -> (Vec<FileEntry>, Vec<FileEntry>)` (versões, extras) e `pub(crate) fn default_version(versions: &[FileEntry], remembered: Option<&str>, work_title: &str) -> Option<String>`.

- [ ] **Step 1: Tests** (`title.rs` tests):
  - `split_work_dirs(["[Aenianos] Smile Precure [1280x720][hi10p]", "[DragsterPS] Glitter Force S01 e S02 [1080p] [Multi-Audio] [Multi-Subs]", "Extras"])` separa 2 versões e 1 extra.
  - `default_version` com lembrada válida devolve a lembrada.
  - Com lembrada inexistente ou `None`, devolve a de maior `resolution_rank` (a Glitter Force, 1080p).

  Rodar → FAIL.
- [ ] **Step 2: Implement.**
  - **Versões** (`releases()`) = subpastas não-extra da obra. Com vídeos soltos na pasta da obra **e** subpastas de versão, entra também a versão sintética `LOOSE_FILES_VERSION`, que aponta para a própria pasta.
  - **Cartões** do mockup versoes A: nome em destaque, etiquetas resolução/fonte/áudio e "N ep". A contagem de episódios só aparece para a versão carregada: as outras não são listadas, para economizar requisições.
  - **Escolha:** clicar navega como hoje e chama `core.prefs().set_version(work_path, pasta)`. Ao abrir sem versão no caminho, usar `default_version`.
  - **Uma versão só:** sem cartões, só as etiquetas, como hoje.
  - **Seção "Extras"** abaixo dos episódios, com um link por pasta extra (navega para o caminho, que cai em `Place::Title` com `rest`).
  - **Link "Ver pasta"** (`raw_path`) ao lado das ações.
  - **Obra só-MAL:** abas Personagens/Equipe/Relacionadas já somem (edges vazios). Conferir que Detalhes mostra a sinopse e um link "Ver no MyAnimeList" (`siteUrl`).
- [ ] **Step 3: Manual check.**
  - Smile Precure! mostra 2 cartões ("Aenianos 720p", "Glitter Force 1080p · Multi-Audio").
  - A escolha é lembrada ao voltar.
  - Splash Star mostra 480p e "TupaC 1080p CR".
  - Uma obra com `Extras` mostra a seção.
  - Uma obra só-MAL abre sem erro.
- [ ] **Step 4: Run** `cargo test -p higanbana-desktop` e `cargo test -p higanbana-core` → PASS.
- [ ] **Step 5: Commit** `feat(desktop): cartões de versão, extras e versão lembrada na página da obra`

---

### Task 13: Limpeza final

**Files:**
- Delete: `crates/higanbana-core/examples/tree.rs` (nunca commitado; apagar do disco)

- [ ] **Step 1:** Rodar `cargo test --workspace` e `cargo build -p higanbana-desktop --release` → OK. Rodar `cargo clippy --workspace` e corrigir avisos novos introduzidos pela branch.
- [ ] **Step 2:** Apagar `crates/higanbana-core/examples/` e conferir que `git status` está limpo.
- [ ] **Step 3: Commit** (só se o clippy mudou algo) `chore: ajustes do clippy no catálogo`

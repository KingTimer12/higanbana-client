# Capas do fanart.tv — design

Data: 2026-11-12

## Intenção

As capas que o AniList entrega (`coverImage.extraLarge`/`large`) têm qualidade baixa
(pequenas, compressão visível). O fanart.tv tem pôsteres em alta resolução. A capa de
cada obra passa a vir do fanart.tv; o AniList continua fornecendo o **banner**
(`bannerImage`) e vira **reserva** da capa.

## Objetivos

1. Capa de toda obra de anime vem do fanart.tv quando ele tiver o pôster **daquela
   temporada**.
2. Sem pôster da temporada no fanart → capa do AniList (decisão do usuário: preferir a
   temporada certa em baixa qualidade a um pôster geral de outra temporada).
3. Vale em **todas as telas** com capa, no **desktop e no mobile**.
4. Pôster baixado é reduzido e guardado **em disco como WebP**; a memória não cresce por
   causa da alta resolução.
5. O cache de imagens em memória do desktop (`AppState::images`) ganha **limite**, com
   descarte das menos usadas recentemente.

## Fora do escopo

- Banner: segue do AniList, sem mudança (`views/title.rs:584`, `mobile/pages/Title.tsx:110`).
  O fundo da página da obra quando não há banner continua usando a capa do AniList.
- Mangá: fanart.tv não tem; capa de mangá segue do AniList.
- JPEG XL em disco: adiado (precisaria da libjxl em C++ no build e decodificação extra; o
  gpui não abre JXL). A função que grava a capa deve ficar isolada para trocar o formato depois.
- A app Tauri/Solid antiga em `src/` não faz parte (o workspace é `crates/*` + `mobile/`).

## Anti-objetivos (falha mesmo funcionando)

- Pôster de **outra temporada** na capa de uma obra (ex.: arte da 1ª temporada na 2ª).
- Tela segurando/esperando o fanart: a capa do AniList aparece na hora; a do fanart troca
  quando chegar.
- Rajada de requisições: centenas de buscas no TMDB ou fanart ao rolar o catálogo.
  (Lembrete do projeto: a conta da Anitsu já foi bloqueada por 429 — nada aqui pode
  tocar na nuvem da Anitsu.)
- Build quebrando sem a chave: sem `FANART_TV`, o app funciona igual a hoje.
- Memória subindo sem limite ao rolar o catálogo.

## Decisões

### Chave

- Variável `FANART_TV` (já em `.env.example`), embutida mascarada em `crates/higanbana-core/build.rs`
  com `embed_optional`, como `TMDB_TOKEN`/`TMDB_KEY`. Leitura num `fanart/credentials.rs` no mesmo
  molde de `tmdb/credentials.rs` (reaproveitar `unmask`, de preferência movendo-o para um módulo comum).
- Sem chave: `NotConfigured`, sem rede, capa do AniList.

### AniList/MAL → fanart (decisão do Claude)

O fanart.tv é indexado por **TVDB id** (séries) e **TMDB id** (filmes), não por AniList.
Mapeamento pela lista da comunidade **Fribb/anime-lists** (`anime-list-full.json`, no GitHub
raw), que liga `anilist_id`/`mal_id` → `thetvdb_id`, temporada no TVDB, `themoviedb_id`, `type`.

- Baixar a lista uma vez, **compactar** só os campos usados (anilist_id, mal_id, tvdb_id,
  temporada tvdb, tmdb_id, se é filme) num arquivo próprio no cache, e manter na memória só
  esse mapa compacto (alvo: < 1 MB). Atualizar a cada 7 dias; falha ao atualizar mantém a
  versão anterior.
- Obra só da MAL (`MediaSummary.id < 0`, `id_mal`): procurar por `mal_id`.
- **Não** usar o casamento do TMDB (`Tmdb::work_episodes`) para isso: ele faz várias
  chamadas por obra e só roda na página da obra.
- Obra ausente da lista → AniList.

O builder deve conferir o formato atual do JSON (nomes dos campos, em especial o da
temporada) antes de modelar.

### Escolha do pôster (decisão do Claude, regra de temporada do usuário)

API v3 do fanart.tv: `GET /v3/tv/{tvdb_id}?api_key=…` e `GET /v3/movies/{tmdb_id}?api_key=…`.
O usuário citou `https://api.fanart.tv/`; a base documentada historicamente é
`https://webservice.fanart.tv/v3/` — o builder confere qual responde e usa essa.

- **Série, temporada N**: `seasonposter` com `season == N`.
- **Temporada 1** sem `seasonposter` da 1: aceita `tvposter` (o pôster geral normalmente é
  a arte da 1ª temporada). Para N > 1, **nunca** `tvposter` → AniList.
- **Filme** (lista diz que é filme, ou `format == MOVIE`): `movieposter` pelo TMDB id.
- Temporada desconhecida na lista: trata como 1 só se a série tiver uma temporada só no
  mapeamento; senão AniList.
- **Temporada dividida** (várias obras do AniList apontando para o mesmo `(tvdb_id, temporada)`,
  ex.: "Part 2", "2nd Cour"): só a **primeira parte** usa o pôster do fanart; as demais →
  AniList (decisão do usuário). O mapa compacto guarda, por `(tvdb_id, temporada)`, quais ids
  o compartilham. Primeira parte = menor `anilist_id` do grupo, porque obras mais antigas
  costumam ter id menor (decisão do Claude; a lista não traz datas).
- **Temporada 0** (especiais, OVAs, ONAs no TVDB) → AniList. (Decisão do Claude.)
- Numeração absoluta (`"a"` ou equivalente na lista) → tratada como temporada desconhecida.
  (Decisão do Claude.)
- Desempate: idioma `en` > sem texto (`00`/vazio) > `ja` > outros; dentro do idioma, mais `likes`.

### Cache e disco

- Resposta do fanart → URL escolhida (ou "não tem") num `DiskCache` (como o TMDB):
  achado 7 dias, "não tem" 1 dia. Erro de rede nunca vai para o cache.
- Pôster: baixar a URL, **reduzir para 480 px de largura** (proporção mantida; ~1,4 MB
  decodificado), codificar **WebP com perda, qualidade ~85** (crate `webp`, libwebp via `cc`;
  o `image` só faz WebP sem perda) e gravar em `paths.cache/covers/<hash>.webp`.
  Largura e qualidade em constantes.
  Se a libwebp (`cc`) não compilar no build Android do `nsh`, o codificador fica atrás de uma
  feature cargo e o mobile grava **JPEG reduzido** pelo crate `image` (Rust puro), sem
  bloquear a entrega. (Decisão do Claude.)
- Tela de login (`views/login.rs`) usa a mesma resolução; o fanart não depende de sessão.
  (Decisão do Claude.)
- Pasta `covers/` com teto de **300 MB**: na abertura, apagar os mais antigos (mtime) até
  caber. (Decisão do Claude.)
- Concorrência: no máximo 4 downloads/consultas ao fanart ao mesmo tempo; o desktop e o
  mobile só pedem capa de card que está na tela (o pedido acompanha a montagem do card,
  como `wantLookup` no mobile).

### API do núcleo

Um serviço `fanart` no `higanbana-core`, montado no `Core` como o `Tmdb`. Forma sugerida
(o builder pode ajustar): `Core::cover(&MediaSummary) -> Option<PathBuf>` assíncrono, que
resolve mapa → fanart → arquivo WebP e devolve o caminho local; `None` = usar AniList.
Também uma consulta síncrona barata "já tenho arquivo para esta obra?" para a capa local
aparecer sem piscar quando a tela é revisitada.

### Desktop (`crates/higanbana-desktop`)

- Ponto central: `components::cover` (`components.rs:110`), usado por pôster, estante,
  catálogo, home, franquia etc.; mais as miniaturas da busca (`chrome.rs:154`, `cover_small`)
  e a tela de login (`views/login.rs:36,88`). Todas passam pela mesma resolução.
- Mostra a do AniList; pede a do fanart; quando o arquivo existir, troca.
- **Limite do `AppState::images`** (`state.rs:109`, hoje `HashMap` sem descarte): LRU por
  **orçamento de bytes decodificados estimados** (largura × altura × 4, lidos do cabeçalho),
  orçamento padrão **384 MB** em constante. Uso marcado em `image()`/`cached_image()` (a tela
  chama a cada frame, então o que está visível nunca sai). Vale para tudo que passa pelo mapa,
  inclusive páginas do leitor e capas da nuvem (`put_image`).
  O builder deve confirmar que soltar o `Arc<gpui_kit::Image>` libera também a textura/asset
  do gpui; se não liberar, descartar explicitamente no gpui.

### Mobile (`mobile/`)

- Nova função nativa em `mobile/src/lib.rs` (módulo `Catalog`) que devolve o caminho/URI do
  WebP local para uma mídia, ou `null`.
- `store.ts`: sinal por id (como `lookupOf`/`wantLookup`), pedido quando o card aparece.
- `Cover`/`mediaCover` (`components/ui.tsx:200`, `lib/format.ts:19`, `components/Poster.tsx:66`,
  `pages/Title.tsx:134`): usam a capa do fanart quando houver, senão a do AniList.
- O builder confere se o `Image` do `nsh` abre arquivo local (`file://`); se não abrir,
  servir o arquivo pelo servidor HTTP local que o núcleo já tem (`player/proxy.rs`).
- Sem LRU próprio no mobile: o `Image` nativo cuida da memória.
- Fixture (`mobile/fixture/fake.ts`): simular capa do fanart para alguns itens.

## Deixado para o builder

- Nomes de módulos/tipos, onde exatamente fica o `unmask` comum.
- Detalhes do formato do arquivo compactado do mapa.
- Testes: escolha do pôster (temporada, idiomas, likes), mapeamento, LRU por orçamento,
  redução + WebP; sem rede nos testes.
- README: seção da chave `FANART_TV`, ao lado da do TMDB.

# Catálogo de animes: design

Data: 2026-11-08. Escopo: só a seção **Animes** do app desktop (GPUI, `crates/higanbana-desktop` + `crates/higanbana-core`). Branch `feat/catalogo-animes`, a partir do `master`.

## Intenção

Hoje, clicar em "Animes" mostra pastas (`Letra A`, `Letra B`, coleções) e cada letra é uma estante de pôsteres separada. O dono quer um **catálogo**: todas as obras do acervo numa grade só, rolando de 0-9 até Z, com **filtragem avançada**.

O dono costuma procurar **pelo nome**. Quando não lembra o nome, procura por **gênero**, **ano** e às vezes **temporada** (inverno, primavera…).

Hoje também há dois problemas que um amigo encontrou ao testar o app:

1. **Organização de temporadas e versões ruim.** Ele procurou a franquia Precure para chegar a uma série específica. A página da franquia mostrou só 5 das ~50 obras como "temporada" e misturou o resto como "versão".
2. **Animes sem capa.** O Anitsu nomeia as pastas pelo **MyAnimeList**, e a busca pelo nome no AniList às vezes não acha a obra.

Sucesso: abrir Animes e achar qualquer obra por nome ou por filtros; abrir uma franquia grande e chegar na série certa sem confusão; escolher a versão com informação legível; quase toda obra com capa.

## Fatos verificados (sondagem de 2026-11-08)

- **Raiz `Animes/`:** `0-9`, `Letra A` … `Letra Z` (27 pastas) e 6 coleções (`[Colecao Curtas]`, `[Colecao Transformers]`, …). `Letra P` tem 143 títulos, o que dá umas ~4 mil obras no total.
- **Pasta de título** fica em `Animes/<Letra>/<Título>` (profundidade 3, ver `library::classify`).
- **Franquia (exemplo `Animes/Letra P/Precure`):** cada subpasta é uma obra, no formato `[ANO.ORDEM] Nome [TIPO]`:
  - `[2004] Futari wa Precure  [TV]`, `[2005.0] Futari wa Precure Max Heart [TV]`, `[2005.1] … Movie 1 [FILME]`, `[2008.3] Precure All Stars GoGo Dream Live! [SPECIAL]`, `[2011.3] … [OVA]`, `[2026] Meitantei Precure!` (sem tipo).
  - Os tipos vistos são `[TV]`, `[FILME]`, `[OVA]` e `[SPECIAL]`. Há erros de catalogação: `[2019.0] Star Twinkle Precure [FILME]` tem 49 episódios.
  - Há espaços duplos e espaço sobrando antes da `/` (`[FILME] /`).
- **Versões** ficam um nível abaixo da obra:
  - Os arquivos podem estar direto na pasta da obra (só uma versão).
  - Ou há subpastas de release:
    - Splash Star: `[Aenianos]_Pretty_Cure_Splash_Star [640x480]` e `PreCure.Splash.Star.S01.1080p.CR.WEB-DL.AAC2.0.H.264-TupaC`.
    - Smile: `[Aenianos] Smile Precure [1280x720][hi10p]` (48 ep) e `[DragsterPS] Glitter Force S01 e S02 [1080p] [Multi-Audio] [Multi-Subs]` (40 ep, dublagem americana, numeração própria).
- **Causa do "sumiu temporada":** `library::folder_year` usa `[\[(]((?:19|20)\d\d)[\])]`, que não aceita `[2005.1]`. `TitleView::seasons` só conta como temporada a subpasta com ano, então 45 de 50 obras viraram "versão".
- **Limite da nuvem:** **HTTP 429 "Too many requests"** depois de ~50 listagens seguidas. Nada pode varrer o acervo em profundidade.
- **AniList hoje:** `AniList::lookup` busca pelo nome em lotes de 10 (aliases) e guarda o resultado em disco por 7 dias (`anilist-matches.json`). `SUMMARY_FRAGMENT` já traz `format`, `genres`, `seasonYear`, `startDate`, `coverImage`, `averageScore`, `popularity` e `episodes`, mas **não** traz `season` nem `idMal`.

## Decisões do dono

1. **Catálogo contínuo:** clicar em Animes mostra **todas as obras**, sem pastas. A lista começa em 0-9, e ao chegar no fim de uma letra a próxima entra (rolagem infinita), com cabeçalho por letra.
2. **Filtros numa barra no topo** (mockup "B"): busca por nome, botões que abrem menus (Formato, Gênero, Ano, Temporada, …), chips removíveis para os filtros ativos e "Limpar".
3. **Franquia como um card só** no catálogo ("Precure", uma pasta só no acervo). Ao abrir, a **página da franquia tem abas por formato** (Séries TV · Filmes · Especiais), cada uma com grade cronológica e **capa própria por obra**.
4. **Versões como cartões** acima da lista de episódios: nome legível + etiquetas (resolução, fonte, áudio, nº de episódios). O app lembra a última versão escolhida.
5. **Capa de fallback pelo MyAnimeList** quando o AniList não acha pelo nome.

## Decisões minhas (o dono não opinou; pode contestar)

- **Fallback MAL via Jikan** (`api.jikan.moe/v4`, pública, sem login, ~60 req/min). Ordem:
  1. AniList pelo nome (como hoje).
  2. Se não casar, Jikan `GET /anime?q=<título>&limit=6` e escolher o candidato com o mesmo matcher.
  3. Com o `mal_id`, AniList `Media(idMal:, type: ANIME)` em lote, para os metadados ficarem uniformes.
  4. Se o AniList não tiver a obra, montar um `MediaSummary` com os dados da Jikan (capa, gêneros, ano, temporada, formato, episódios).

  Tudo entra no mesmo cache de matches.
- **Índice de nomes completo, renderização preguiçosa.** As 27 pastas de letra são listadas em segundo plano, **uma por vez com intervalo** (ex.: 400 ms, recuando em 429), e ficam em disco com TTL de 12 h. Isso dá todos os nomes em segundos, então a busca por nome cobre o acervo inteiro mesmo antes de rolar até a letra. Na tela, a grade continua aparecendo letra por letra conforme a rolagem.
- **Metadados para filtros em segundo plano.** Casar ~4 mil obras com o AniList leva ~400 requisições em lote, alguns minutos na primeira vez. Primeiro vêm os cards visíveis, depois o resto em ritmo baixo. Enquanto não termina, a barra mostra o progresso ("metadados 3.120 / 4.031"), e os filtros valem só para as obras já indexadas, sem esconder isso.
- **Busca:** filtra os nomes do índice localmente na hora. Uma busca na API da nuvem (`Cloud::search`, com debounce) também acha obras **dentro** de franquias ("Heartcatch" → `Precure/[2010.0] Heartcatch Precure! [TV]`) e mostra esses resultados num grupo à parte.
- **Filtros:**
  - Formato (TV, Filme, OVA, ONA, Especial).
  - Gênero (vários; a obra precisa ter todos).
  - Ano (intervalo).
  - Temporada (inverno/primavera/verão/outono).
  - "Dublado", só pela tag no nome da pasta (`DUAL`, `Dublado`, `Multi-Audio`), então pode deixar obras de fora.
  - **Ordenar:** A–Z (padrão, com cabeçalhos de letra), Ano (novo→velho) e Nota.
  - Fora de A–Z, os cabeçalhos de letra somem e a grade é uma só.
  - Pede adicionar `season` e `idMal` ao `SUMMARY_FRAGMENT`.
- **Índice A–Z lateral** (# A … Z) para pular de letra; clicar carrega até aquela letra e rola.
- **Franquia nos filtros:** o card da franquia usa o match do próprio nome da pasta. As obras de dentro só ficam conhecidas quando a franquia é aberta (ou aparecem na busca da nuvem); depois disso, a franquia passa nos filtros se **qualquer** obra dela passar.
- **O que conta como franquia/temporadas:** pasta de título cuja maioria das subpastas começa com `[ANO]` ou `[ANO.N]`. Vale também para um anime comum com várias temporadas.
  - Se todas as obras forem do mesmo formato, não há abas, só a grade.
  - Pasta de título sem subpastas de ano vai direto para a página da obra.
- **Ano e tipo pelo nome:**
  - `[2005.1]` → ano 2005, ordem 1.
  - `[TV]`/`[FILME]`/`[OVA]`/`[SPECIAL]`/`[ESPECIAL]`/`[ONA]` → formato.
  - Quando o AniList/MAL casar, o formato deles vale mais que a tag (corrige Star Twinkle).
  - Espaços repetidos são normalizados.
- **Rótulo da versão** sai do nome da pasta:
  - **Grupo:** `[Aenianos]` no início ou `-TupaC` no fim.
  - **Resolução:** `1080p`, ou `1280x720` → `720p`.
  - **Fonte:** CR, NF, BD, WEB-DL.
  - **Áudio:** Dual, Multi-Audio, Dub.
  - **Nº de episódios:** contagem de vídeos da pasta.
  - Quando o nome traz um título diferente da obra ("Glitter Force"), esse título vira o nome do cartão.
  - **Versão padrão:** a última escolhida para aquela obra (persistida); senão, a de maior resolução.
- **Coleções** (`[Colecao …]`) ficam fora da grade A–Z e vão para um menu "Coleções" na barra. Os caminhos antigos `Animes/Letra X` abrem o catálogo já na letra X.
- **Saída de emergência:** a página da obra e a da franquia mantêm um link "Ver pasta" para a navegação crua de arquivos, para nada do acervo ficar inacessível se a heurística errar.

### Pontos da revisão (também decisões minhas)

- **Card da franquia é sempre um card só**, com ou sem filtro. O mockup do catálogo mostrava obras de Precure separadas; vale a decisão 3 do dono. Obras internas aparecem soltas apenas no grupo de resultados da busca da nuvem. O modelo do catálogo é a **lista de pastas de título**.
- **Cards de obra na franquia não listam pastas.** A página da franquia custa **uma** listagem (a da pasta da franquia). O nº de episódios no card vem do AniList/MAL (`episodes`), quando houver. O selo "N versões" sai do card e só aparece dentro da obra.
- **Um limitador global de requisições à nuvem no core**, por onde passam listagem, busca e a indexação. A indexação de nomes começa quando a seção Animes é aberta, não ao iniciar o app, e **pausa enquanto há vídeo tocando**. Em 429: recuo exponencial (começando em ~5 s), retomada automática e aviso discreto na barra se demorar. Não se sabe a janela exata do 429; o recuo cobre isso.
- **O que é versão dentro de uma obra:**
  - Subpastas com nome de extra conhecido (`Extras`, `Especiais`, `Bônus`, `Legendas`, `NCOP`, `NCED`, `OST`, `Scans`; sem diferenciar maiúsculas e acentos) **não** viram versão. Elas ficam numa seção "Extras" abaixo dos episódios, com link para a pasta.
  - Arquivos de vídeo soltos ao lado de subpastas viram uma versão própria ("Arquivos da pasta").
  - Nenhuma subpasta é escondida.
- **Subpastas sem ano numa franquia** vão para uma aba "Outros" (só aparece se existir).
- **Obra só com dados da Jikan:**
  - `MediaSummary` ganha a origem (AniList ou MAL); o id da MAL vai com sinal negativo para não colidir com ids do AniList.
  - A página da obra mostra capa, título, ano, formato, gêneros, sinopse (`synopsis` da Jikan) e episódios. Personagens, Equipe e Relacionadas somem para essas obras.
  - O `Catalog` (conferir se um anime do AniList está no acervo) ignora esses ids.

## Objetivos

- Catálogo A–Z contínuo com busca e filtros, sem estourar o limite da nuvem.
- Página de franquia/temporadas com abas por formato e capa por obra.
- Cartões de versão legíveis, com versão lembrada.
- Fallback MAL para capas e metadados.

## Não objetivos

- Mangás (a estante por letra continua como está).
- Tela inicial, player, leitor, músicas.
- Listas do usuário, favoritos, histórico de "assistindo".
- Redesenho das coleções.

## Anti-objetivos (falha mesmo se "funcionar")

- Disparar 429 na nuvem durante uso normal.
- Filtro que esconde obras em silêncio porque o índice ainda não terminou.
- Obra ou arquivo que deixa de ser alcançável pela interface.
- Travar a UI enquanto indexa: indexação sempre no runtime do core, com atualização incremental.

## Restrições

- Stack existente: GPUI + `gpui-kit` (seguir os Coding Guides e Design Guides do gpui-kit) e tema `themes/higanbana.json`.
- Toda rede passa pelo `higanbana-core`, com cache em disco nas pastas de `Paths`.
- Respeitar os limites: nuvem (~50 listagens em sequência), AniList (lotes, ~30–90 req/min) e Jikan (~3 req/s, 60/min).
- Commits sem trailer de coautoria.

## O que fica para quem implementa

- Estrutura dos módulos novos (ex.: `catalog` de índice no core, `views/catalog.rs`, `views/franchise.rs`) e como o `Place` ganha as variantes novas.
- Virtualização da grade (lista de linhas) e o ponto exato de disparo do carregamento da próxima letra.
- Ritmo exato das filas de indexação, e o backoff em 429 da nuvem e do AniList/Jikan.
- Visual fino de chips, menus e cartões, dentro do tema atual e dos mockups em `.superpowers/brainstorm/` (catalogo-layout B, franquia-layout B, versoes-layout A).
- Testes: unitários para as funções puras (ano/tipo, rótulo de versão, filtros, matcher Jikan); conferência visual rodando o app.

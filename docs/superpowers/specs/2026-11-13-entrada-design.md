# Entrada nova: destaques do ano, continuar e descobrir

## Por quê

A entrada atual (`crates/higanbana-desktop/src/views/home.rs`) tem saudação, grade de
seções e a grade da temporada atual do AniList. Ficou vaga e a temporada atual é
inútil: o Anitsu só recebe uma temporada na temporada seguinte, então quase nada ali
está no acervo.

Quem abre o app, se está vendo algo, quer retomar. A entrada passa a ser:
retomar rápido + vitrine do que **está no acervo**.

Junto vem uma correção: o app diz que a 3ª temporada de um anime está no acervo só
porque as outras temporadas estão.

## Objetivos

1. Carrossel de banner largo no topo com 6 obras do ano atual, alternando Anime/Mangá.
2. "Continuar": destaque do último item + fileira (assistindo e lendo).
3. Seções da nuvem compactas.
4. Carrossel "Descobrir" com abas: Mais bem avaliados do ano, Mais populares, Em alta.
5. Toda obra mostrada nos carrosséis está **confirmada no acervo**; a que não está é
   pulada e entra a próxima candidata.
6. Conferência de acervo precisa para sequências (corrige o falso positivo).

## Não-objetivos

- Temporada atual na entrada (removida).
- Histórico completo / tela de histórico.
- Recomendações personalizadas.
- Formatos de pasta peculiares (fica para depois, como novas regras da busca única).
- Traduzir sinopse (fica no idioma que o AniList devolve, igual à página do título).

## Anti-objetivos (falha mesmo que "funcione")

- Varrer pastas da nuvem em lote. A conta já foi bloqueada por HTTP 429. Toda listagem
  passa pelo limitador global em prioridade de segundo plano, uma letra/pasta por vez,
  só quando necessária, com cache.
- Mostrar obra que não abre ("ainda não está na nuvem") nos carrosséis.
- Entrada que trava ou pula de layout enquanto confere: reservar a altura do banner e
  da fileira com skeleton.

## Abordagens consideradas (bloco Continuar)

- **A. Destaque + fileira** (escolhida): último item grande com "Continuar", demais
  títulos em fileira de capas com barra de progresso. Retomar = um clique.
- B. Só fileiras: mais simples, mas o último item não ganha destaque.
- C. Histórico em lista por dia: informativo, mas sem capas, cara de log.

Carrossel do topo: escolhido **banner largo um por vez** (setas + passa sozinho) em vez
de 6 capas lado a lado, para ser apresentativo e não repetir o formato das fileiras.

## Layout

```
┌ Destaques de 2026                          [Anime | Mangá]            ┐
│ ┌──────────────────────────────────────────────────────────────────┐ │
│ │ bannerImage (gradiente p/ legibilidade)          ‹            ›  │ │
│ │ [capa]  Título                                                   │ │
│ │         ★ 87%  · TV · 24 eps · 2026 · Estúdio                    │ │
│ │         [Ação] [Fantasia] [tag] [tag]                            │ │
│ │         Sinopse em 3 linhas…                                     │ │
│ │         [▶ Abrir]                                     • • ● • • •│ │
│ └──────────────────────────────────────────────────────────────────┘ │
Continuar
  [destaque: capa · Título · Ep 7 · faltam 12 min · ▶ Continuar]
  Assistindo  [capa▬][capa▬][capa▬] →
  Lendo       [capa▬][capa▬] →
Seções   [Animes][Mangás][Músicas]…   (compactas)
Descobrir   [Mais bem avaliados | Mais populares | Em alta]
  ‹ [capa][capa][capa][capa][capa][capa] ›
```

Visual segue as Design Guides do GPUI Kit (skill `gpui-kit-design-guides`) e os
componentes/estilo já usados no app (`components::poster`, `section_title`,
`hover_layer`, `Skeleton`).

### Carrossel do topo (banner)

- Fonte: obras do ano atual, `sort: POPULARITY_DESC`, `isAdult: false`.
  - Anime: `seasonYear: <ano>`.
  - Mangá: `startDate_greater: <ano>0000`, `startDate_lesser: <ano+1>0000`.
- Candidatos: até 50 por consulta; conferidos **em ordem** até achar 6 no acervo.
  Se acabar com menos de 6, busca o ano anterior e continua.
- Alterna Anime/Mangá por um controle segmentado no cabeçalho; cada tipo carrega e
  confere só quando escolhido pela primeira vez. Padrão: Anime.
- Slide: `bannerImage` de fundo (sem banner → capa grande desfocada/cor `coverImage.color`),
  capa à esquerda, título, linha de metadados (nota, formato, episódios/capítulos, ano,
  estúdio principal), gêneros + até 3 tags sem spoiler, sinopse limpa
  (`text::clean_description`) em 3 linhas, botão "Abrir" → caminho do acervo.
- Passa sozinho a cada ~8 s; pausa com o mouse em cima; setas e pontos para navegar.
  Sem animação forte (respeitar `motion.rs`).
- Enquanto o primeiro slide não está confirmado: skeleton com a altura final.
- Fim da fila com menos de 6: mostra as achadas. Nenhuma (ou AniList fora): o bloco
  some. Nas abas de Descobrir, nenhuma → estado vazio na aba.

### Continuar

- Fonte local: `PlayerData.progress` (chave = caminho do arquivo, `updated_at`,
  `watched`) e progresso do leitor (`reader::progress_map`). Nenhuma chamada à nuvem
  para montar.
- Agrupar por obra: `library::classify(path)` → `Place::Title { title_path, rest, .. }`;
  em franquia (primeiro segmento de `rest` datado, `is_dated`) a chave é
  `title_path/<subpasta da obra>`, então cada obra da franquia vira um cartão; fica o
  item mais recente de cada obra.
- Capa/título: `AppState::lookup(nome da pasta da obra, media)` (mesmo caminho das
  prateleiras, com cache).
- Destaque = item mais recente. Texto: "Ep N · faltam X min" (não visto) ou
  "Próximo: Ep N+1" (visto).
  - Clique em não visto → lista a pasta do episódio (uma listagem, só no clique, para
    montar `PlayTarget.files` e manter o próximo automático) e toca na posição salva.
  - Clique em visto → abre a obra/pasta do episódio; o próximo é resolvido lá.
- Fileira: demais obras (até ~12), capa com barra de progresso.
- "Lendo": mesma lógica com o progresso do leitor; clique abre o leitor no capítulo e
  página salvos; some se vazio.
- Sem histórico: bloco some; Seções sobem e ganham uma linha de dica.

### Seções compactas

As mesmas seções de hoje (lista da raiz), em chips/cartões mais baixos.

### Descobrir

- Abas (anime): "Mais bem avaliados" (`SCORE_DESC`, ano atual), "Mais populares"
  (`POPULARITY_DESC`, ano atual), "Em alta" (`TRENDING_DESC`, `status: FINISHED`).
- Cada aba confere só quando aberta pela primeira vez; para em 12 confirmadas.
- Fileira horizontal de `poster` com setas (carrossel), mesma pulagem de não-acervo.
- Segue o mesmo seletor Anime/Mangá do topo? **Não**: tem seu próprio seletor
  Anime/Mangá, padrão Anime (decisão minha; muda pouco o código).

## Núcleo (higanbana-core)

### Consulta de ranking

Nova query em `anilist/queries.rs` e método em `anilist/service.rs`:

```rust
pub enum Ranking { Popular, TopRated, Trending }
pub async fn ranking(&self, media: MediaType, ranking: Ranking, year: Option<i32>) -> AniListResult<Value>
```

- Usa `SUMMARY_FRAGMENT` + campos extras só desta query: `description(asHtml: false)`,
  `tags { name rank isMediaSpoiler }`, `studios(isMain: true) { nodes { name } }`.
  Os extras ficam num tipo próprio (`RankedMedia { summary, description, tags, studio }`)
  para não pesar o fragmento usado nas buscas.
- Cache no `details` com chave `ranking:<tipo>:<ranking>:<ano>`, validade de 12 h
  (rankings mudam devagar).

### Conferência de acervo (Catalog)

**Bug atual** (`catalog.rs`, fim de `resolve`): quando só bate o nome base e a
confirmação no AniList falha, devolve a pasta base mesmo assim
(`near.into_iter().next()`). Resultado: Temporada 3 "no acervo" porque T1/T2 existem.

**Correção**:

1. Nome exato → caminho (como hoje).
2. Nome base bate e o AniList confirma que a pasta base **é** essa obra → caminho
   (como hoje).
3. Senão, para cada pasta base candidata: listar a pasta (1 listagem, limitador global,
   segundo plano). Se for franquia (`library::is_franchise`), analisar subpastas com
   `parse_work_name`:
   - título da subpasta normalizado igual a um título da obra → caminho da subpasta;
   - ano igual a `seasonYear`/`startDate.year` **e** formato compatível
     (`WorkKind::from_anilist`) → confirmar no AniList (lote, cache) que o nome da
     subpasta resolve para o mesmo id → caminho da subpasta.
4. Nada bateu → `None` (não está no acervo).

- Versão no arquivo do cache (`Stored.v`); versão diferente descarta o cache antigo,
  apagando os falsos positivos já gravados.
- Falha de rede continua não sendo gravada.

### Busca única para anime e mangá

A seção de mangá (`Mangás/Séries Mangás`) tem a mesma organização da de animes
(letras `0-9` / `Letra X`, pasta de obra, franquias com subpastas datadas). Decisão do
usuário: **um caminho só** de conferência para os dois tipos, para que um formato de
pasta peculiar resolvido em animes passe a valer em mangás sem código novo.

- Um índice só (`AnimeIndex` vira índice de seção) recebe a raiz da seção
  (`library::SECTIONS`: raiz + profundidade) e lista **uma letra por vez, só quando
  pedida**, com o mesmo limitador, pausa e cache de 12 h.
- Um `resolve` só no `Catalog`, que recebe o `MediaType` e não tem nenhum ramo
  específico de anime ou mangá. Chave do cache: `(MediaType, id)`.
- Os formatos de pasta reconhecidos dentro da pasta base ficam numa lista de regras
  (`trait`/funções `fn(&[FileEntry], &MediaSummary) -> Match`), consultadas em ordem.
  Hoje só existe a regra "franquia com subpastas datadas" (`is_franchise` +
  `parse_work_name`). Formatos peculiares (subpastas sem data, várias temporadas numa
  release, sequência em pasta própria na letra) entram depois como novas regras e
  valem para os dois tipos.
- Formatos não reconhecidos → `None` (aceito: preferimos falso negativo a falso
  positivo).

### Orçamento de nuvem

- No máximo 20 listagens da nuvem por visita à entrada (somando índice de letra e
  pastas base); ao estourar, candidatos não conferidos são pulados.
- A fila pausa enquanto há vídeo tocando (mesma pausa do índice).
- Listagens de pasta base ficam em disco por 12 h, como as letras.
- O cache de ranking usa um `DiskCache` próprio com validade de 12 h.

### Fila "até N confirmadas"

Função no núcleo que recebe a lista de candidatos e confere em ordem, com no máximo 2
em paralelo (semáforo existente), parando ao chegar em N. Emite cada confirmada assim
que sai, para a interface ir preenchendo. Cancelável (trocar aba/tipo para a fila
anterior).

## Interface (higanbana-desktop)

- `views/home.rs` vira a composição; blocos em arquivos próprios
  (`views/home/hero.rs`, `continue_watching.rs`, `discover.rs`) se passar de ~300 linhas.
- Remove `season`/`open_season`/`season_label` da entrada (`season()` do núcleo pode
  ficar se usado em outro lugar; senão sai).
- Clique em obra confirmada: `state.go(path)` direto (caminho já conhecido).

## Testes

- `catalog`: sequência sem subpasta → `None`; sequência com subpasta datada → caminho
  da subpasta; nome exato e confirmação pela pasta base continuam funcionando.
  Usar listagens falsas (sem rede).
- Agrupamento do Continuar: vários episódios da mesma obra → um item, o mais recente;
  ordem por `updated_at`.
- Fila até N: pula não-acervo, para em N, completa com o ano anterior.
- Montagem das queries de ranking (variáveis por tipo/ranking/ano).

## Deixado para quem constrói

- Medidas exatas, tipografia e cores dentro das Design Guides.
- Intervalo exato do auto-avanço e da transição.
- Quantos itens a fileira Continuar mostra antes de rolar.

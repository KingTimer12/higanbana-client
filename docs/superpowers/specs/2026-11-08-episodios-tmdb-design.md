# Episódios com TMDB — design

Branch: `feat/catalogo-animes` (a pedido da pessoa, tudo nesta branch).

## Intenção

Hoje a lista de episódios da página de uma obra mostra o nome do arquivo
(`[Grupo] Anime - 05 [1080p].mkv`) ou números soltos. A pessoa quer uma lista
como a da Crunchyroll/Amazon: miniatura do episódio, título de verdade, nota e
duração. O TMDB tem esses dados por episódio; o AniList não.

## Objetivos

1. Na aba "Episódios" da página de uma obra (anime), cada arquivo de vídeo que
   casar com um episódio do TMDB mostra: miniatura (still), "E{n}", título,
   nota ★ (se houver votos) e duração.
2. Dois layouts, alternados por um botão de ícone (grade ↔ lista) ao lado do
   cabeçalho da aba Episódios; a escolha é global e persistida:
   - **A — grade de cards (padrão):** miniatura 16:9 com duração no canto e
     barra de progresso vermelha embaixo; ▶ ao passar o mouse; embaixo "E1",
     título (até 2 linhas), nota.
   - **B — lista:** uma linha por episódio: miniatura grande à esquerda;
     "E1 · data de exibição", título, nota, sinopse curta (2–3 linhas).
   Referência visual aprovada: `.superpowers/brainstorm/107-1794158077/content/episodios-layout.html`
   (opções A e B).
3. Uma abstração (trait) de "fonte de notas" com duas implementações, AniList e
   TMDB, para que no futuro a pessoa escolha nas configurações de onde vem a
   nota da obra. Agora: fixa em AniList (comportamento atual). Nota por
   episódio vem sempre do TMDB (o AniList não tem).
4. Usar a crate `tmdb_client` (bcourtine/tmdb-client-rs, 1.8.0, reqwest 0.12 —
   mesma versão do workspace), sugerida pela pessoa. Se ela não cobrir algum
   endpoint necessário ou trouxer problema concreto (ex.: dependências pesadas,
   API bloqueante), o builder pode chamar a API REST direto com o `reqwest` do
   projeto e explicar o motivo no relatório.

## Não-objetivos

- Trocar o AniList como fonte principal da obra (capa, sinopse, gêneros,
  casamento de nomes continuam no AniList + fallback MAL).
- Tela de configurações para escolher a fonte de notas (só a trait agora).
- Notas do IMDb (o TMDB não fornece; só o id).
- Abas de temporada dentro da página de uma obra: cada pasta de temporada já é
  uma obra separada na página da franquia.
- Mangás.

## Anti-objetivos (seria falha mesmo funcionando)

- Lista de episódios que quebra ou some quando o TMDB não acha, está fora do ar
  ou não tem token: tem que cair no visual de hoje.
- Episódio ligado ao arquivo errado (título/miniatura de outro episódio).
  Na dúvida, não ligar.
- Voltar a estourar limite de API (ver trabalho recente de ritmo no AniList).
- Token em texto puro no repositório.

## Restrições

- Rust; crates `higanbana-core` (rede, cache, lógica) e `higanbana-desktop`
  (UI GPUI + gpui-kit; seguir as skills `gpui-kit` e `gpui-kit-design-guides`).
- Credenciais: `.env` (fora do git) tem `TMDB_TOKEN` (v4 read access token) e
  `TMDB_KEY` (v3). Embutir no binário mascarado com XOR, como
  `crates/higanbana-core/build.rs` já faz com `ANITSU_SUPABASE_ANON_KEY`.
  Diferente da chave da Anitsu, a falta do token do TMDB **não** deve quebrar o
  build: sem token, o recurso fica desligado. Atualizar `.env.example`.
  Usar o token (Bearer) de preferência.
- User-Agent genérico (`higanbana-client/<versão>`), nunca e-mail da pessoa.
- Cache em disco como os do AniList (`anilist/cache.rs` / `DiskCache`, em
  `%LOCALAPPDATA%/<id>`). Sugestão: casamento obra→(série TMDB, temporada) 7 dias;
  episódios da temporada 24 h.
- O TMDB só é consultado quando a página da obra é aberta (nada de varrer o
  catálogo). Miniaturas carregam quando ficam visíveis, pelo mesmo caminho das
  capas (cache de imagens existente). Tamanho de still sugerido: `w300` na grade,
  `w500`/`w300` na lista.
- Limite do TMDB é folgado (~40–50 req/s), mas aplicar um ritmo simples
  (ex.: uma por vez, intervalo curto, respeitar 429/Retry-After).
- NUNCA rodar `cargo fmt` (arquivos CRLF). Commits em português, um por
  mudança, sem trailer de atribuição.
- No final, atualizar o `README.md` (pedido da pessoa): como configurar
  `TMDB_TOKEN`/`TMDB_KEY` no `.env`, o que o recurso faz e que sem token ele
  fica desligado.

## Como ligar obra e arquivos ao TMDB (decidido)

Dados medidos com o token real:
- Busca `/search/tv` acha bem nomes no formato MAL ("HappinessCharge Precure!",
  "Sousou no Frieren").
- Frieren e One Piece: 100% dos episódios com still, título pt-BR e nota.
  Happiness Charge Precure: still em todos, mas sem título pt-BR ("Episódio 1")
  e sem notas; títulos existem em en-US.
- One Piece no TMDB é dividido em temporadas por arco (S1 East Blue 61 eps,
  S2 16, …); arquivos costumam vir numerados de forma absoluta.

Regras:
1. **Obra → série TMDB:** buscar `/search/tv` com o título da obra (do AniList:
   romaji/inglês/nativo, e o nome da pasta) e escolher pelo ano de estreia
   (ano da obra no AniList / pasta). Filmes (`format` MOVIE) ficam fora deste
   trabalho. Partes/temporadas posteriores (ano da obra depois da estreia da
   série) devem casar com a série-mãe: aceitar série cuja estreia é anterior
   ao ano da obra quando o título bate.
2. **Obra → temporada TMDB:** escolher a temporada cuja `air_date` bate com o
   início da obra no AniList (ano/mês). Se a série tem uma única temporada
   regular, usar ela.
3. **Arquivo → episódio:** número via `player::logic::episode_number` (já
   existe). Se o número maior que o total da temporada escolhida, e a série tem
   várias temporadas, tratar como numeração absoluta e converter somando os
   `episode_count` das temporadas regulares em ordem (caso One Piece).
4. Sem número, número fora de qualquer temporada, ou dúvida → sem dados do
   TMDB para aquele arquivo (visual de hoje). Nunca chutar.
5. Título do episódio: pt-BR; se vier vazio ou genérico ("Episódio N"), usar
   en-US; senão "Episódio N". Sinopse com a mesma regra.

6. **Obra que começa no meio de uma temporada do TMDB** (split cour: AniList
   "X Part 2" começa 2022-10, TMDB tem S1 com 24 eps desde 2022-04) —
   *decisão do Claude*: buscar, nas temporadas regulares, o episódio cuja
   `air_date` bate com o início da obra no AniList (tolerância de ~7 dias) e
   usar o deslocamento a partir dele. Arquivo "01" → esse episódio; arquivos
   numerados acima do deslocamento (ex.: 13–24) e que caibam na temporada são
   tratados como numeração da própria temporada. A regra "temporada única"
   só vale quando a data de início da obra bate com o começo da temporada; se
   não bater e nenhum episódio bater, não liga nada.
7. **Formatos** — ONA e TV_SHORT: tratados como TV. OVA e SPECIAL (*decisão da
   pessoa: tentar*): casar com a temporada 0 ("Especiais") da série; liga um
   arquivo só quando a ordem e a `air_date` do episódio da temporada 0 batem
   com a obra (início no AniList, tolerância ~7 dias para o primeiro; os
   seguintes na ordem). Se não bater, visual de hoje. A pessoa aceita ajustar
   com o tempo. MOVIE fica fora.
8. **Temporada no nome do arquivo** (`S02E03`) — *decisão do Claude*: se o
   arquivo traz temporada explícita e ela existe no TMDB, ela vence a
   temporada escolhida pela data; senão vale a regra normal. Requer extrair a
   temporada além do número (função nova ou estendida em `player::logic`).

## Tela quando o casamento é parcial ou não existe (decisão do Claude)

- Todos os vídeos viram cards (A) / linhas (B), na ordem natural de hoje.
  Vídeo sem dados do TMDB: miniatura neutra (gradiente/ícone do tema), título
  = "Episódio N" quando há número, senão o nome do arquivo; sem nota.
- Subpastas e arquivos que não são vídeo (legendas, .txt) aparecem abaixo, na
  lista compacta de hoje (`components::file_list`).
- O botão de layout aparece sempre que há vídeos, com ou sem TMDB.
- Sem token, TMDB fora do ar ou obra não casada: mesmos cards com miniatura
  neutra (a tela não depende do TMDB para existir).
- Downloads, progresso, "Continuar/Assistir" e menus de arquivo que existem hoje
  na lista continuam disponíveis nos cards/linhas.

## Fonte de notas (decidido)

- Trait no core, ex.: `RatingSource` com nota da obra (0–10 ou 0–100
  normalizada — escolha do builder, documentada) e, opcionalmente, nota por
  episódio. Implementações: AniList (`averageScore`) e TMDB (`vote_average`,
  só quando `vote_count > 0`).
- Preferência guardada (padrão AniList) num lugar que uma futura tela de
  configurações possa mudar; nenhuma UI de escolha agora.
- Episódio sem votos não mostra estrela.

## Deixado para o builder

- Estrutura de módulos (ex.: `crates/higanbana-core/src/tmdb/`), nomes, formato
  do cache, como expor ao desktop (seguir o padrão de `AppState::want_lookup` /
  pedidos sob demanda).
- Onde exatamente fica o botão de layout no cabeçalho da aba e o ícone.
- Estados de carregamento (esqueleto dos cards enquanto o TMDB responde).
- Onde guardar a preferência de layout e de fonte de notas (`prefs.rs` existe e
  só guarda versões por obra hoje).
- Testes: funções puras (escolha de série/temporada, conversão absoluta,
  escolha de título) com testes unitários; teste ao vivo marcado `#[ignore]`.

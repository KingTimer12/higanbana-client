# Limite das APIs (AniList/MAL) — plano

## Diagnóstico (medido em 2026-11-08)

- AniList em modo degradado: `X-RateLimit-Limit: 30` (30 req/min). Com o app aberto: `429, Retry-After: 11`.
- Índice: 4837 nomes; cache de matches: 651 (11 nulos, 5 só-MAL). MAL quase não é usada; o gargalo é o AniList.
- Rastreio de metadados (`catalog_state.rs`) roda sempre que o catálogo abre: 40 nomes a cada 3 s = 4 req/3 s ≈ 80 req/min, 2,7× o limite.
- `AniListClient` (`anilist/client.rs`) não tem ritmo próprio: só reage ao 429. Pedidos dos cards visíveis, franquia, detalhe e rastreio saem em paralelo.
- Lote que falha (`flush_lookups`, ramo `None`) libera os nomes na hora; os cards visíveis pedem de novo no próximo frame → laço quente.
- Complexidade do AniList: 27 pontos por alias de busca, teto 500 → cabem 18 aliases por requisição (testado: 18 OK, 20 = 540 recusado). Hoje usamos 10.

## Regras globais

- NUNCA rodar `cargo fmt` (arquivos CRLF; formatar o crate inteiro estraga o diff).
- Commits sem trailer `Co-Authored-By` nem qualquer linha de atribuição.
- Comentários e mensagens em português, no estilo do código existente.
- Testes: `cargo test -p higanbana-core` e `cargo test -p higanbana-desktop` verdes; build sem warnings novos.

## Tarefa 1 — Ritmo próprio no cliente do AniList (core)

Arquivo: `crates/higanbana-core/src/anilist/client.rs` (+ reexport em `anilist/mod.rs` se preciso).

- `pub enum Priority { Interactive, Background }`.
- Estado de limite (std `Mutex`): `limit` (padrão 30), `remaining: Option<u32>`, `reset_at: Option<Instant>`, `blocked_until: Option<Instant>`, `last_sent: Option<Instant>`.
- Função pura testável `fn send_at(state, now, priority) -> Instant` (quando pode sair):
  - nunca antes de `blocked_until`;
  - intervalo mínimo desde `last_sent`: Interactive 700 ms; Background `60 s / limit` (2 s com limite 30);
  - se `remaining` conhecido e `remaining <= reserva` e `reset_at > now` → espera `reset_at`. Reserva: Interactive 1; Background `limit / 3` (10 com 30). Background deixa folga para o que a pessoa está vendo.
- Uma requisição por vez: `tokio::sync::Mutex<()>` segurado da espera até a resposta. Background cede a vez: se há Interactive esperando (contador atômico), solta a trava e espera ~200 ms antes de tentar de novo.
- Depois de cada resposta: ler `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset` (epoch em segundos → `Instant`). Em 429: `blocked_until = agora + Retry-After + 1 s`, `remaining = 0`.
- API: `query(q, vars)` continua (Interactive); novo `query_with(priority, q, vars)`; `pub fn paused_until(&self) -> Option<Instant>` (`blocked_until` no futuro).
- Testes da função pura: espera o bloqueio; intervalo por prioridade; reserva segura Background mas não Interactive; leitura dos cabeçalhos (função pura de parse).

## Tarefa 2 — Lotes maiores e prioridade no serviço (core)

Arquivo: `crates/higanbana-core/src/anilist/service.rs`.

- `BATCH_SIZE = 18` com teste que trava a conta: `18 * 27 <= 500` (constante `ALIAS_COMPLEXITY = 27`, `MAX_COMPLEXITY = 500`).
- `lookup_with(items, Priority)`; `lookup(items)` = Interactive. A prioridade vai para `search_batch` e para a query de `idMal` do fallback MAL.
- `AniList::paused_until()` repassa o do cliente.
- `media`, `season` e detalhe continuam Interactive.

## Tarefa 3 — Rastreio sob demanda, fila com prioridade e pausa após falha (desktop)

Arquivos: `crates/higanbana-desktop/src/state.rs`, `catalog_state.rs`, `views/catalog.rs`, `views/catalog_filters.rs`.

- Rastreio só trabalha quando os metadados são necessários: filtros com `needs_metadata()` ou ordenação diferente de `Sort::Az` (escolha guardada em `cat.view`). Sem isso, `crawl_step` fica ocioso (não termina, não pede nada). Busca por nome e rolagem pedem só os cards visíveis.
- Rastreio manda lotes de 18 nomes (uma requisição) como `Priority::Background`, um lote por vez (já espera `lookups_busy`).
- Fila de buscas com duas faixas: cards visíveis/título/franquia/busca (Interactive) saem primeiro; o rastreio (Background) só sai quando a faixa Interactive está vazia. Lote Interactive até 36 nomes.
- Falha de lote (`None`): define `lookups_cooldown_until = max(agora + 30 s, anilist.paused_until())`. Enquanto vale, `flush_lookups` não manda nada (reagenda para o fim da pausa) e `lookups_busy()` é verdadeiro. Acaba o laço quente.
- Aviso no contador de metadados do catálogo quando o AniList pediu pausa: "AniList pediu uma pausa — continua em N s" (redesenho ao fim da pausa, como o aviso da nuvem).
- Testes das partes puras (decisão de quando o rastreio trabalha; escolha de faixa).

//! AniList em lote: nomes pedidos pelas telas juntam numa requisição, com duas
//! faixas (o que a pessoa está vendo primeiro; o rastreio do catálogo depois),
//! pausa depois de falha/429 e novas tentativas para respostas adiadas.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use higanbana_core::anilist::{types::MediaSummary, Lookup, LookupRequest, MediaType, Priority};
use rustc_hash::FxHashMap;

use super::AppState;

const LOOKUP_DELAY: Duration = Duration::from_millis(60);
/// Nomes por lote do que a pessoa está vendo (duas requisições de 18 no AniList).
const LOOKUP_MAX: usize = 36;
/// Nomes por lote do rastreio (uma requisição).
const LOOKUP_MAX_BACKGROUND: usize = 18;
/// Espera antes de repetir buscas adiadas (fallback MAL com limite de requisições); dobra a cada vez sem progresso.
const LOOKUP_RETRY: Duration = Duration::from_secs(3);
/// Tentativas seguidas sem progresso antes de desistir de um nome adiado.
const LOOKUP_RETRY_MAX: u32 = 5;
/// Pausa mínima depois de um lote que falhou (rede ou limite do AniList).
const LOOKUP_COOLDOWN: Duration = Duration::from_secs(30);

/// `None` = ainda buscando; `Some(None)` = sem correspondência.
type Answer = Option<Option<Arc<MediaSummary>>>;
type Key = (MediaType, String);

/// Respostas por tipo de mídia (consulta por `&str` sem montar chave a cada card).
#[derive(Default)]
pub(super) struct Lookups {
    anime: FxHashMap<String, Answer>,
    manga: FxHashMap<String, Answer>,
    /// Muda a cada lote de respostas.
    generation: u64,
    /// Fila do que a pessoa está vendo (cards, título, busca).
    queue: Vec<Key>,
    /// Fila do rastreio: só sai com `queue` vazia.
    background: Vec<Key>,
    /// Próxima saída agendada.
    flush: Option<tokio::task::JoinHandle<()>>,
    due: Option<Instant>,
    inflight: usize,
    /// Um lote da tela falhou: nada sai até aqui.
    cooldown_until: Option<Instant>,
    /// Um lote do rastreio falhou: só a faixa dele espera.
    background_cooldown_until: Option<Instant>,
    /// Nomes adiados (`Pending`): tentativas seguidas sem progresso.
    retries: HashMap<Key, u32>,
}

impl Lookups {
    fn of(&self, media: MediaType) -> &FxHashMap<String, Answer> {
        match media {
            MediaType::Anime => &self.anime,
            MediaType::Manga => &self.manga,
        }
    }

    fn of_mut(&mut self, media: MediaType) -> &mut FxHashMap<String, Answer> {
        match media {
            MediaType::Anime => &mut self.anime,
            MediaType::Manga => &mut self.manga,
        }
    }

    fn insert(&mut self, (media, name): Key, answer: Answer) {
        self.of_mut(media).insert(name, answer);
    }

    fn remove(&mut self, (media, name): &Key) {
        self.of_mut(*media).remove(name);
    }

    fn lane(&mut self, p: Priority) -> &mut Vec<Key> {
        match p {
            Priority::Interactive => &mut self.queue,
            Priority::Background => &mut self.background,
        }
    }
}

/// Qual faixa sai agora e quantos nomes: o que a pessoa está vendo primeiro.
pub(crate) fn next_lane(interactive: usize, background: usize) -> Option<(Priority, usize)> {
    if interactive > 0 {
        Some((Priority::Interactive, interactive.min(LOOKUP_MAX)))
    } else if background > 0 {
        Some((Priority::Background, background.min(LOOKUP_MAX_BACKGROUND)))
    } else {
        None
    }
}

/// Fim da pausa depois de um lote que falhou: a nossa espera mínima ou a pedida pelo AniList, a mais longa.
pub(crate) fn cooldown_end(now: Instant, anilist_paused: Option<Instant>) -> Instant {
    (now + LOOKUP_COOLDOWN).max(anilist_paused.unwrap_or(now))
}

/// Nome adiado: `(tentativas sem progresso, espera)` da próxima vez, ou `None` para desistir.
pub(crate) fn next_retry(strikes: Option<u32>, progress: bool) -> Option<(u32, Duration)> {
    let n = if progress { 0 } else { strikes.map_or(0, |n| n + 1) };
    (n < LOOKUP_RETRY_MAX).then(|| (n, LOOKUP_RETRY * 2u32.pow(n)))
}

/// Até quando uma faixa espera: a pausa da tela e o 429 valem para as duas; a do rastreio só para ele.
pub(crate) fn lane_paused_until(priority: Priority, now: Instant, global: Option<Instant>, background: Option<Instant>, client: Option<Instant>) -> Option<Instant> {
    let own = match priority {
        Priority::Interactive => None,
        Priority::Background => background,
    };
    [global, own, client].into_iter().flatten().filter(|t| *t > now).max()
}

impl AppState {
    /// `None` = ainda buscando; `Some(None)` = sem correspondência.
    pub fn lookup(&self, name: &str, media: MediaType) -> Answer {
        self.lookups.of(media).get(name).cloned().flatten()
    }

    /// Como `lookup`, sem copiar (para varrer o índice inteiro).
    pub fn lookup_ref(&self, name: &str, media: MediaType) -> Option<Option<&MediaSummary>> {
        self.lookups.of(media).get(name).and_then(Option::as_ref).map(Option::as_deref)
    }

    /// Pausa da faixa do rastreio (falha de lote ou 429 do AniList).
    pub(crate) fn anilist_paused_until(&self) -> Option<Instant> {
        self.lane_paused(Priority::Background)
    }

    /// Há buscas na fila, prestes a sair, esperando resposta ou em pausa (o rastreio espera).
    pub(crate) fn lookups_busy(&self) -> bool {
        let l = &self.lookups;
        !l.queue.is_empty() || !l.background.is_empty() || l.flush.is_some() || l.inflight > 0 || self.anilist_paused_until().is_some()
    }

    /// Muda sempre que algum nome passa a ter resposta.
    pub fn lookups_gen(&self) -> u64 {
        self.lookups.generation
    }

    fn lane_paused(&self, p: Priority) -> Option<Instant> {
        let l = &self.lookups;
        lane_paused_until(p, Instant::now(), l.cooldown_until, l.background_cooldown_until, self.core.anilist().paused_until())
    }

    /// Pede a busca do que a pessoa está vendo. `false` se já estava pedido ou respondido.
    pub fn want_lookup(&mut self, name: &str, media: MediaType) -> bool {
        self.want_lookup_with(name, media, Priority::Interactive)
    }

    pub(crate) fn want_lookup_with(&mut self, name: &str, media: MediaType, priority: Priority) -> bool {
        if self.lookups.of(media).contains_key(name) {
            // Esperava no rastreio e agora a pessoa está vendo: passa para a frente.
            if priority == Priority::Interactive
                && let Some(pos) = self.lookups.background.iter().position(|k| k.0 == media && k.1 == name)
            {
                let key = self.lookups.background.remove(pos);
                self.lookups.queue.push(key);
                self.schedule_flush(LOOKUP_DELAY);
            }
            return false;
        }
        let key = (media, name.to_owned());
        self.lookups.insert(key.clone(), None);
        self.lookups.lane(priority).push(key);
        self.schedule_flush(LOOKUP_DELAY);
        true
    }

    /// Agenda a próxima saída; uma já agendada só é trocada se esta sair antes.
    fn schedule_flush(&mut self, after: Duration) {
        let due = Instant::now() + after;
        if self.lookups.flush.is_some() && self.lookups.due.is_some_and(|d| d <= due) {
            return;
        }
        if let Some(old) = self.lookups.flush.take() {
            old.abort();
        }
        self.lookups.due = Some(due);
        self.lookups.flush = Some(self.after(after, |s, _| s.flush_lookups()));
    }

    fn flush_lookups(&mut self) {
        self.lookups.flush = None;
        self.lookups.due = None;
        let Some((priority, n)) = next_lane(self.lookups.queue.len(), self.lookups.background.len()) else { return };
        if let Some(until) = self.lane_paused(priority) {
            self.schedule_flush(until.saturating_duration_since(Instant::now()));
            return;
        }
        let batch: Vec<Key> = self.lookups.lane(priority).drain(..n).collect();
        if !self.lookups.queue.is_empty() || !self.lookups.background.is_empty() {
            self.schedule_flush(LOOKUP_DELAY);
        }
        let core = self.core.clone();
        let items: Vec<LookupRequest> = batch.iter().map(|(m, n)| LookupRequest { name: n.clone(), media_type: *m }).collect();
        self.lookups.inflight += 1;
        self.run(async move { core.anilist().lookup_with(&items, priority).await.ok() }, move |s, result, _| {
            s.lookups.inflight = s.lookups.inflight.saturating_sub(1);
            match result {
                Some(list) => s.lookups_answered(batch, list, priority),
                None => s.lookups_failed(batch, priority),
            }
        });
    }

    fn lookups_answered(&mut self, batch: Vec<Key>, list: Vec<Lookup>, priority: Priority) {
        let l = &mut self.lookups;
        l.generation += 1;
        let mut pending = Vec::new();
        let mut settled = Vec::new();
        // Progresso: algum nome que já tinha voltado adiado agora tem resposta.
        let mut progress = false;
        for (key, v) in batch.into_iter().zip(list) {
            if !matches!(v, Lookup::Pending) {
                progress |= l.retries.remove(&key).is_some();
            }
            if !matches!(v, Lookup::Pending) {
                settled.push(key.clone());
            }
            match v {
                Lookup::Found(v) => l.insert(key, Some(serde_json::from_value::<MediaSummary>(v).ok().map(Arc::new))),
                Lookup::NotFound => l.insert(key, Some(None)),
                Lookup::Pending => pending.push(key),
            }
        }
        let mut later: HashMap<Duration, Vec<Key>> = HashMap::new();
        for key in pending {
            match next_retry(l.retries.get(&key).copied(), progress) {
                Some((n, wait)) => {
                    l.retries.insert(key.clone(), n);
                    later.entry(wait).or_default().push(key);
                }
                None => {
                    l.retries.remove(&key);
                    l.remove(&key);
                    // O rastreio não pede de novo os que desistiu.
                    settled.push(key);
                }
            }
        }
        self.crawl_lookups_done(&settled, false);
        for (wait, retry) in later {
            self.after(wait, move |s, _| {
                s.lookups.lane(priority).splice(0..0, retry);
                s.schedule_flush(LOOKUP_DELAY);
            });
        }
    }

    /// Falha de rede/limite: libera os nomes e pausa a faixa.
    fn lookups_failed(&mut self, batch: Vec<Key>, priority: Priority) {
        let end = Some(cooldown_end(Instant::now(), self.core.anilist().paused_until()));
        match priority {
            Priority::Interactive => self.lookups.cooldown_until = end,
            Priority::Background => self.lookups.background_cooldown_until = end,
        }
        self.crawl_lookups_done(&batch, true);
        for key in batch {
            self.lookups.retries.remove(&key);
            self.lookups.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactive_lane_goes_first() {
        assert_eq!(next_lane(0, 0), None);
        assert_eq!(next_lane(5, 100), Some((Priority::Interactive, 5)));
        assert_eq!(next_lane(50, 0), Some((Priority::Interactive, 36)));
        assert_eq!(next_lane(0, 7), Some((Priority::Background, 7)));
        assert_eq!(next_lane(0, 100), Some((Priority::Background, 18)));
    }

    #[test]
    fn crawl_failure_pauses_only_its_lane() {
        let now = Instant::now();
        let (i, b) = (Priority::Interactive, Priority::Background);
        let later = |s| Some(now + Duration::from_secs(s));
        assert_eq!(lane_paused_until(i, now, None, None, None), None);
        assert_eq!(lane_paused_until(i, now, None, later(30), None), None);
        assert_eq!(lane_paused_until(b, now, None, later(30), None), later(30));
        assert_eq!(lane_paused_until(i, now, later(30), None, None), later(30));
        assert_eq!(lane_paused_until(b, now, later(30), later(10), None), later(30));
        assert_eq!(lane_paused_until(i, now, Some(now - Duration::from_secs(1)), None, later(12)), later(12));
        assert_eq!(lane_paused_until(b, now, None, Some(now), None), None);
    }

    #[test]
    fn pending_retries_back_off_then_give_up() {
        let s = Duration::from_secs;
        assert_eq!(next_retry(None, false), Some((0, s(3))));
        assert_eq!(next_retry(Some(0), false), Some((1, s(6))));
        assert_eq!(next_retry(Some(1), false), Some((2, s(12))));
        assert_eq!(next_retry(Some(2), false), Some((3, s(24))));
        assert_eq!(next_retry(Some(3), false), Some((4, s(48))));
        assert_eq!(next_retry(Some(4), false), None);
        assert_eq!(next_retry(Some(4), true), Some((0, s(3))));
    }

    #[test]
    fn cooldown_takes_the_longest_pause() {
        let now = Instant::now();
        assert_eq!(cooldown_end(now, None), now + Duration::from_secs(30));
        assert_eq!(cooldown_end(now, Some(now + Duration::from_secs(5))), now + Duration::from_secs(30));
        assert_eq!(cooldown_end(now, Some(now + Duration::from_secs(61))), now + Duration::from_secs(61));
    }
}

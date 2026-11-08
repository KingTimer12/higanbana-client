use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Mutex,
    time::Instant,
};

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::{
    cache::DiskCache,
    client::AniListClient,
    error::{AniListError, AniListResult},
    jikan::{self, JikanClient},
    mal_site::MalSiteClient,
    matcher::{self, ParsedName},
    queries::{self, MediaType},
};

/// Buscas por requisição (aliases). Mantém a complexidade da query baixa.
const BATCH_SIZE: usize = 10;
const CANDIDATES_PER_SEARCH: u32 = 6;
const MATCH_TTL: i64 = 7 * 24 * 3600;
const DETAIL_TTL: i64 = 24 * 3600;
/// Buscas na MAL por chamada de `lookup` (a busca do site vai a ~1 req/s; o resto fica `Pending`).
const MAL_PER_CALL: usize = 4;

#[derive(Debug, Clone, Deserialize)]
pub struct LookupRequest {
    /// Nome da pasta na nuvem, ex.: `[2018] B - The Beginning`.
    pub name: String,
    #[serde(rename = "type")]
    pub media_type: MediaType,
}

/// Arquivo do cache de matches. `-v2`: os `null` da versão anterior (sem fallback MAL) não valem mais.
const MATCHES_FILE: &str = "anilist-matches-v2.json";

/// Resultado de casar uma pasta com uma mídia.
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup {
    Found(Value),
    /// Nem AniList nem MAL acharam (resultado cacheado).
    NotFound,
    /// Sem resposta ainda (erro ou fallback MAL adiado); vale tentar de novo depois.
    Pending,
}

impl Lookup {
    fn from_cached(v: Option<Value>) -> Self {
        v.map_or(Self::NotFound, Self::Found)
    }
}

/// Separa os misses em (rodam agora, adiados) conforme o limite por chamada.
pub(crate) fn split_mal<T>(mut misses: Vec<T>, cap: usize) -> (Vec<T>, Vec<T>) {
    let later = misses.split_off(cap.min(misses.len()));
    (misses, later)
}

/// Para cada busca sem match no AniList: o melhor candidato da MAL (formato AniList) ou None.
pub(crate) fn pick_mal(candidates: &[Value], wanted: &ParsedName) -> Option<Value> {
    matcher::pick_best(candidates, wanted).map(|idx| candidates[idx].clone())
}

/// Junta resultado do AniList-por-idMal com o candidato da MAL: AniList vence; senão MAL.
pub(crate) fn merge_mal(anilist: Option<Value>, jikan: Value) -> Value {
    anilist.unwrap_or(jikan)
}

/// Metadados do AniList com cache em disco.
pub struct AniList {
    client: AniListClient,
    /// Detalhe das obras só-MAL.
    jikan: JikanClient,
    /// Busca da MAL (site), para o fallback quando o AniList não acha.
    mal_site: MalSiteClient,
    /// chave de busca → resumo da mídia casada (ou `null` se nada casou).
    matches: DiskCache<Option<Value>>,
    /// id → detalhe.
    details: DiskCache<Value>,
    /// Chaves sem match no AniList cujo fallback MAL ainda não foi decidido.
    anilist_missed: Mutex<HashSet<String>>,
}

impl AniList {
    pub fn new(dir: &Path) -> Self {
        Self {
            client: AniListClient::new(),
            jikan: JikanClient::new(),
            mal_site: MalSiteClient::new(),
            matches: DiskCache::open(dir.join(MATCHES_FILE), MATCH_TTL),
            details: DiskCache::open(dir.join("anilist-details.json"), DETAIL_TTL),
            anilist_missed: Mutex::new(HashSet::new()),
        }
    }

    /// Fim da pausa pedida pelo AniList (429), se ainda estiver valendo.
    pub fn paused_until(&self) -> Option<Instant> {
        self.client.paused_until()
    }

    /// Casa nomes de pasta com mídias do AniList. Resultado na mesma ordem da entrada.
    /// `Pending`: ainda sem resposta (falha de rede ou fallback MAL adiado); não entra no cache.
    pub async fn lookup(&self, items: &[LookupRequest]) -> AniListResult<Vec<Lookup>> {
        let keyed: Vec<(String, ParsedName, MediaType)> = items
            .iter()
            .map(|it| {
                let parsed = matcher::parse_folder_name(&it.name);
                let key = format!(
                    "{}|{}|{}",
                    it.media_type.as_str(),
                    matcher::normalize(&parsed.title),
                    parsed.year.map(|y| y.to_string()).unwrap_or_default()
                );
                (key, parsed, it.media_type)
            })
            .collect();

        // Buscas que faltam no cache, sem repetição.
        let mut pending: Vec<&(String, ParsedName, MediaType)> = Vec::new();
        for item in &keyed {
            if self.matches.get(&item.0).is_none() && !pending.iter().any(|p| p.0 == item.0) {
                pending.push(item);
            }
        }

        let mut fresh: HashMap<String, Lookup> = HashMap::new();
        let mut last_error = None;
        // Animes sem match no AniList: candidatos a fallback na MAL. Os que já falharam no
        // AniList numa chamada anterior (adiados) pulam direto para a MAL.
        let mut misses: Vec<&(String, ParsedName, MediaType)> = Vec::new();
        let mut to_search: Vec<&(String, ParsedName, MediaType)> = Vec::new();
        {
            let missed = self.anilist_missed.lock().unwrap();
            for item in &pending {
                if missed.contains(&item.0) {
                    misses.push(item);
                } else {
                    to_search.push(item);
                }
            }
        }
        for chunk in to_search.chunks(BATCH_SIZE) {
            match self.search_batch(chunk).await {
                Ok(found) => {
                    for (key, value) in found {
                        let item = chunk.iter().find(|c| c.0 == key).copied();
                        match item {
                            Some(item) if value.is_none() && item.2 == MediaType::Anime => {
                                self.anilist_missed.lock().unwrap().insert(key);
                                misses.push(item);
                            }
                            _ => {
                                self.matches.insert(key.clone(), value.clone());
                                fresh.insert(key, Lookup::from_cached(value));
                            }
                        }
                    }
                }
                Err(e) => last_error = Some(e),
            }
        }

        // A busca da MAL vai a ~1 req/s: só uma parte dos misses por chamada, o resto volta `Pending`.
        let (now, _later) = split_mal(misses, MAL_PER_CALL);
        for (key, value, cache) in self.mal_fallback(&now, &mut last_error).await {
            if cache {
                self.matches.insert(key.clone(), value.clone());
                self.anilist_missed.lock().unwrap().remove(&key);
            }
            fresh.insert(key, Lookup::from_cached(value));
        }
        if !pending.is_empty() {
            self.matches.flush();
        }

        // Tudo falhou: propaga o erro em vez de fingir que nada casou.
        if let Some(e) = last_error
            && fresh.is_empty()
            && !pending.is_empty()
        {
            return Err(e);
        }

        Ok(keyed
            .iter()
            .map(|(key, _, _)| match fresh.get(key) {
                Some(l) => l.clone(),
                None => self.matches.get(key).map_or(Lookup::Pending, Lookup::from_cached),
            })
            .collect())
    }

    /// Procura na MAL (busca do site) as obras que o AniList não achou e tenta ligá-las ao AniList por `idMal`.
    /// Devolve `(chave, resultado, gravar_no_cache)`; `None` = nem AniList nem MAL acharam.
    /// Falhas de rede ficam de fora (viram `Pending`) e vão para `last_error`.
    async fn mal_fallback(
        &self,
        misses: &[&(String, ParsedName, MediaType)],
        last_error: &mut Option<AniListError>,
    ) -> Vec<(String, Option<Value>, bool)> {
        let mut decided = Vec::new();
        let mut hits: Vec<(&str, Value)> = Vec::new();
        for (key, parsed, _) in misses.iter().copied() {
            match self.mal_site.search(&parsed.title).await {
                Ok(candidates) => match pick_mal(&candidates, parsed) {
                    Some(jk) => hits.push((key, jk)),
                    None => decided.push((key.clone(), None, true)),
                },
                Err(AniListError::NotFound) => decided.push((key.clone(), None, true)),
                Err(e) => *last_error = Some(e),
            }
        }

        for chunk in hits.chunks(BATCH_SIZE) {
            let mut variables = Map::new();
            for (i, (_, jk)) in chunk.iter().enumerate() {
                variables.insert(format!("i{i}"), jk["idMal"].clone());
            }
            // Falha do AniList não descarta o achado da MAL: responde com ele, sem cachear.
            let (data, cache) = match self.client.query(&queries::idmal_batch_query(chunk.len()), Value::Object(variables)).await {
                Ok(data) => (data, true),
                // Nenhum idMal existe no AniList: todos ficam só-MAL.
                Err(AniListError::NotFound) => (Value::Null, true),
                Err(e) => {
                    *last_error = Some(e);
                    (Value::Null, false)
                }
            };
            for (i, (key, jk)) in chunk.iter().enumerate() {
                let on_anilist = data.get(format!("m{i}")).filter(|m| !m.is_null()).cloned();
                decided.push((key.to_string(), Some(merge_mal(on_anilist, jk.clone())), cache));
            }
        }
        decided
    }

    async fn search_batch(
        &self,
        chunk: &[&(String, ParsedName, MediaType)],
    ) -> AniListResult<Vec<(String, Option<Value>)>> {
        let types: Vec<MediaType> = chunk.iter().map(|c| c.2).collect();
        let query = queries::batch_search_query(&types, CANDIDATES_PER_SEARCH);
        let mut variables = Map::new();
        for (i, c) in chunk.iter().enumerate() {
            variables.insert(format!("s{i}"), Value::String(c.1.title.clone()));
        }

        let data = self.client.query(&query, Value::Object(variables)).await?;

        Ok(chunk
            .iter()
            .enumerate()
            .map(|(i, (key, parsed, _))| {
                let candidates = data
                    .get(format!("m{i}"))
                    .and_then(|p| p.get("media"))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let best = matcher::pick_best(&candidates, parsed).map(|idx| candidates[idx].clone());
                (key.clone(), best)
            })
            .collect())
    }

    /// Animes da temporada atual. Usa o cache de detalhes com chave própria.
    pub async fn season(&self) -> AniListResult<Value> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or_default();
        let (season, year) = queries::season_at(now);
        let key = format!("season:{season}:{year}");
        if let Some(v) = self.details.get(&key) {
            return Ok(v);
        }
        let data = self
            .client
            .query(&queries::season_query(), json!({ "season": season, "year": year }))
            .await?;
        let list = data
            .pointer("/Page/media")
            .cloned()
            .ok_or_else(|| AniListError::Decode("Page.media ausente".into()))?;
        self.details.insert(key, list.clone());
        self.details.flush();
        Ok(list)
    }

    /// Detalhe completo (descrição, staff, elenco...). Id negativo é obra só-MAL (`-mal_id`).
    pub async fn media(&self, id: i64) -> AniListResult<Value> {
        let key = id.to_string();
        if let Some(v) = self.details.get(&key) {
            return Ok(v);
        }
        if id < 0 {
            return match self.jikan.anime(-id).await {
                Ok(media) => {
                    self.details.insert(key, media.clone());
                    self.details.flush();
                    Ok(media)
                }
                // Jikan fora do ar ou inalcançável: a página abre com o resumo já casado
                // (sem cachear, para tentar o detalhe completo de novo depois).
                Err(e) => self
                    .matches
                    .find(|v| v.as_ref().and_then(|m| m.get("id")).and_then(Value::as_i64) == Some(id))
                    .flatten()
                    .map(|summary| jikan::detail_from_summary(&summary))
                    .ok_or(e),
            };
        }
        let data = self.client.query(queries::DETAIL_QUERY, json!({ "id": id })).await?;
        let media = data.get("Media").cloned().filter(|m| !m.is_null()).ok_or(AniListError::NotFound)?;
        self.details.insert(key, media.clone());
        self.details.flush();
        Ok(media)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mal_fallback_choice() {
        let jk = json!({"id": -1531, "idMal": 1531, "title": {"romaji": "Fushigiboshi no☆Futago Hime"}, "synonyms": [], "seasonYear": 2005});
        let wanted = matcher::parse_folder_name("Fushigiboshi no Futago Hime");
        assert_eq!(pick_mal(&[jk.clone()], &wanted).unwrap()["id"], -1531);
        assert_eq!(merge_mal(None, jk.clone())["id"], -1531);
        assert_eq!(merge_mal(Some(json!({"id": 777})), jk)["id"], 777);
    }

    #[test]
    fn mal_rejects_dissimilar() {
        let jk = json!({"id": -9, "idMal": 9, "title": {"romaji": "Hyouge Mono"}, "synonyms": [], "seasonYear": 2011});
        let wanted = matcher::parse_folder_name("Howl no Ugoku Shiro");
        assert!(pick_mal(&[jk], &wanted).is_none());
    }

    #[test]
    fn mal_calls_are_capped() {
        let (now, later) = split_mal((0..7).collect::<Vec<_>>(), MAL_PER_CALL);
        assert_eq!((now, later), (vec![0, 1, 2, 3], vec![4, 5, 6]));
        let (now, later) = split_mal(vec![1, 2], MAL_PER_CALL);
        assert_eq!((now, later), (vec![1, 2], Vec::<i32>::new()));
    }

    #[test]
    fn match_cache_is_versioned() {
        assert!(MATCHES_FILE.ends_with("-v2.json"));
    }
}

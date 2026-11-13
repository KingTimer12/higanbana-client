//! Escolha do pôster nas listas do fanart.tv: temporada exata, depois idioma e curtidas.

use serde::Deserialize;

/// Uma imagem da resposta. `likes` e `season` chegam como texto na API.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct Art {
    pub url: String,
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub likes: String,
    #[serde(default)]
    pub season: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct TvImages {
    /// Série a que a resposta pertence. O fanart.tv às vezes responde um TVDB id
    /// com outra série (Ox Tales, 82737 → Ulysses, 351049).
    #[serde(default)]
    pub thetvdb_id: Option<String>,
    #[serde(default)]
    pub tvposter: Vec<Art>,
    #[serde(default)]
    pub seasonposter: Vec<Art>,
}

impl TvImages {
    /// A resposta é da série pedida? Sem id na resposta, aceita.
    pub fn is_for(&self, tvdb: u32) -> bool {
        same_id(self.thetvdb_id.as_deref(), tvdb)
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct MovieImages {
    #[serde(default)]
    pub tmdb_id: Option<String>,
    #[serde(default)]
    pub movieposter: Vec<Art>,
}

impl MovieImages {
    /// A resposta é do filme pedido? Sem id na resposta, aceita.
    pub fn is_for(&self, tmdb: u32) -> bool {
        same_id(self.tmdb_id.as_deref(), tmdb)
    }
}

fn same_id(got: Option<&str>, wanted: u32) -> bool {
    got.is_none_or(|g| g.trim() == wanted.to_string())
}

/// Pôster da temporada; sem ele, o da série só se `series_poster` (temporada 1).
/// Só vale `season` numérico idêntico ao pedido ("all" e "0" não casam).
pub fn pick_tv(images: &TvImages, season: u32, series_poster: bool) -> Option<String> {
    let wanted = season.to_string();
    let of_season: Vec<&Art> = images
        .seasonposter
        .iter()
        .filter(|a| a.season.as_deref().map(str::trim) == Some(wanted.as_str()))
        .collect();
    best(&of_season).or_else(|| {
        if series_poster {
            best(&images.tvposter.iter().collect::<Vec<_>>())
        } else {
            None
        }
    })
}

pub fn pick_movie(images: &MovieImages) -> Option<String> {
    best(&images.movieposter.iter().collect::<Vec<_>>())
}

/// `en` > sem idioma (`00`/vazio) > `ja`; empate pelas curtidas. Outros idiomas
/// nunca: sem nenhum destes, a capa fica a do AniList.
fn best(arts: &[&Art]) -> Option<String> {
    arts.iter()
        .filter(|a| !a.url.is_empty())
        .filter_map(|a| Some(((lang_rank(&a.lang)?, a.likes.trim().parse::<u32>().unwrap_or(0)), a)))
        .max_by_key(|(rank, _)| *rank)
        .map(|(_, a)| a.url.clone())
}

fn lang_rank(lang: &str) -> Option<u8> {
    match lang.trim().to_ascii_lowercase().as_str() {
        "en" => Some(3),
        "00" | "" => Some(2),
        "ja" => Some(1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn art(url: &str, lang: &str, likes: &str, season: Option<&str>) -> Art {
        Art {
            url: url.into(),
            lang: lang.into(),
            likes: likes.into(),
            season: season.map(Into::into),
        }
    }

    #[test]
    fn picks_matching_season() {
        let imgs = TvImages {
            thetvdb_id: None,
            tvposter: vec![],
            seasonposter: vec![
                art("s1", "en", "0", Some("1")),
                art("s2", "en", "0", Some("2")),
            ],
        };
        assert_eq!(pick_tv(&imgs, 2, false).as_deref(), Some("s2"));
    }

    #[test]
    fn non_numeric_season_never_matches() {
        let imgs = TvImages {
            thetvdb_id: None,
            tvposter: vec![],
            seasonposter: vec![
                art("all", "en", "9", Some("all")),
                art("z", "en", "9", Some("0")),
            ],
        };
        assert_eq!(pick_tv(&imgs, 1, false), None);
    }

    #[test]
    fn no_season_falls_to_none_after_s1() {
        let imgs = TvImages {
            thetvdb_id: None,
            tvposter: vec![art("t", "en", "5", None)],
            seasonposter: vec![],
        };
        assert_eq!(pick_tv(&imgs, 2, false), None);
    }

    #[test]
    fn season_one_uses_tvposter() {
        let imgs = TvImages {
            thetvdb_id: None,
            tvposter: vec![art("t", "en", "5", None)],
            seasonposter: vec![],
        };
        assert_eq!(pick_tv(&imgs, 1, true).as_deref(), Some("t"));
    }

    #[test]
    fn lang_order_en_then_textless_then_ja() {
        let mut imgs = TvImages {
            thetvdb_id: None,
            tvposter: vec![
                art("ja", "ja", "9", None),
                art("none", "00", "1", None),
                art("en", "en", "0", None),
            ],
            seasonposter: vec![],
        };
        assert_eq!(pick_tv(&imgs, 1, true).as_deref(), Some("en"));
        imgs.tvposter.remove(2);
        assert_eq!(pick_tv(&imgs, 1, true).as_deref(), Some("none"));
    }

    #[test]
    fn response_for_another_show_is_rejected() {
        // Ox Tales (TVDB 82737): o fanart.tv respondeu com o Ulysses (351049).
        let body = r#"{"name":"Ulysses","thetvdb_id":"351049","tvposter":[{"url":"https://a/u.jpg","lang":"en","likes":"0"}]}"#;
        let t: TvImages = serde_json::from_str(body).unwrap();
        assert!(!t.is_for(82737));
        assert!(t.is_for(351049));
        assert!(TvImages::default().is_for(82737), "sem id na resposta: aceita");

        let m: MovieImages = serde_json::from_str(r#"{"tmdb_id":"10","movieposter":[]}"#).unwrap();
        assert!(!m.is_for(11));
        assert!(m.is_for(10));
    }

    #[test]
    fn other_languages_are_never_picked() {
        // Kusuriya no Hitorigoto: a 2ª temporada só tinha pôster em francês.
        let imgs = TvImages {
            thetvdb_id: None,
            tvposter: vec![art("t-en", "en", "5", None)],
            seasonposter: vec![art("s2-fr", "fr", "9", Some("2"))],
        };
        assert_eq!(pick_tv(&imgs, 2, false), None);
        let movie = MovieImages { tmdb_id: None, movieposter: vec![art("fr", "fr", "9", None)] };
        assert_eq!(pick_movie(&movie), None);
    }

    #[test]
    fn season_one_in_other_language_falls_to_series_poster() {
        let imgs = TvImages {
            thetvdb_id: None,
            tvposter: vec![art("t-en", "en", "5", None)],
            seasonposter: vec![art("s1-fr", "fr", "9", Some("1"))],
        };
        assert_eq!(pick_tv(&imgs, 1, true).as_deref(), Some("t-en"));
    }

    #[test]
    fn likes_break_ties() {
        let imgs = TvImages {
            thetvdb_id: None,
            tvposter: vec![art("a", "en", "3", None), art("b", "en", "7", None)],
            seasonposter: vec![],
        };
        assert_eq!(pick_tv(&imgs, 1, true).as_deref(), Some("b"));
    }

    #[test]
    fn movie_picks_best_poster() {
        let imgs = MovieImages {
            tmdb_id: None,
            movieposter: vec![art("a", "ja", "9", None), art("b", "en", "1", None)],
        };
        assert_eq!(pick_movie(&imgs).as_deref(), Some("b"));
        assert_eq!(pick_movie(&MovieImages::default()), None);
    }
}

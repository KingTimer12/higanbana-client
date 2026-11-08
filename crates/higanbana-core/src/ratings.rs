//! Notas exibidas na interface: de onde vêm (AniList ou TMDB) e como aparecem.

use serde::{Deserialize, Serialize};

use crate::{
    anilist::types::MediaSummary,
    tmdb::{EpisodeView, WorkEpisodes},
};

/// Nota na escala 0–10, com o número de votos quando a fonte informa.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    pub value: f32,
    pub votes: Option<u32>,
}

impl Score {
    /// "8,5": vírgula decimal e uma casa.
    pub fn label(&self) -> String {
        format!("{:.1}", self.value).replace('.', ",")
    }
}

pub trait RatingSource: Send + Sync {
    fn work(&self, anilist: Option<&MediaSummary>, tmdb: Option<&WorkEpisodes>) -> Option<Score>;
    fn episode(&self, ep: &EpisodeView) -> Option<Score>;
}

/// O AniList não tem nota por episódio, então usa a do TMDB.
fn tmdb_episode(ep: &EpisodeView) -> Option<Score> {
    (ep.vote_count > 0).then_some(Score { value: ep.vote_average, votes: Some(ep.vote_count) })
}

pub struct AniListRatings;

impl RatingSource for AniListRatings {
    fn work(&self, anilist: Option<&MediaSummary>, _tmdb: Option<&WorkEpisodes>) -> Option<Score> {
        let score = anilist?.average_score?;
        Some(Score { value: score as f32 / 10.0, votes: None })
    }

    fn episode(&self, ep: &EpisodeView) -> Option<Score> {
        tmdb_episode(ep)
    }
}

pub struct TmdbRatings;

impl RatingSource for TmdbRatings {
    fn work(&self, _anilist: Option<&MediaSummary>, tmdb: Option<&WorkEpisodes>) -> Option<Score> {
        let (value, votes) = tmdb?.show_vote;
        (votes > 0).then_some(Score { value, votes: Some(votes) })
    }

    fn episode(&self, ep: &EpisodeView) -> Option<Score> {
        tmdb_episode(ep)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RatingProvider {
    #[default]
    AniList,
    Tmdb,
}

pub fn source(p: RatingProvider) -> &'static dyn RatingSource {
    match p {
        RatingProvider::AniList => &AniListRatings,
        RatingProvider::Tmdb => &TmdbRatings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn episode(vote_average: f32, vote_count: u32) -> EpisodeView {
        EpisodeView {
            season: 1,
            number: 1,
            title: String::new(),
            overview: String::new(),
            air_date: None,
            still_path: None,
            vote_average,
            vote_count,
            runtime: None,
        }
    }

    #[test]
    fn anilist_work_score() {
        let media: MediaSummary = serde_json::from_str(r#"{"id":1,"averageScore":85}"#).unwrap();
        let score = source(RatingProvider::AniList).work(Some(&media), None).unwrap();
        assert_eq!(score.value, 8.5);
        assert_eq!(score.label(), "8,5");
        assert!(source(RatingProvider::AniList).work(None, None).is_none());
    }

    #[test]
    fn tmdb_episode_needs_votes() {
        let tmdb = source(RatingProvider::Tmdb);
        assert!(tmdb.episode(&episode(9.0, 0)).is_none());
        assert_eq!(tmdb.episode(&episode(7.94, 68)).unwrap().label(), "7,9");
        assert_eq!(source(RatingProvider::AniList).episode(&episode(7.94, 68)).unwrap().label(), "7,9");
    }
}

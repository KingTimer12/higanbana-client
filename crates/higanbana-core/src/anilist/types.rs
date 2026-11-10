//! Formas das respostas do AniList (campos dos fragments `Summary` e de detalhe).

use serde::Deserialize;

use super::MediaType;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Title {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
    pub user_preferred: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CoverImage {
    pub extra_large: Option<String>,
    pub large: Option<String>,
    /// Cor dominante da capa (`#rrggbb`).
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
pub struct FuzzyDate {
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub day: Option<u32>,
}

/// Campos do fragment `Summary` (cards e casamento).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSummary {
    pub id: i64,
    #[serde(rename = "type")]
    pub media_type: Option<MediaType>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub title: Title,
    #[serde(default)]
    pub synonyms: Vec<String>,
    #[serde(default)]
    pub season: Option<String>,
    #[serde(default)]
    pub season_year: Option<i32>,
    #[serde(default)]
    pub start_date: Option<FuzzyDate>,
    #[serde(default)]
    pub cover_image: Option<CoverImage>,
    #[serde(default)]
    pub banner_image: Option<String>,
    #[serde(default)]
    pub average_score: Option<i64>,
    #[serde(default)]
    pub popularity: Option<i64>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub episodes: Option<i64>,
    #[serde(default)]
    pub chapters: Option<i64>,
    #[serde(default)]
    pub id_mal: Option<i64>,
}

impl MediaSummary {
    /// Obra que só existe na MAL (id negativo = `-mal_id`).
    pub fn is_mal_only(&self) -> bool {
        self.id < 0
    }

    pub fn title(&self) -> String {
        let t = &self.title;
        t.user_preferred.clone().or_else(|| t.romaji.clone()).or_else(|| t.english.clone()).or_else(|| t.native.clone()).unwrap_or_default()
    }

    pub fn year(&self) -> Option<i32> {
        self.season_year.or_else(|| self.start_date.and_then(|d| d.year))
    }

    pub fn cover(&self) -> Option<&str> {
        let c = self.cover_image.as_ref()?;
        c.extra_large.as_deref().or(c.large.as_deref())
    }

    pub fn cover_small(&self) -> Option<&str> {
        let c = self.cover_image.as_ref()?;
        c.large.as_deref().or(c.extra_large.as_deref())
    }

    pub fn color(&self) -> Option<&str> {
        self.cover_image.as_ref()?.color.as_deref()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PersonName {
    pub full: Option<String>,
    pub native: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Image {
    pub large: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Person {
    pub id: i64,
    #[serde(default)]
    pub name: PersonName,
    #[serde(default)]
    pub image: Option<Image>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tag {
    pub name: String,
    #[serde(default)]
    pub rank: i64,
    #[serde(default, rename = "isMediaSpoiler")]
    pub spoiler: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Trailer {
    pub id: String,
    #[serde(default)]
    pub site: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Studio {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(bound = "T: Deserialize<'de>")]
pub struct Nodes<T> {
    #[serde(default = "Vec::new")]
    pub nodes: Vec<T>,
}

impl<T> Default for Nodes<T> {
    fn default() -> Self {
        Self { nodes: Vec::new() }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StaffEdge {
    #[serde(default)]
    pub role: String,
    pub node: Person,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterEdge {
    #[serde(default)]
    pub role: String,
    pub node: Person,
    #[serde(default)]
    pub voice_actors: Vec<Person>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RelatedTitle {
    pub user_preferred: Option<String>,
    pub romaji: Option<String>,
    pub native: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedNode {
    pub id: i64,
    #[serde(rename = "type")]
    pub media_type: Option<MediaType>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub title: RelatedTitle,
    #[serde(default)]
    pub cover_image: Option<CoverImage>,
    #[serde(default)]
    pub start_date: Option<FuzzyDate>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationEdge {
    #[serde(default)]
    pub relation_type: String,
    pub node: RelatedNode,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(bound = "T: Deserialize<'de>")]
pub struct Edges<T> {
    #[serde(default = "Vec::new")]
    pub edges: Vec<T>,
}

impl<T> Default for Edges<T> {
    fn default() -> Self {
        Self { edges: Vec::new() }
    }
}

/// Detalhe completo de uma mídia.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaDetail {
    #[serde(flatten)]
    pub summary: MediaSummary,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub site_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub end_date: Option<FuzzyDate>,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub volumes: Option<i64>,
    #[serde(default)]
    pub tags: Vec<Tag>,
    #[serde(default)]
    pub trailer: Option<Trailer>,
    #[serde(default)]
    pub studios: Nodes<Studio>,
    #[serde(default)]
    pub staff: Edges<StaffEdge>,
    #[serde(default)]
    pub characters: Edges<CharacterEdge>,
    #[serde(default)]
    pub relations: Edges<RelationEdge>,
}

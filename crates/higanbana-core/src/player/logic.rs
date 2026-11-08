//! Lógica pura do player (sem mpv): capítulos, episódios, faixas, próximo episódio.

use std::{sync::LazyLock, time::Duration};

use regex::Regex;
use serde::Deserialize;

use super::aniskip::{Interval, SkipTimes};
use super::store::TrackPref;
use crate::{
    library::{natural_cmp, parse_episode, strip_ext},
    nuvem::FileEntry,
};

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Chapter {
    #[serde(default)]
    pub title: String,
    pub time: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackKind {
    Sub,
    Audio,
    Video,
}

/// Item de `track-list` do mpv.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Track {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: TrackKind,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub selected: bool,
}

impl Track {
    /// Nome para menus: título, idioma ou número.
    pub fn name(&self) -> String {
        self.title
            .clone()
            .filter(|t| !t.is_empty())
            .or_else(|| self.lang.as_ref().map(|l| l.to_uppercase()))
            .unwrap_or_else(|| format!("Faixa {}", self.id))
    }
}

pub const VIDEO_EXT: [&str; 9] = [".mkv", ".mp4", ".avi", ".webm", ".m4v", ".mov", ".ts", ".wmv", ".flv"];

const OP_WORDS: [&str; 4] = ["opening", "op", "intro", "abertura"];
const ED_WORDS: [&str; 5] = ["ending", "ed", "outro", "credits", "encerramento"];

/// Título começa com uma das palavras, sem letra colada depois ("OP1" sim, "Operation" não).
fn starts_with_word(title: &str, words: &[&str]) -> bool {
    let t = title.trim().to_lowercase();
    words.iter().any(|w| t.starts_with(w) && !t[w.len()..].starts_with(|c: char| c.is_ascii_lowercase()))
}

/// Abertura/encerramento pelos títulos dos capítulos; cada um vai até o capítulo seguinte.
pub fn classify_chapters(chapters: &[Chapter], duration: f64) -> SkipTimes {
    let mut sorted = chapters.to_vec();
    sorted.sort_by(|a, b| a.time.total_cmp(&b.time));
    let find = |words: &[&str]| {
        let i = sorted.iter().position(|c| starts_with_word(&c.title, words))?;
        Some(Interval { start: sorted[i].time, end: sorted.get(i + 1).map_or(duration, |c| c.time) })
    };
    SkipTimes { op: find(&OP_WORDS), ed: find(&ED_WORDS) }
}

static SE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)S(\d+)E(\d+)").unwrap());
// ` - 06 [1080p]`: o número logo antes das tags vence títulos como "100 Years Quest".
static DASH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s-\s(\d{1,4})(?:v\d)?\s*(?:\[|$)").unwrap());
static LOOSE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)[_\s.]-?[_\s.]?(\d{1,4})[_\s.]*(?:\[|$)").unwrap());

/// Número do episódio pelo nome do arquivo (`S01E03`, `Ep. 12`, ` - 06 [`, `_01_[`).
pub fn episode_number(file_name: &str) -> Option<u32> {
    let base = strip_ext(file_name);
    if let Some(c) = SE.captures(base) {
        return c[2].parse().ok();
    }
    if let Some(c) = DASH.captures(base) {
        return c[1].parse().ok();
    }
    if let Some(n) = parse_episode(file_name).number.and_then(|n| n.parse::<f64>().ok())
        && n.trunc() > 0.0 {
            return Some(n.trunc() as u32);
        }
    LOOSE.captures(base).and_then(|c| c[1].parse().ok())
}

/// Temporada explícita no nome do arquivo (`S02E03` → 2); `None` sem o padrão `SxxEyy`.
pub fn episode_season(file_name: &str) -> Option<u32> {
    SE.captures(strip_ext(file_name)).and_then(|c| c[1].parse().ok())
}

pub fn is_video(f: &FileEntry) -> bool {
    !f.is_directory && f.extension.as_deref().is_some_and(|e| VIDEO_EXT.contains(&e.to_lowercase().as_str()))
}

/// Só vídeos, em ordem natural (igual à lista de arquivos).
pub fn video_files(files: &[FileEntry]) -> Vec<FileEntry> {
    let mut v: Vec<FileEntry> = files.iter().filter(|f| is_video(f)).cloned().collect();
    v.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    v
}

pub fn next_episode(files: &[FileEntry], current: &str) -> Option<FileEntry> {
    let list = video_files(files);
    let i = list.iter().position(|f| f.name == current)?;
    list.get(i + 1).cloned()
}

/// Visto: 90% da duração ou começo do encerramento.
pub fn is_watched(position: f64, duration: f64, ed: Option<Interval>) -> bool {
    (duration > 0.0 && position >= duration * 0.9) || ed.is_some_and(|e| position >= e.start)
}

/// Quando mostrar o cartão de próximo episódio.
pub fn next_card_start(duration: f64, ed: Option<Interval>, countdown_sec: u8) -> f64 {
    ed.map_or(duration - countdown_sec as f64, |e| e.start)
}

/// ISO 639-1 → 639-2 dos idiomas comuns no acervo; arquivos usam os dois (e `pt-BR`).
pub fn normalize_lang(lang: Option<&str>) -> Option<String> {
    let lang = lang?.to_lowercase();
    let base = lang.split(['-', '_']).next().unwrap_or("");
    if base.is_empty() {
        return None;
    }
    Some(
        match base {
            "pt" => "por",
            "en" => "eng",
            "ja" => "jpn",
            "ko" => "kor",
            "es" => "spa",
            "zh" | "zho" => "chi",
            "fr" | "fra" => "fre",
            "de" | "deu" => "ger",
            "it" => "ita",
            "ru" => "rus",
            "ar" => "ara",
            other => other,
        }
        .to_owned(),
    )
}

fn is_aux(t: &Track) -> bool {
    let title = t.title.as_deref().unwrap_or("").to_lowercase();
    ["sign", "song", "forced"].iter().any(|w| title.contains(w))
}

fn best_of<'a>(tracks: &[&'a Track], lang: Option<&str>, title: Option<&str>) -> Option<&'a Track> {
    let want = normalize_lang(lang)?;
    let same: Vec<&Track> = tracks.iter().copied().filter(|t| normalize_lang(t.lang.as_deref()).as_deref() == Some(&want)).collect();
    same.iter()
        .find(|t| title.is_some() && t.title.as_deref() == title)
        .or_else(|| same.iter().find(|t| !is_aux(t)))
        .or_else(|| same.first())
        .copied()
}

/// Legenda a aplicar: id, desligada, ou deixar como o mpv escolheu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SubPick {
    Id(i64),
    Off,
}

/// Legenda/áudio a aplicar ao abrir um arquivo: escolha lembrada do release
/// (por idioma + título, nunca índice) e, sem ela, os idiomas preferidos.
/// `None` = deixar como o mpv escolheu.
pub fn pick_tracks(tracks: &[Track], pref: Option<&TrackPref>, sub_lang: &str, audio_lang: &str) -> (Option<SubPick>, Option<i64>) {
    let subs: Vec<&Track> = tracks.iter().filter(|t| t.kind == TrackKind::Sub).collect();
    let audios: Vec<&Track> = tracks.iter().filter(|t| t.kind == TrackKind::Audio).collect();
    let sid = if pref.is_some_and(|p| p.sub_off) {
        Some(SubPick::Off)
    } else {
        best_of(&subs, pref.and_then(|p| p.sub_lang.as_deref()), pref.and_then(|p| p.sub_title.as_deref()))
            .or_else(|| best_of(&subs, Some(sub_lang), None))
            .map(|t| SubPick::Id(t.id))
    };
    let aid = best_of(&audios, pref.and_then(|p| p.audio_lang.as_deref()), pref.and_then(|p| p.audio_title.as_deref()))
        .or_else(|| best_of(&audios, Some(audio_lang), None))
        .map(|t| t.id);
    (sid, aid)
}

pub fn format_time(sec: f64) -> String {
    let s = sec.max(0.0).floor() as u64;
    let (h, m, ss) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{ss:02}") } else { format!("{m}:{ss:02}") }
}

/// Pode ir sozinho para o próximo episódio? Só com o arquivo atual carregado
/// (um `eof` atrasado do arquivo anterior não conta) e uma vez por arquivo.
pub struct AdvanceState<'a> {
    pub loaded: bool,
    pub eof: bool,
    pub auto_next: bool,
    pub cancelled: bool,
    pub has_next: bool,
    pub advanced_from: Option<&'a str>,
    pub current: &'a str,
}

pub fn can_auto_advance(s: &AdvanceState) -> bool {
    s.loaded && s.eof && s.auto_next && !s.cancelled && s.has_next && s.advanced_from != Some(s.current)
}

const RETRY_DELAYS: [u64; 5] = [2000, 4000, 8000, 16000, 30000];

/// Espera antes da tentativa `attempt` (0, 1, ...); `None` = desistir.
pub fn retry_delay(attempt: usize) -> Option<Duration> {
    RETRY_DELAYS.get(attempt).map(|ms| Duration::from_millis(*ms))
}

/// Duração típica do encerramento de anime de TV (ED de ~1:30).
const TYPICAL_ED_SEC: f64 = 90.0;

/// Sem capítulos nem AniSkip: estima os créditos ~1:30 antes do fim, só em
/// episódios de anime de 20–30 min (formato de TV).
pub fn estimate_ending(duration: f64, is_anime: bool) -> Option<Interval> {
    (is_anime && (20.0 * 60.0..=30.0 * 60.0).contains(&duration))
        .then_some(Interval { start: duration - TYPICAL_ED_SEC, end: duration })
}

/// Rótulo curto do episódio: `Ep. 3` ou o nome do arquivo.
pub fn episode_label(file_name: &str) -> String {
    episode_number(file_name).map_or_else(|| file_name.to_owned(), |n| format!("Ep. {n}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(title: &str, time: f64) -> Chapter {
        Chapter { title: title.into(), time }
    }
    fn iv(start: f64, end: f64) -> Option<Interval> {
        Some(Interval { start, end })
    }

    #[test]
    fn chapters_op_ed() {
        let r = classify_chapters(
            &[ch("Avant", 0.), ch("Opening", 62.), ch("Part A", 152.), ch("Part B", 700.), ch("Ending", 1290.), ch("Preview", 1380.)],
            1420.,
        );
        assert_eq!(r, SkipTimes { op: iv(62., 152.), ed: iv(1290., 1380.) });
        let r = classify_chapters(&[ch("Intro", 0.), ch("Episode", 90.), ch("Credits", 1300.)], 1400.);
        assert_eq!(r, SkipTimes { op: iv(0., 90.), ed: iv(1300., 1400.) });
        let r = classify_chapters(&[ch("op", 0.), ch("main", 90.), ch("ed", 1300.)], 1400.);
        assert_eq!(r, SkipTimes { op: iv(0., 90.), ed: iv(1300., 1400.) });
        let r = classify_chapters(&[ch("Prologue", 0.), ch("OP1", 30.), ch("A", 120.), ch("ED2", 1300.)], 1400.);
        assert_eq!(r, SkipTimes { op: iv(30., 120.), ed: iv(1300., 1400.) });
        assert_eq!(classify_chapters(&[ch("Opening1", 0.), ch("Main", 90.)], 1400.).op, iv(0., 90.));
        let none = SkipTimes { op: None, ed: None };
        assert_eq!(classify_chapters(&[ch("Operation", 0.), ch("Education", 600.)], 1400.), none);
        assert_eq!(classify_chapters(&[], 1400.), none);
        assert_eq!(classify_chapters(&[ch("Chapter 1", 0.), ch("Chapter 2", 600.)], 1400.), none);
    }

    #[test]
    fn episode_numbers() {
        for (name, n) in [
            ("[FênixFansub_&_IsekaiSubs]_Fairy_Gone_01_[HDTV][1080p][8bits][877F5B35].mp4", Some(1)),
            ("[Erai-raws] Fairy Tail - 100 Years Quest - 06 [1080p][HEVC][Multiple Subtitle][F52256A9].mkv", Some(6)),
            ("Zombie.Detective.S01E03.1080p.NF.WEB-DL.DDP2.0.x264-HBO.mkv", Some(3)),
            ("Naruto Ep. 012.mkv", Some(12)),
            ("[Grupo] Filme [1080p].mkv", None),
            ("[Grupo] Titulo - 1080p.mkv", None),
        ] {
            assert_eq!(episode_number(name), n, "{name}");
        }
    }

    #[test]
    fn episode_seasons() {
        assert_eq!(episode_season("[G] X S02E03 [1080p].mkv"), Some(2));
        assert_eq!(episode_season("X - 03.mkv"), None);
    }

    fn file(name: &str, dir: bool) -> FileEntry {
        FileEntry {
            name: name.into(),
            is_directory: dir,
            size: 1,
            modified: String::new(),
            extension: (!dir).then(|| name[name.rfind('.').unwrap()..].to_owned()),
        }
    }

    #[test]
    fn next_in_natural_order() {
        let files = [file("Ep 10.mkv", false), file("Ep 2.mkv", false), file("Ep 1.mkv", false), file("Ep 1.ass", false), file("notas.txt", false), file("Extras", true)];
        assert_eq!(next_episode(&files, "Ep 1.mkv").unwrap().name, "Ep 2.mkv");
        assert_eq!(next_episode(&files, "Ep 2.mkv").unwrap().name, "Ep 10.mkv");
        assert!(next_episode(&files, "Ep 10.mkv").is_none());
    }

    #[test]
    fn watched_and_next_card() {
        assert!(!is_watched(1200., 1400., None));
        assert!(is_watched(1260., 1400., None));
        assert!(is_watched(1291., 1400., iv(1290., 1380.)));
        assert_eq!(next_card_start(1400., iv(1290., 1380.), 5), 1290.);
        assert_eq!(next_card_start(1400., None, 10), 1390.);
    }

    fn t(id: i64, kind: TrackKind, lang: &str, title: Option<&str>, selected: bool) -> Track {
        Track { id, kind, lang: Some(lang.into()), title: title.map(Into::into), selected }
    }

    #[test]
    fn picks_tracks() {
        use TrackKind::*;
        let tracks = [
            t(1, Sub, "eng", Some("English"), false),
            t(2, Sub, "por", Some("Português (Brasil)"), false),
            t(3, Sub, "por", Some("Signs"), false),
            t(1, Audio, "jpn", None, true),
            t(2, Audio, "por", None, false),
        ];
        assert_eq!(pick_tracks(&tracks, None, "por", "jpn"), (Some(SubPick::Id(2)), Some(1)));
        let off = TrackPref { sub_off: true, ..Default::default() };
        assert_eq!(pick_tracks(&tracks, Some(&off), "por", "jpn").0, Some(SubPick::Off));
        let eng = TrackPref { sub_lang: Some("eng".into()), sub_title: Some("English".into()), ..Default::default() };
        assert_eq!(pick_tracks(&tracks, Some(&eng), "por", "jpn").0, Some(SubPick::Id(1)));
        let short = [
            t(1, Sub, "en", Some("English"), true),
            t(2, Sub, "pt-BR", Some("Portuguese(Brazil)"), false),
            t(3, Sub, "es", Some("Spanish(Latin_America)"), false),
            t(1, Audio, "ja", None, true),
        ];
        assert_eq!(pick_tracks(&short, None, "por", "jpn"), (Some(SubPick::Id(2)), Some(1)));
        let spa = TrackPref { sub_lang: Some("spa".into()), ..Default::default() };
        assert_eq!(pick_tracks(&tracks, Some(&spa), "por", "jpn").0, Some(SubPick::Id(2)));
    }

    #[test]
    fn formats_time() {
        assert_eq!(format_time(75.), "1:15");
        assert_eq!(format_time(3725.), "1:02:05");
        assert_eq!(format_time(0.), "0:00");
    }

    #[test]
    fn auto_advance() {
        let base = AdvanceState { loaded: true, eof: true, auto_next: true, cancelled: false, has_next: true, advanced_from: None, current: "ep1" };
        assert!(can_auto_advance(&base));
        assert!(!can_auto_advance(&AdvanceState { loaded: false, ..base }));
        assert!(!can_auto_advance(&AdvanceState { advanced_from: Some("ep1"), ..base }));
        assert!(!can_auto_advance(&AdvanceState { cancelled: true, ..base }));
        assert!(!can_auto_advance(&AdvanceState { auto_next: false, ..base }));
        assert!(!can_auto_advance(&AdvanceState { has_next: false, ..base }));
    }

    #[test]
    fn retries_and_ending() {
        let ms: Vec<u128> = (0..5).map(|i| retry_delay(i).unwrap().as_millis()).collect();
        assert_eq!(ms, vec![2000, 4000, 8000, 16000, 30000]);
        assert!(retry_delay(5).is_none());
        assert_eq!(estimate_ending(1431., true), iv(1341., 1431.));
        assert_eq!(estimate_ending(3486., true), None);
        assert_eq!(estimate_ending(600., true), None);
        assert_eq!(estimate_ending(1431., false), None);
    }
}

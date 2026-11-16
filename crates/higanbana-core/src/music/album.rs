//! Álbum a partir de uma pasta da nuvem: faixas, discos, capa e o resto (scans, `.cue`).

use std::sync::LazyLock;

use regex::Regex;

use crate::{
    cloud::Cloud,
    library::{join_path, natural_cmp},
    nuvem::FileEntry,
};

/// O que o decodificador (symphonia 0.5) toca. APE, CUE, WavPack e Opus ficam só para baixar.
pub const AUDIO_EXT: [&str; 5] = [".flac", ".mp3", ".wav", ".ogg", ".m4a"];
const IMAGE_EXT: [&str; 5] = [".jpg", ".jpeg", ".png", ".webp", ".gif"];
/// Vídeos e quadrinhos: pasta com eles é de episódios/capítulos, não um álbum.
const NOT_ALBUM_EXT: [&str; 10] = [".mkv", ".mp4", ".avi", ".webm", ".m4v", ".mov", ".wmv", ".ts", ".cbz", ".cbr"];

pub fn ext_of(name: &str) -> String {
    name.rfind('.').map_or(String::new(), |i| name[i..].to_lowercase())
}

fn base_of(name: &str) -> &str {
    crate::library::strip_ext(name)
}

pub fn is_audio(f: &FileEntry) -> bool {
    !f.is_directory && AUDIO_EXT.contains(&ext_of(&f.name).as_str())
}

fn is_image(f: &FileEntry) -> bool {
    !f.is_directory && IMAGE_EXT.contains(&ext_of(&f.name).as_str())
}

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pat).unwrap());
    };
}
re!(TRACK, r"^(?:(\d{1,2})-)?(\d{1,3})(?:\s*-\s+|\.\s*|\s+)(.+)$");
re!(TRACK_WORD, r"(?i)^track\s*(\d{1,3})$");
re!(CATALOG, r"[\[(]([A-Z]{2,6}-\d{3,6}[A-Z]?)[\])]");
re!(YEAR, r"(?:^|[\[(\s])((?:19|20)\d\d)(?:[.\-\])\s]|$)");
re!(ALBUM_TAG, r"(?i)^\[album\]\s*");
re!(DATE_PREFIX, r"^\d{4}(?:[.-]\d{2}){0,2}\s*-\s*");
re!(BRACKETS, r"\s*\[[^\]]*\]");
re!(CODE, r"\s*\([A-Z]{2,6}-\d{3,6}[A-Z]?\)");
re!(SPACES, r"\s+");
re!(DISC, r"(?i)^(?:disc|disk|disco|cd)\s*[-_.]?\s*(\d{1,2})$");
re!(NAMED_COVER, r"(?i)^(cover|front|folder)\b");
re!(ART_DIR, r"(?i)scan|cover|capa|art|book|jacket|image|img");

#[derive(Debug, Clone, PartialEq)]
pub struct TrackName {
    pub disc: Option<u32>,
    pub number: Option<u32>,
    pub title: String,
}

/// `1-03 - Título.flac`, `01 - Título`, `01. Título`, `Track 3`. Datas não são número.
pub fn parse_track(file_name: &str) -> TrackName {
    let base = base_of(file_name).trim();
    if let Some(m) = TRACK.captures(base) {
        let title = m[3].trim().to_owned();
        // `1984-01-25 …`: o "número" seria parte de uma data.
        if !(m.get(1).is_none() && m[2].len() == 4) {
            return TrackName { disc: m.get(1).and_then(|d| d.as_str().parse().ok()), number: m[2].parse().ok(), title };
        }
    }
    let number = TRACK_WORD.captures(base).and_then(|c| c[1].parse().ok());
    TrackName { disc: None, number, title: base.to_owned() }
}

/// Código do disco entre parênteses ou colchetes: `(SVWC-7767)`.
pub fn catalog_code(folder: &str) -> Option<String> {
    CATALOG.captures_iter(folder).last().map(|c| c[1].to_owned())
}

/// `2011.05.25 - …` ou `[2008.11.26]` → ano.
pub fn album_year(folder: &str) -> Option<String> {
    YEAR.captures(folder).map(|c| c[1].to_owned())
}

/// Nome do álbum sem data na frente nem código/tags no fim.
pub fn album_title(folder: &str) -> String {
    let t = ALBUM_TAG.replace(folder, "");
    let t = DATE_PREFIX.replace(&t, "");
    let t = BRACKETS.replace_all(&t, "");
    let t = CODE.replace_all(&t, "");
    // "_" separa palavras em nomes como "-hack_Quantum_Main_Theme".
    let t = t.replace('_', " ");
    let t = SPACES.replace_all(&t, " ").trim().to_owned();
    if t.is_empty() { folder.to_owned() } else { t }
}

/// `Disc 1`, `CD2`, `Disco 3` → número do disco.
pub fn is_disc_dir(name: &str) -> Option<u32> {
    DISC.captures(name.trim()).and_then(|c| c[1].parse().ok())
}

fn has_non_album(files: &[FileEntry]) -> bool {
    files.iter().any(|f| !f.is_directory && NOT_ALBUM_EXT.contains(&ext_of(&f.name).as_str()))
}

/// A pasta vira encarte: tem faixas tocáveis (ou só discos em subpastas) e nenhum vídeo/quadrinho.
pub fn is_album_folder(files: &[FileEntry], disc_files: &[Vec<FileEntry>]) -> bool {
    if has_non_album(files) || disc_files.iter().any(|d| has_non_album(d)) {
        return false;
    }
    let discs = files.iter().filter(|f| f.is_directory && is_disc_dir(&f.name).is_some()).count();
    files.iter().any(is_audio) || discs >= 2
}

pub fn looks_like_album(files: &[FileEntry]) -> bool {
    is_album_folder(files, &[])
}

/// Capa: nome "cover/front/folder" (raiz, depois subpastas) > 1ª imagem da raiz > 1ª imagem de subpasta.
pub fn pick_cover(dir: &str, files: &[FileEntry], sub: &[(String, Vec<FileEntry>)]) -> Option<String> {
    let mut sub = sub.to_vec();
    sub.sort_by(|a, b| natural_cmp(&a.0, &b.0));
    let groups: Vec<(String, Vec<FileEntry>)> = std::iter::once((dir.to_owned(), files.to_vec()))
        .chain(sub)
        .map(|(d, f)| {
            let mut images: Vec<FileEntry> = f.into_iter().filter(is_image).collect();
            images.sort_by(|a, b| natural_cmp(&a.name, &b.name));
            (d, images)
        })
        .collect();
    for (d, images) in &groups {
        if let Some(named) = images.iter().find(|f| NAMED_COVER.is_match(base_of(&f.name))) {
            return Some(join_path(d, &named.name));
        }
    }
    groups.iter().find(|(_, i)| !i.is_empty()).map(|(d, i)| join_path(d, &i[0].name))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub path: String,
    pub size: u64,
    /// Nome do arquivo.
    pub name: String,
    pub disc: u32,
    pub number: Option<u32>,
    pub title: String,
    pub ext: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Album {
    pub path: String,
    pub title: String,
    pub catalog: Option<String>,
    pub year: Option<String>,
    pub tracks: Vec<Track>,
    pub discs: Vec<u32>,
    pub cover_path: Option<String>,
    /// Tamanho da capa (para buscar os bytes).
    pub cover_size: Option<u64>,
    /// O que não é faixa (scans, `.cue`, `.ape`, subpastas), para a aba "Arquivos".
    pub others: Vec<FileEntry>,
    /// Subpastas de disco já incorporadas na fila.
    pub disc_dirs: Vec<String>,
}

/// Álbum a partir da listagem (e das subpastas de disco já listadas). `None` se nada for tocável.
pub fn build_album(path: &str, files: &[FileEntry], discs: &[(u32, String, Vec<FileEntry>)]) -> Option<Album> {
    let mut discs = discs.to_vec();
    discs.sort_by_key(|d| d.0);
    let to_track = |dir: &str, f: &FileEntry, disc_default: Option<u32>| {
        let t = parse_track(&f.name);
        Track {
            path: join_path(dir, &f.name),
            size: f.size,
            name: f.name.clone(),
            disc: disc_default.or(t.disc).unwrap_or(1),
            number: t.number,
            title: t.title,
            ext: ext_of(&f.name),
        }
    };
    let mut tracks: Vec<Track> = files
        .iter()
        .filter(|f| is_audio(f))
        .map(|f| to_track(path, f, None))
        .chain(discs.iter().flat_map(|(n, dir, fs)| fs.iter().filter(|f| is_audio(f)).map(move |f| to_track(dir, f, Some(*n)))))
        .collect();
    if tracks.is_empty() {
        return None;
    }
    tracks.sort_by(|a, b| {
        a.disc.cmp(&b.disc).then_with(|| a.number.unwrap_or(u32::MAX).cmp(&b.number.unwrap_or(u32::MAX))).then_with(|| natural_cmp(&a.name, &b.name))
    });
    let disc_names: Vec<&str> = discs.iter().map(|(_, d, _)| crate::library::base_name(d)).collect();
    let mut others: Vec<FileEntry> =
        files.iter().filter(|f| !is_audio(f) && !(f.is_directory && disc_names.contains(&f.name.as_str()))).cloned().collect();
    others.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    let folder = crate::library::base_name(path);
    let mut disc_list: Vec<u32> = tracks.iter().map(|t| t.disc).collect();
    disc_list.dedup();
    let sub: Vec<(String, Vec<FileEntry>)> = discs.iter().map(|(_, d, f)| (d.clone(), f.clone())).collect();
    Some(Album {
        path: path.to_owned(),
        title: album_title(folder),
        catalog: catalog_code(folder),
        year: album_year(folder),
        discs: disc_list,
        cover_path: pick_cover(path, files, &sub),
        cover_size: None,
        others,
        disc_dirs: discs.iter().map(|(_, d, _)| d.clone()).collect(),
        tracks,
    })
}

const MAX_ART_DIRS: usize = 2;

impl Cloud {
    /// Monta o álbum da pasta: lista os discos e até duas pastas de arte, uma por vez
    /// (a nuvem limita pedidos seguidos). `None` se não houver nada tocável.
    pub async fn load_album(&self, path: &str, files: &[FileEntry]) -> Option<Album> {
        if !looks_like_album(files) {
            return None;
        }
        let mut discs = Vec::new();
        for d in files.iter().filter(|f| f.is_directory) {
            let Some(n) = is_disc_dir(&d.name) else { continue };
            let dir = join_path(path, &d.name);
            if let Ok(l) = self.list(&dir).await {
                discs.push((n, dir, l.files));
            }
        }
        let disc_files: Vec<Vec<FileEntry>> = discs.iter().map(|d| d.2.clone()).collect();
        if !is_album_folder(files, &disc_files) {
            return None;
        }
        let mut album = build_album(path, files, &discs)?;
        let mut groups: Vec<(String, Vec<FileEntry>)> = discs.iter().map(|(_, d, f)| (d.clone(), f.clone())).collect();
        let has_named = std::iter::once(files).chain(groups.iter().map(|g| g.1.as_slice())).any(|fs| fs.iter().any(|f| !f.is_directory && NAMED_COVER.is_match(&f.name)));
        if !has_named {
            for d in files.iter().filter(|f| f.is_directory && ART_DIR.is_match(&f.name)).take(MAX_ART_DIRS) {
                let dir = join_path(path, &d.name);
                if let Ok(l) = self.list(&dir).await {
                    groups.push((dir, l.files));
                }
            }
        }
        album.cover_path = pick_cover(path, files, &groups);
        album.cover_size = album.cover_path.as_ref().and_then(|cp| {
            std::iter::once((path.to_owned(), files.to_vec()))
                .chain(groups.iter().cloned())
                .flat_map(|(d, fs)| fs.into_iter().map(move |f| (join_path(&d, &f.name), f.size)))
                .find(|(p, _)| p == cp)
                .map(|(_, s)| s)
        });
        Some(album)
    }
}

/// `1:02:05` / `4:07`.
pub fn format_duration(secs: f64) -> String {
    crate::player::logic::format_time(secs)
}

fn khz(hz: u32) -> String {
    let v = hz as f64 / 1000.;
    let s = if (v - v.round()).abs() < 0.05 { format!("{}", v.round() as u64) } else { format!("{v:.1}").replace('.', ",") };
    format!("{s} kHz")
}

/// `FLAC · 16 bit / 44,1 kHz · sem perdas` (`→ 48 kHz` se reamostrado).
pub fn format_label(codec: &str, bits: Option<u32>, sample_rate: u32, lossless: bool, resampled_to: Option<u32>) -> String {
    let rate = match bits {
        Some(b) => format!("{b} bit / {}", khz(sample_rate)),
        None => khz(sample_rate),
    };
    let mut parts = vec![codec.to_owned(), rate];
    if lossless {
        parts.push("sem perdas".into());
    }
    let mut s = parts.join(" · ");
    if let Some(r) = resampled_to {
        s.push_str(" → ");
        s.push_str(&khz(r));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(name: &str) -> FileEntry {
        let dir = !name.contains('.');
        FileEntry { name: name.into(), is_directory: dir, size: 10, modified: String::new(), extension: (!dir).then(|| ext_of(name)) }
    }

    #[test]
    fn track_names() {
        assert_eq!(parse_track("1-03 - Título.flac"), TrackName { disc: Some(1), number: Some(3), title: "Título".into() });
        assert_eq!(parse_track("01 - Abertura.mp3").number, Some(1));
        assert_eq!(parse_track("07. Fim.flac").number, Some(7));
        assert_eq!(parse_track("Track 3.flac").number, Some(3));
        assert_eq!(parse_track("Sem número.flac").number, None);
    }

    #[test]
    fn folder_names() {
        let folder = "2011.05.25 - Album Name [FLAC] (SVWC-7767)";
        assert_eq!(catalog_code(folder).as_deref(), Some("SVWC-7767"));
        assert_eq!(album_year(folder).as_deref(), Some("2011"));
        assert_eq!(album_title(folder), "Album Name");
        assert_eq!(album_title("-hack_Quantum_Main_Theme"), "-hack Quantum Main Theme");
        assert_eq!(album_year("[2008.11.26] X").as_deref(), Some("2008"));
        assert_eq!(is_disc_dir("Disc 2"), Some(2));
        assert_eq!(is_disc_dir("CD1"), Some(1));
        assert_eq!(is_disc_dir("Scans"), None);
    }

    #[test]
    fn album_detection_and_build() {
        assert!(looks_like_album(&[f("01 - A.flac"), f("cover.jpg")]));
        assert!(looks_like_album(&[f("CD1"), f("CD2")]));
        assert!(!looks_like_album(&[f("01 - A.flac"), f("ep.mkv")]));
        assert!(!looks_like_album(&[f("a.ape"), f("a.cue")]));
        let files = [f("02 - B.flac"), f("01 - A.flac"), f("Scans"), f("back.jpg"), f("folder.jpg"), f("a.cue")];
        let a = build_album("OSTs/X (ABC-1234)", &files, &[]).unwrap();
        assert_eq!(a.tracks.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), vec!["A", "B"]);
        assert_eq!(a.cover_path.as_deref(), Some("OSTs/X (ABC-1234)/folder.jpg"));
        assert_eq!(a.catalog.as_deref(), Some("ABC-1234"));
        assert_eq!(a.others.len(), 4);
        let discs = [(2, "OSTs/Y/CD2".to_string(), vec![f("01 - C.flac")]), (1, "OSTs/Y/CD1".to_string(), vec![f("01 - A.flac")])];
        let a = build_album("OSTs/Y", &[f("CD1"), f("CD2")], &discs).unwrap();
        assert_eq!(a.discs, vec![1, 2]);
        assert!(a.others.is_empty());
    }

    #[test]
    fn labels() {
        assert_eq!(format_label("FLAC", Some(16), 44100, true, None), "FLAC · 16 bit / 44,1 kHz · sem perdas");
        assert_eq!(format_label("MP3", None, 44100, false, Some(48000)), "MP3 · 44,1 kHz → 48 kHz");
    }
}

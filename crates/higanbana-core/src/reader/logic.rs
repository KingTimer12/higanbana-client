//! Lógica pura do leitor: ordem das páginas, rótulo do capítulo, páginas
//! duplas, teclas e capítulos dentro de coletâneas.

use std::{cmp::Ordering, sync::LazyLock};

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::api::{OpenResult, PageInfo};
use crate::{library::natural_cmp, nuvem::FileEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    #[default]
    Rtl,
    Ltr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReadMode {
    #[default]
    Single,
    Double,
    Long,
    Wide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProgressStyle {
    #[default]
    Thread,
    Number,
    Hidden,
}

pub const COMIC_EXT: [&str; 4] = [".cbz", ".zip", ".cbr", ".rar"];

pub fn is_comic(f: &FileEntry) -> bool {
    !f.is_directory && f.extension.as_deref().is_some_and(|e| COMIC_EXT.contains(&e.to_lowercase().as_str()))
}

/// Só `.cbz`/`.zip`/`.cbr`/`.rar`, em ordem natural.
pub fn comic_files(files: &[FileEntry]) -> Vec<FileEntry> {
    let mut v: Vec<FileEntry> = files.iter().filter(|f| is_comic(f)).cloned().collect();
    v.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    v
}

pub fn sort_pages(pages: &[PageInfo]) -> Vec<PageInfo> {
    let mut v = pages.to_vec();
    v.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    v
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChapterLabel {
    /// `Cap.`, `Vol.` ou vazio.
    pub kind: &'static str,
    pub number: Option<String>,
    pub name: Option<String>,
}

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pat).unwrap());
    };
}
re!(EXT, r"(?i)\.[a-z0-9]{2,4}$");
re!(TAGS, r"\[[^\]]*\]|\([^)]*\)");
re!(RANGE, r"\d+\s*[-–]\s*\d+");
re!(CAP, r"(?i)\b(?:cap[íi]tulo|cap|chapter|ch|c)\.?\s*(\d+(?:\.\d+)?)");
re!(VOL, r"(?i)\b(?:volume|vol|v)\.?\s*(\d+(?:\.\d+)?)");
re!(BARE, r"(\d+(?:\.\d+)?)\s*$");

fn tidy(n: &str) -> String {
    n.parse::<f64>().map_or_else(|_| n.to_owned(), |v| v.to_string())
}

/// "Cap. 12", "Vol. 3", só o número, ou o nome do arquivo.
pub fn chapter_label(file_name: &str) -> ChapterLabel {
    // "_" separa palavras em nomes como "[Redisu]_AKTMP_V01_C_01".
    let base = EXT.replace(file_name, "").replace('_', " ");
    let untagged = TAGS.replace_all(&base, "");
    // "Caps. 1-100": vários capítulos num arquivo só; o nome diz mais que um número.
    if RANGE.is_match(&untagged) {
        return ChapterLabel { kind: "", number: None, name: Some(base.trim().to_owned()) };
    }
    if let Some(c) = CAP.captures(&base) {
        return ChapterLabel { kind: "Cap.", number: Some(tidy(&c[1])), name: None };
    }
    if let Some(c) = VOL.captures(&base) {
        return ChapterLabel { kind: "Vol.", number: Some(tidy(&c[1])), name: None };
    }
    if let Some(c) = BARE.captures(&untagged) {
        return ChapterLabel { kind: "", number: Some(tidy(&c[1])), name: None };
    }
    ChapterLabel { kind: "", number: None, name: Some(base.trim().to_owned()) }
}

pub fn label_text(l: &ChapterLabel) -> String {
    match &l.number {
        None => l.name.clone().unwrap_or_default(),
        Some(n) => format!("{} {n}", l.kind).trim().to_owned(),
    }
}

/// Pares da página dupla, em ordem de leitura: capa sozinha, depois pares.
/// Página larga fica sozinha e os pares recomeçam depois dela.
pub fn build_spreads(count: usize, is_wide: impl Fn(usize) -> bool) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < count {
        if i == 0 || is_wide(i) || i + 1 >= count || is_wide(i + 1) {
            out.push(vec![i]);
            i += 1;
        } else {
            out.push(vec![i, i + 1]);
            i += 2;
        }
    }
    out
}

pub fn spread_of(spreads: &[Vec<usize>], page: usize) -> usize {
    spreads.iter().position(|s| s.contains(&page)).unwrap_or(spreads.len().saturating_sub(1))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turn {
    Next,
    Prev,
}

/// Tecla (nome do GPUI) → virar página, respeitando a direção de leitura.
pub fn key_action(key: &str, dir: Direction) -> Option<Turn> {
    match key {
        "space" | "pagedown" => Some(Turn::Next),
        "pageup" => Some(Turn::Prev),
        "left" => Some(if dir == Direction::Rtl { Turn::Next } else { Turn::Prev }),
        "right" => Some(if dir == Direction::Rtl { Turn::Prev } else { Turn::Next }),
        _ => None,
    }
}

/// Quanto da pasta já foi lido (0–1): capítulos anteriores + parte do atual.
pub fn strip_progress(chapter: usize, chapters: usize, page: usize, pages: usize) -> f64 {
    if chapters == 0 || pages == 0 {
        return 0.;
    }
    (chapter as f64 + ((page + 1).min(pages) as f64 / pages as f64)) / chapters as f64
}

/// Unidade de leitura: o arquivo inteiro, um `.cbz` dentro dele (`inner`) ou uma pasta dentro dele (`group`).
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    pub file: FileEntry,
    pub path: String,
    pub inner: Option<String>,
    pub group: Option<String>,
    /// Chave do progresso. Arquivo inteiro = o próprio caminho.
    pub key: String,
    pub name: String,
}

pub fn unit_key(path: &str, inner: Option<&str>, group: Option<&str>) -> String {
    let mut k = path.to_owned();
    if let Some(i) = inner {
        k.push_str("::");
        k.push_str(i);
    }
    if let Some(g) = group {
        k.push('#');
        k.push_str(g);
    }
    k
}

fn dir_of(name: &str) -> &str {
    name.rfind('/').map_or("", |i| &name[..i])
}

fn last_seg(p: &str) -> &str {
    p.split('/').rfind(|s| !s.is_empty()).unwrap_or(p)
}

pub fn base_unit(path: &str, file: &FileEntry) -> Unit {
    Unit { file: file.clone(), path: path.to_owned(), inner: None, group: None, key: path.to_owned(), name: file.name.clone() }
}

/// Divide um arquivo em vários capítulos (zips internos ou pastas); `None` = é um capítulo só.
pub fn expand_units(path: &str, file: &FileEntry, r: &OpenResult) -> Option<Vec<Unit>> {
    if r.pages.is_empty() && !r.archives.is_empty() {
        let mut inner = r.archives.clone();
        inner.sort_by(|a, b| natural_cmp(a, b));
        return Some(
            inner
                .into_iter()
                .map(|i| Unit {
                    file: file.clone(),
                    path: path.to_owned(),
                    key: unit_key(path, Some(&i), None),
                    name: last_seg(&i).to_owned(),
                    inner: Some(i),
                    group: None,
                })
                .collect(),
        );
    }
    let mut groups: Vec<&str> = r.pages.iter().map(|p| dir_of(&p.name)).collect();
    groups.sort_by(|a, b| natural_cmp(a, b));
    groups.dedup();
    if groups.len() < 2 {
        return None;
    }
    Some(
        groups
            .into_iter()
            .map(|g| Unit {
                file: file.clone(),
                path: path.to_owned(),
                inner: None,
                key: unit_key(path, None, Some(g)),
                name: if g.is_empty() { file.name.clone() } else { last_seg(g).to_owned() },
                group: Some(g.to_owned()),
            })
            .collect(),
    )
}

pub fn pages_of_group(pages: &[PageInfo], group: Option<&str>) -> Vec<PageInfo> {
    match group {
        None => pages.to_vec(),
        Some(g) => pages.iter().filter(|p| dir_of(&p.name) == g).cloned().collect(),
    }
}

/// Comparação usada para ordenar capítulos (exposta para a interface).
pub fn compare_names(a: &str, b: &str) -> Ordering {
    natural_cmp(a, b)
}

/// Largura/altura de uma página pelo cabeçalho da imagem (sem decodificar):
/// a tela reserva a altura certa antes de a imagem chegar. `None` se o formato
/// não é conhecido (AVIF, por exemplo) ou a imagem não tem altura.
pub fn page_ratio<R: std::io::BufRead + std::io::Seek>(image: R) -> Option<f32> {
    let (w, h) = image::ImageReader::new(image).with_guessed_format().ok()?.into_dimensions().ok()?;
    (w > 0 && h > 0).then(|| w as f32 / h as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proporcao_pelo_cabecalho() {
        let mut png = Vec::new();
        image::RgbImage::new(70, 100).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        assert_eq!(page_ratio(std::io::Cursor::new(&png)), Some(0.7));
        assert_eq!(page_ratio(std::io::Cursor::new(b"nada")), None);
    }

    fn label(s: &str) -> String {
        label_text(&chapter_label(s))
    }

    #[test]
    fn chapter_labels() {
        assert_eq!(label("Chainsaw Man Cap 012.cbz"), "Cap. 12");
        assert_eq!(label("[Grupo] Obra Vol.03.cbz"), "Vol. 3");
        assert_eq!(label("[Redisu]_AKTMP_V01_C_01.cbz"), "Cap. 1");
        assert_eq!(label("Obra 7.5.cbz"), "7.5");
        assert_eq!(label("Caps. 1-100.zip"), "Caps. 1-100");
        assert_eq!(label("Extras.cbz"), "Extras");
    }

    #[test]
    fn spreads() {
        assert_eq!(build_spreads(5, |_| false), vec![vec![0], vec![1, 2], vec![3, 4]]);
        assert_eq!(build_spreads(6, |i| i == 2), vec![vec![0], vec![1], vec![2], vec![3, 4], vec![5]]);
        let s = build_spreads(5, |_| false);
        assert_eq!(spread_of(&s, 4), 2);
        assert_eq!(spread_of(&s, 99), 2);
    }

    #[test]
    fn keys_follow_direction() {
        assert_eq!(key_action("left", Direction::Rtl), Some(Turn::Next));
        assert_eq!(key_action("left", Direction::Ltr), Some(Turn::Prev));
        assert_eq!(key_action("space", Direction::Ltr), Some(Turn::Next));
        assert_eq!(key_action("x", Direction::Ltr), None);
    }

    #[test]
    fn progress_strip() {
        assert_eq!(strip_progress(1, 4, 4, 10), 0.375);
        assert_eq!(strip_progress(0, 0, 0, 0), 0.);
    }

    fn file(name: &str) -> FileEntry {
        FileEntry { name: name.into(), is_directory: false, size: 1, modified: String::new(), extension: Some(".zip".into()) }
    }

    #[test]
    fn units_from_inner_archives_and_groups() {
        let f = file("Caps. 1-100.zip");
        let r = OpenResult { pages: vec![], archives: vec!["Cap 10.cbz".into(), "Cap 2.cbz".into()] };
        let u = expand_units("a/Caps.zip", &f, &r).unwrap();
        assert_eq!(u.iter().map(|u| u.name.as_str()).collect::<Vec<_>>(), vec!["Cap 2.cbz", "Cap 10.cbz"]);
        assert_eq!(u[0].key, "a/Caps.zip::Cap 2.cbz");
        let r = OpenResult {
            pages: vec![PageInfo { index: 0, name: "Cap 1/01.jpg".into() }, PageInfo { index: 1, name: "Cap 2/01.jpg".into() }],
            archives: vec![],
        };
        let u = expand_units("a/b.zip", &f, &r).unwrap();
        assert_eq!(u[1].key, "a/b.zip#Cap 2");
        assert_eq!(pages_of_group(&r.pages, Some("Cap 2")).len(), 1);
        let single = OpenResult { pages: vec![PageInfo { index: 0, name: "01.jpg".into() }], archives: vec![] };
        assert!(expand_units("a/b.zip", &f, &single).is_none());
    }
}

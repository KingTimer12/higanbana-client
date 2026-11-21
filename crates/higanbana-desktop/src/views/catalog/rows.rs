//! O catálogo como uma lista virtual de linhas: cabeçalho, título de cada letra,
//! linhas de pôsteres (uma linha da grade por item), esqueleto/erro da letra e
//! rodapé. Só as linhas visíveis são construídas (RAM e CPU com 4 mil obras).

use std::sync::Arc;

use higanbana_core::anime_index::{IndexEntry, LETTERS};
use higanbana_core::library::norm;

/// Obras de uma letra: (índice da letra, entradas).
pub type LetterSection = (usize, Arc<Vec<IndexEntry>>);

#[derive(Clone)]
pub enum Row {
    Header,
    /// Aviso de lista vazia (abaixo do cabeçalho).
    Notice,
    LetterHead { letter: usize, count: Option<usize> },
    /// Uma linha da grade: `works[range]`.
    Cards { works: Arc<[IndexEntry]>, start: usize, end: usize },
    Loading { letter: usize },
    Failed { letter: usize, error: String },
    NoneInLetter,
    /// Fim da lista: aparecer pede a próxima letra (`Some`).
    Footer { next: Option<usize> },
}

/// Corpo de uma letra (sequência sem busca).
pub enum Body {
    Entries(Arc<Vec<IndexEntry>>),
    Loading,
    Failed(String),
}

/// Medidas que definem as alturas.
#[derive(Clone, Copy, PartialEq)]
pub struct Metrics {
    pub cols: usize,
    pub card_w: f32,
    pub header_h: f32,
}

pub const LETTER_HEAD: f32 = 60.0;
pub const ROW_GAP: f32 = 32.0;
pub const FOOTER: f32 = 64.0;
const NOTICE: f32 = 200.0;
const FAILED: f32 = 112.0;
const NONE: f32 = 52.0;

impl Metrics {
    /// Capa 2:3 + espaço + título em 2 linhas + formato, e o espaço até a próxima linha.
    pub fn card_row(&self) -> f32 {
        self.card_w * 1.5 + 8.0 + 40.0 + 18.0 + ROW_GAP
    }

    pub fn height(&self, r: &Row) -> f32 {
        match r {
            Row::Header => self.header_h,
            Row::Notice => NOTICE,
            Row::LetterHead { .. } => LETTER_HEAD,
            Row::Cards { .. } | Row::Loading { .. } => self.card_row(),
            Row::Failed { .. } => FAILED,
            Row::NoneInLetter => NONE,
            Row::Footer { .. } => FOOTER,
        }
    }
}

/// Rótulo da letra no índice lateral: `0-9` → `#`, `Letra F` → `F`.
pub fn rail_label(letter: &str) -> &str {
    if letter == "0-9" { "#" } else { letter.trim_start_matches("Letra ") }
}

/// Rótulo da letra no cabeçalho da seção.
pub fn header_label(letter: &str) -> &str {
    if letter == "0-9" { "0–9" } else { letter.trim_start_matches("Letra ") }
}

/// Obras cujo nome contém `query` (sem acento nem caixa), por letra; letras sem obra somem.
pub fn search_sections<'a>(letters: impl IntoIterator<Item = (usize, &'a [IndexEntry])>, query: &str) -> Vec<LetterSection> {
    let q = norm(query);
    letters
        .into_iter()
        .map(|(ix, entries)| (ix, entries.iter().filter(|e| norm(&e.name).contains(&q)).cloned().collect::<Vec<_>>()))
        .filter(|(_, found)| !found.is_empty())
        .map(|(ix, found)| (ix, Arc::new(found)))
        .collect()
}

fn push_cards(rows: &mut Vec<Row>, works: Arc<[IndexEntry]>, cols: usize) {
    let n = works.len();
    for start in (0..n).step_by(cols.max(1)) {
        rows.push(Row::Cards { works: works.clone(), start, end: (start + cols).min(n) });
    }
}

/// Sem busca: letras de 0-9 até `upto`, cada uma com o estado dela, e o rodapé.
pub fn letter_rows(bodies: Vec<(usize, Body)>, cols: usize, upto: usize) -> Vec<Row> {
    let mut rows = vec![Row::Header];
    let mut last_loaded = false;
    for (letter, body) in bodies {
        match body {
            Body::Entries(e) => {
                rows.push(Row::LetterHead { letter, count: Some(e.len()) });
                if e.is_empty() {
                    rows.push(Row::NoneInLetter);
                } else {
                    push_cards(&mut rows, e.as_slice().into(), cols);
                }
                last_loaded = true;
            }
            Body::Loading => {
                rows.push(Row::LetterHead { letter, count: None });
                rows.push(Row::Loading { letter });
                last_loaded = false;
            }
            Body::Failed(error) => {
                rows.push(Row::LetterHead { letter, count: None });
                rows.push(Row::Failed { letter, error });
                last_loaded = false;
            }
        }
    }
    let next = (last_loaded && upto + 1 < LETTERS.len()).then_some(upto + 1);
    rows.push(Row::Footer { next });
    rows
}

/// Com busca: só as letras com resultado.
pub fn search_rows(found: &[LetterSection], cols: usize, notice: bool) -> Vec<Row> {
    let mut rows = vec![Row::Header];
    if notice {
        rows.push(Row::Notice);
    }
    for (letter, e) in found {
        rows.push(Row::LetterHead { letter: *letter, count: Some(e.len()) });
        push_cards(&mut rows, e.as_slice().into(), cols);
    }
    rows.push(Row::Footer { next: None });
    rows
}

/// Com filtro ou fora de A–Z: uma grade só.
pub fn flat_rows(works: Arc<[IndexEntry]>, cols: usize, notice: bool) -> Vec<Row> {
    let mut rows = vec![Row::Header];
    if notice {
        rows.push(Row::Notice);
    }
    push_cards(&mut rows, works, cols);
    rows.push(Row::Footer { next: None });
    rows
}

/// Linha do título da letra `letter`, se está na lista.
pub fn letter_row(rows: &[Row], letter: usize) -> Option<usize> {
    rows.iter().position(|r| matches!(r, Row::LetterHead { letter: l, .. } if *l == letter))
}

/// Letra cuja seção está no topo em `offset` (`None` = cabeçalho).
pub fn letter_at(rows: &[Row], heights: &[f32], offset: f32) -> Option<usize> {
    let (mut y, mut cur) = (0.0, None);
    for (r, h) in rows.iter().zip(heights) {
        if y > offset + 1.0 {
            break;
        }
        if let Row::LetterHead { letter, .. } = r {
            cur = Some(*letter);
        }
        y += h;
    }
    cur
}

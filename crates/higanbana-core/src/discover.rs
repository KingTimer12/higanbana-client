//! Vitrines da entrada: percorre uma lista de ranking e fica só com o que está
//! no acervo, conferindo em ordem até achar `n`.

use std::{collections::HashSet, future::Future};

use futures_util::{StreamExt, stream};

use crate::anilist::types::RankedMedia;

/// Candidatas conferidas ao mesmo tempo. A ordem do ranking se mantém: cada
/// achada só sai depois das que vêm antes dela. O catálogo limita as listagens
/// da nuvem por conta própria; aqui é só para não esperar uma de cada vez.
pub const AHEAD: usize = 4;

/// Confere candidatas em ordem (um ano de cada vez) até achar `n` no acervo.
/// Cada achada sai por `on_found` assim que é confirmada. Devolve quantas achou.
pub async fn fill<FL, FutL, FC, FutC>(
    n: usize,
    years: &[Option<i32>],
    mut fetch: FL,
    mut check: FC,
    mut on_found: impl FnMut(RankedMedia, String),
) -> usize
where
    FL: FnMut(Option<i32>) -> FutL,
    FutL: Future<Output = Vec<RankedMedia>>,
    FC: FnMut(&RankedMedia) -> FutC,
    FutC: Future<Output = Option<String>>,
{
    let mut seen = HashSet::new();
    let mut found = 0;
    for &year in years {
        if found >= n {
            break;
        }
        let fresh: Vec<RankedMedia> = fetch(year).await.into_iter().filter(|m| seen.insert(m.summary.id)).collect();
        let mut checks = stream::iter(fresh)
            .map(|m| {
                let pending = check(&m);
                async move { (m, pending.await) }
            })
            .buffered(AHEAD);
        while let Some((m, path)) = checks.next().await {
            if let Some(path) = path {
                found += 1;
                on_found(m, path);
                if found >= n {
                    // As conferências adiantadas que sobraram são canceladas aqui.
                    break;
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    fn ranked(id: i64) -> RankedMedia {
        serde_json::from_value(serde_json::json!({ "id": id })).unwrap()
    }

    async fn run(n: usize, years: &[Option<i32>], lists: Vec<(Option<i32>, Vec<i64>)>, accept: impl Fn(i64) -> bool) -> (Vec<i64>, Vec<i64>) {
        let checked = Rc::new(RefCell::new(Vec::new()));
        let mut found = Vec::new();
        let c = checked.clone();
        fill(
            n,
            years,
            |y| {
                let ids = lists.iter().find(|(ly, _)| *ly == y).map(|(_, v)| v.clone()).unwrap_or_default();
                async move { ids.into_iter().map(ranked).collect() }
            },
            |m| {
                let id = m.summary.id;
                c.borrow_mut().push(id);
                let ok = accept(id);
                async move { ok.then(|| format!("p/{id}")) }
            },
            |m, _| found.push(m.summary.id),
        )
        .await;
        let checked = checked.borrow().clone();
        (found, checked)
    }

    #[tokio::test]
    async fn skips_not_in_library_and_stops_at_n() {
        let (found, checked) = run(2, &[Some(2026)], vec![(Some(2026), (1..=20).collect())], |id| id % 2 == 0).await;
        assert_eq!(found, vec![2, 4]);
        // Só as adiantadas além da última achada; o resto da lista nem começa.
        assert!(checked.len() <= 4 + AHEAD, "{checked:?}");
    }

    #[tokio::test]
    async fn falls_back_to_previous_year() {
        let (found, _) = run(2, &[Some(2026), Some(2025)], vec![(Some(2026), vec![1, 2]), (Some(2025), vec![3])], |id| id != 2).await;
        assert_eq!(found, vec![1, 3]);
    }

    #[tokio::test]
    async fn same_id_in_two_years_once() {
        let (found, checked) = run(3, &[Some(2026), Some(2025)], vec![(Some(2026), vec![1]), (Some(2025), vec![1, 2])], |_| true).await;
        assert_eq!(found, vec![1, 2]);
        assert_eq!(checked, vec![1, 2]);
    }

    #[tokio::test]
    async fn empty_fetch_returns_zero() {
        let n = fill(6, &[None], |_| async { Vec::new() }, |_| async { Some(String::new()) }, |_, _| {}).await;
        assert_eq!(n, 0);
    }
}

// As seções do acervo que usam a mesma tela de catálogo, a mesma busca e o
// mesmo pôster (como o `series.rs` do desktop). O que muda entre elas fica aqui;
// uma seção nova entra com um descritor novo e um índice de letras no núcleo.
import type { MediaKind } from "./backend";

export interface Section {
  kind: MediaKind;
  /** Título da aba: `Animes`. */
  title: string;
  /** A obra no singular, para os textos: `anime`, `mangá`. */
  noun: string;
}

export const ANIME: Section = { kind: "anime", title: "Animes", noun: "anime" };
export const MANGA: Section = { kind: "manga", title: "Mangás", noun: "mangá" };

export function sectionOf(kind: MediaKind): Section {
  return kind === "manga" ? MANGA : ANIME;
}

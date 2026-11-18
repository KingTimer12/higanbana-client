import type { Media } from "./backend";

/** `75` → `1:15`; `3725` → `1:02:05`. */
export function formatTime(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`;
}

/** O título que o AniList prefere mostrar. */
export function mediaTitle(media: Media): string {
  const t = media.title;
  return t.userPreferred ?? t.romaji ?? t.english ?? t.native ?? "";
}

/** A capa em tamanho de card. */
export function mediaCover(media: Media | null | undefined): string | null {
  return media?.coverImage?.large ?? media?.coverImage?.extraLarge ?? null;
}

const FORMATS: Record<string, string> = {
  TV: "TV",
  TV_SHORT: "TV curto",
  MOVIE: "Filme",
  OVA: "OVA",
  ONA: "ONA",
  SPECIAL: "Especial",
  MUSIC: "Música",
  MANGA: "Mangá",
  NOVEL: "Novel",
  ONE_SHOT: "One-shot",
};

/** `2024 · TV · 12 episódios · 82%` (ou `· 120 capítulos`), só com o que a mídia tem. */
export function mediaFacts(media: Media): string {
  const facts: string[] = [];
  if (media.seasonYear) facts.push(String(media.seasonYear));
  if (media.format) facts.push(FORMATS[media.format] ?? media.format);
  if (media.episodes) facts.push(media.episodes === 1 ? "1 episódio" : `${media.episodes} episódios`);
  if (media.chapters) facts.push(media.chapters === 1 ? "1 capítulo" : `${media.chapters} capítulos`);
  else if (media.volumes) facts.push(media.volumes === 1 ? "1 volume" : `${media.volumes} volumes`);
  if (media.averageScore) facts.push(`${media.averageScore}%`);
  return facts.join(" · ");
}

/** Linha de baixo do pôster: `TV · 2024`, com o que houver (o ano da pasta quando o AniList não tem). */
export function posterMeta(media: Media | null | undefined, folderYear: number | null = null): string {
  const format = media?.format ? (FORMATS[media.format] ?? media.format) : null;
  const year = media?.seasonYear ?? folderYear;
  return [format, year ? String(year) : null].filter(Boolean).join(" · ");
}

/** `Destaques de 2026`: o título do carrossel do início, o ano corrente como no desktop. */
export function highlightsHeading(now = new Date()): string {
  return `Destaques de ${now.getFullYear()}`;
}

/** Número do episódio no card: `01`, `12`, `104`. */
export function episodeNumber(n: number): string {
  return String(n).padStart(2, "0");
}

/** Sinopse do AniList sem as marcas de HTML. */
export function plainText(html: string): string {
  return html
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<[^>]+>/g, "")
    .replace(/&amp;/g, "&")
    .replace(/&quot;/g, '"')
    .replace(/&#0?39;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

/** `Letra A` → `A`; `0-9` fica. */
export function letterLabel(letter: string): string {
  return letter.replace(/^Letra\s+/, "");
}

/** O último pedaço de um caminho da nuvem. */
export function baseName(path: string): string {
  const parts = path.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

/** Mensagem legível de um erro qualquer. */
export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

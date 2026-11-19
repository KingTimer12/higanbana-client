// O que as telas pedem ao núcleo. A implementação real é o crate Rust
// (`lib/native.ts`); os testes e a prévia usam uma de mentira (`fixture/`).

/** Quem está logado. */
export interface User {
  name: string;
  avatar: string | null;
}

/** Uma versão mais nova do app, publicada no GitHub. */
export interface AppUpdate {
  version: string;
  /** As notas da release (markdown do GitHub). */
  notes: string;
  /** Tamanho do download, em bytes. */
  size: number;
}

/** A seção do acervo: os animes (vídeo) ou os mangás (leitura). */
export type MediaKind = "anime" | "manga";

/** Resumo de uma mídia do AniList (campos que as telas usam). */
export interface Media {
  id: number;
  idMal?: number | null;
  format?: string | null;
  status?: string | null;
  title: { romaji?: string | null; english?: string | null; native?: string | null; userPreferred?: string | null };
  coverImage?: { extraLarge?: string | null; large?: string | null; color?: string | null } | null;
  bannerImage?: string | null;
  averageScore?: number | null;
  genres?: string[];
  episodes?: number | null;
  /** Só nos mangás. */
  chapters?: number | null;
  volumes?: number | null;
  seasonYear?: number | null;
  description?: string | null;
  /** Só nas vitrines: as tags mais relevantes sem spoiler e o estúdio principal. */
  tags?: string[];
  studio?: string | null;
}

/** As listas da vitrine do início, como no desktop. */
export type Ranking = "popular" | "topRated" | "trending";

/** Uma obra da vitrine: a mídia e onde ela está na nuvem. */
export interface Featured {
  media: Media;
  path: string;
}

/** Uma pasta de obra na nuvem. */
export interface WorkEntry {
  name: string;
  path: string;
  title: string;
  year: number | null;
}

/** Uma versão (pasta de release) de uma obra. */
export interface Version {
  folder: string;
  path: string;
  name: string;
  resolution: string | null;
  source: string | null;
  audio: string | null;
}

/** Um episódio com o progresso de quem assiste. */
export interface Episode {
  name: string;
  path: string;
  size: number;
  number: number | null;
  label: string;
  quality: string | null;
  position: number;
  duration: number;
  watched: boolean;
}

/** O que o TMDB sabe de um episódio (casado pelo arquivo, como no desktop). */
export interface EpisodeInfo {
  title: string;
  overview: string;
  /** URL da miniatura (still), se o TMDB tem. */
  still: string | null;
  airDate: string | null;
  /** Minutos. */
  runtime: number | null;
  /** Nota de 0 a 10, só com votos. */
  vote: number | null;
}

/** Um capítulo de mangá (um arquivo `.cbz`, `.zip`, ...) com o progresso de quem lê. */
export interface Comic {
  name: string;
  path: string;
  size: number;
  /** `Cap. 12`, `Vol. 3` ou o nome do arquivo. */
  label: string;
  /** Página onde parou (desde 0) e quantas o capítulo tem; `total` 0 se nunca abriu. */
  page: number;
  total: number;
  read: boolean;
  /** Numa coletânea, o capítulo interno onde parou. */
  unitName: string | null;
}

/** Uma pasta de obra aberta: franquia (`seasons`), releases (`versions`) ou arquivos
 * soltos (`episodes` de vídeo, `chapters` de mangá). */
export interface Work {
  path: string;
  title: string;
  seasons: WorkEntry[];
  /** Só na franquia: `Franquia · 2004–2019`. */
  span?: string;
  versions: Version[];
  defaultVersion: string | null;
  episodes: Episode[];
  chapters: Comic[];
}

/** Uma aba da franquia (`Séries TV · 3`), com as posições das obras em `seasons`. */
export interface FranchiseTab {
  id: string;
  label: string;
  works: number[];
}

/** Um capítulo dentro de um arquivo (coletâneas trazem vários). */
export interface ReadUnit {
  /** Chave do progresso: o caminho do arquivo, quando ele é um capítulo só. */
  key: string;
  name: string;
  label: string;
  inner: string | null;
  group: string | null;
}

/** Onde parou num capítulo. */
export interface ReadProgress {
  page: number;
  total: number;
  read: boolean;
}

/** Uma faixa do arquivo, como o mpv lista. */
export interface Track {
  id: number;
  type: "sub" | "audio" | "video";
  lang?: string | null;
  title?: string | null;
  selected?: boolean;
}

export interface Chapter {
  title: string;
  time: number;
}

export interface Interval {
  start: number;
  end: number;
}

/** Abertura e encerramento de um episódio. */
export interface SkipTimes {
  op: Interval | null;
  ed: Interval | null;
}

/** Um episódio em andamento. */
export interface Resume {
  path: string;
  dir: string;
  name: string;
  title: string;
  label: string;
  position: number;
  duration: number;
}

export interface PlayerSettings {
  autoNext: boolean;
  countdownSec: number;
  showSkipIntro: boolean;
}

/** Faixas a aplicar ao abrir um arquivo; `sub` nulo deixa como o player escolheu. */
export interface TrackPick {
  sub: number | "off" | null;
  audio: number | null;
}

/** Um arquivo baixado para ver sem internet (ou na fila). */
export interface OfflineItem {
  path: string;
  name: string;
  size: number;
  kind: MediaKind;
  workPath: string;
  workTitle: string;
  /** `Ep. 3`, `Cap. 12`. */
  label: string;
  status: "queued" | "downloading" | "done" | "failed";
  error: string | null;
  /** Bytes já baixados. */
  downloaded: number;
}

/** O que a tela manda baixar: o arquivo e o que a lista de baixados mostra sem rede. */
export type OfflineRequest = Pick<OfflineItem, "path" | "name" | "size" | "kind" | "workPath" | "workTitle" | "label">;

/** Uma página do leitor no cache do app. */
export interface ReaderPage {
  uri: string;
  ratio: number | null;
}

export interface Backend {
  user(): Promise<User | null>;
  /** Resolve quando o navegador volta do login; a página a abrir sai em `loginUrl`. */
  login(): Promise<User>;
  loginUrl(): Promise<string | null>;
  cancelLogin(): Promise<void>;
  logout(): Promise<void>;

  /** Até `n` animes de um ranking do AniList que estão no acervo, na ordem do ranking.
   * `onProgress` recebe a lista parcial a cada obra confirmada; a promessa, a final. */
  discover(ranking: Ranking, n: number, onProgress?: (found: Featured[]) => void): Promise<Featured[]>;
  /** Caminho da obra na nuvem, ou `null` se não está no acervo. */
  find(media: Media): Promise<string | null>;
  /** URI do pôster local do fanart.tv, ou `null` (fica a capa do AniList). */
  cover(media: Media): Promise<string | null>;
  media(id: number): Promise<Media>;
  lookup(names: string[], kind: MediaKind): Promise<(Media | null)[]>;
  search(query: string, kind: MediaKind): Promise<WorkEntry[]>;
  letters(): string[];
  letter(letter: string, kind: MediaKind): Promise<WorkEntry[]>;

  open(path: string): Promise<Work>;
  episodes(path: string): Promise<Episode[]>;
  chapters(path: string): Promise<Comic[]>;
  /** As abas da franquia, como no desktop: obras pelo nome da pasta e pelo formato do AniList (`null` enquanto não se sabe). */
  franchiseTabs(names: string[], formats: (string | null)[]): FranchiseTab[];
  /** Título e miniatura do TMDB de cada arquivo, na ordem de `files` (`null` onde não casa).
   * `media` é o detalhe do AniList; `folder`, o nome da pasta da obra (ou da temporada). */
  episodeInfo(media: Media, folder: string, files: string[]): Promise<(EpisodeInfo | null)[]>;
  rememberVersion(workPath: string, folder: string): void;

  streamUrl(path: string, size: number): string;
  settings(): PlayerSettings;
  /** Guarda a posição; devolve se o episódio conta como visto. Com `later`, a gravação
   * fica na fila e a chamada volta sem esperar o disco (o salvamento periódico). */
  saveProgress(path: string, position: number, duration: number, endingStart: number | null, later?: boolean): boolean;
  pickTracks(tracks: Track[], dir: string): TrackPick;
  rememberTracks(tracks: Track[], dir: string, sub: number | null, audio: number | null): void;
  skipTimes(chapters: Chapter[], duration: number, malId: number | null, episode: number | null): Promise<SkipTimes>;
  continueWatching(): Resume[];
  setPlaying(playing: boolean): void;

  /** Os capítulos de um arquivo de mangá (um só, se ele não se divide). */
  readerUnits(path: string, size: number): Promise<ReadUnit[]>;
  /** As páginas de um capítulo, em ordem de leitura (o que `readerPage` recebe). */
  readerPages(path: string, size: number, unit: ReadUnit): Promise<number[]>;
  /** Uma página pronta para o `Image` (baixada para o cache), com a largura/altura
   * dela quando o cabeçalho diz (`null` em formatos que o núcleo não lê, como AVIF). */
  readerPage(path: string, size: number, unit: ReadUnit, index: number): Promise<ReaderPage>;
  /** Baixa um arquivo para ver sem internet; o player e o leitor passam a usar a cópia. */
  offlineDownload(item: OfflineRequest): void;
  /** Os baixados (e os na fila), do mais recente. */
  offlineList(): OfflineItem[];
  /** Interrompe, apaga a cópia e tira da lista. */
  offlineRemove(path: string): void;

  /** Guarda a página onde parou num capítulo (`unit.key`). */
  saveReading(key: string, page: number, total: number): void;
  /** Numa coletânea: em qual capítulo interno (de `count`) parou, gravado no arquivo. */
  saveReadingSummary(path: string, index: number, count: number, read: boolean, unit: ReadUnit): void;
  readingOf(key: string): ReadProgress | null;

  /** A versão instalada (`0.1.0`). */
  version(): string;
  /** Uma versão mais nova, ou `null` (já está na mais nova, ou a plataforma não se atualiza sozinha). */
  checkUpdate(): Promise<AppUpdate | null>;
  /** Baixa e confere a versão achada por `checkUpdate`; resolve quando o arquivo está pronto. */
  downloadUpdate(): Promise<void>;
  /** Quanto do download chegou, de 0 a 1. */
  updateProgress(): number;
  /** Abre o instalador do sistema com o arquivo baixado; `false` se ele não abriu. */
  installUpdate(): Promise<boolean>;
}

let current: Backend | undefined;

/** Define de onde vêm os dados; chamado uma vez, antes de `mount`. */
export function setBackend(backend: Backend) {
  current = backend;
}

/** O núcleo em uso. */
export function backend(): Backend {
  if (!current) throw new Error("backend não definido: chame setBackend antes de mount");
  return current;
}

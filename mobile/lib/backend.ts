// O que as telas pedem ao núcleo. A implementação real é o crate Rust
// (`lib/native.ts`); os testes e a prévia usam uma de mentira (`fixture/`).

/** Quem está logado. */
export interface User {
  name: string;
  avatar: string | null;
}

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
  seasonYear?: number | null;
  description?: string | null;
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

/** Uma pasta de obra aberta: franquia (`seasons`), releases (`versions`) ou arquivos soltos (`episodes`). */
export interface Work {
  path: string;
  title: string;
  seasons: WorkEntry[];
  versions: Version[];
  defaultVersion: string | null;
  episodes: Episode[];
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

export interface Backend {
  user(): Promise<User | null>;
  /** Resolve quando o navegador volta do login; a página a abrir sai em `loginUrl`. */
  login(): Promise<User>;
  loginUrl(): Promise<string | null>;
  cancelLogin(): Promise<void>;
  logout(): Promise<void>;

  season(): Promise<Media[]>;
  /** Caminho da obra na nuvem, ou `null` se não está no acervo. */
  find(media: Media): Promise<string | null>;
  media(id: number): Promise<Media>;
  lookup(names: string[]): Promise<(Media | null)[]>;
  search(query: string): Promise<WorkEntry[]>;
  letters(): string[];
  letter(letter: string): Promise<WorkEntry[]>;

  open(path: string): Promise<Work>;
  episodes(path: string): Promise<Episode[]>;
  rememberVersion(workPath: string, folder: string): void;

  streamUrl(path: string, size: number): string;
  settings(): PlayerSettings;
  /** Guarda a posição; devolve se o episódio conta como visto. */
  saveProgress(path: string, position: number, duration: number, endingStart: number | null): boolean;
  pickTracks(tracks: Track[], dir: string): TrackPick;
  rememberTracks(tracks: Track[], dir: string, sub: number | null, audio: number | null): void;
  skipTimes(chapters: Chapter[], duration: number, malId: number | null, episode: number | null): Promise<SkipTimes>;
  continueWatching(): Resume[];
  setPlaying(playing: boolean): void;
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

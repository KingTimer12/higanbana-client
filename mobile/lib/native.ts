// O núcleo de verdade: o crate Rust do app (`src/lib.rs`), que fala com a
// nuvem, o AniList e guarda sessão e progresso.
import process from "process";
import { openUrl, platform } from "nsh";
import { Account, Catalog, Library, Player, Reader, Updates } from "higanbana/native";
import type {
  AppUpdate,
  Backend,
  Chapter,
  Comic,
  Episode,
  EpisodeInfo,
  Featured,
  FranchiseTab,
  Media,
  PlayerSettings,
  ReadProgress,
  ReadUnit,
  Resume,
  SkipTimes,
  Track,
  TrackPick,
  User,
  Work,
  WorkEntry,
} from "./backend";

/** Inicia o núcleo e devolve o backend nativo. */
export function nativeBackend(): Backend {
  // No celular os dados ficam na pasta do app; no desktop, nas pastas do
  // sistema, as mesmas do app desktop (mesma sessão e progresso).
  // `process.dataDir` é nulo na sessão do `nsh dev` no Android; aí o crate usa a pasta do app.
  Account.start(platform.desktop ? null : process.dataDir);
  // O endereço do arquivo baixado; o host Android o troca pelo instalador de pacotes.
  let installer: string | null = null;
  return {
    user: () => Account.user() as Promise<User | null>,
    login: () => Account.login() as Promise<User>,
    loginUrl: () => Account.loginUrl(),
    cancelLogin: () => Account.cancelLogin(),
    logout: () => Account.logout(),

    discover: (ranking, n) => Catalog.discover(ranking, n) as Promise<Featured[]>,
    find: (media) => Catalog.find(media),
    cover: (media) => Catalog.cover(media),
    media: (id) => Catalog.media(id) as Promise<Media>,
    lookup: (names, kind) => Catalog.lookup(names, kind) as Promise<(Media | null)[]>,
    search: (query, kind) => Catalog.search(query, kind) as Promise<WorkEntry[]>,
    letters: () => Catalog.letters(),
    letter: (letter, kind) => Catalog.letter(letter, kind) as Promise<WorkEntry[]>,

    open: (path) => Library.open(path) as Promise<Work>,
    episodes: (path) => Library.episodes(path) as Promise<Episode[]>,
    chapters: (path) => Library.chapters(path) as Promise<Comic[]>,
    franchiseTabs: (names, formats) => Library.franchiseTabs(names, formats) as FranchiseTab[],
    episodeInfo: (media, folder, files) => Library.tmdbEpisodes(media.id, folder, files) as Promise<(EpisodeInfo | null)[]>,
    rememberVersion: (workPath, folder) => Library.rememberVersion(workPath, folder),

    streamUrl: (path, size) => Player.streamUrl(path, size),
    settings: () => Player.settings() as PlayerSettings,
    saveProgress: (path, position, duration, endingStart, later) =>
      later
        ? Player.saveProgressLater(path, position, duration, endingStart)
        : Player.saveProgress(path, position, duration, endingStart),
    pickTracks: (tracks: Track[], dir) => Player.pickTracks(tracks, dir) as TrackPick,
    rememberTracks: (tracks, dir, sub, audio) => Player.rememberTracks(tracks, dir, sub, audio),
    skipTimes: (chapters: Chapter[], duration, malId, episode) =>
      Player.skipTimes(chapters, duration, malId, episode) as Promise<SkipTimes>,
    continueWatching: () => Player.continueWatching() as Resume[],
    setPlaying: (playing) => Player.setPlaying(playing),

    readerUnits: (path, size) => Reader.units(path, size) as Promise<ReadUnit[]>,
    readerPages: (path, size, unit) => Reader.pages(path, size, unit.inner, unit.group),
    readerPage: (path, size, unit, index) => Reader.page(path, size, unit.inner, index),
    saveReading: (key, page, total) => Reader.saveProgress(key, page, total),
    saveReadingSummary: (path, index, count, read, unit) => Reader.saveSummary(path, index, count, read, unit.key, unit.name),
    readingOf: (key) => Reader.progress(key) as ReadProgress | null,

    version: () => Updates.version(),
    checkUpdate: () => Updates.check() as Promise<AppUpdate | null>,
    downloadUpdate: async () => {
      installer = await Updates.download();
    },
    updateProgress: () => Updates.progress(),
    installUpdate: async () => installer !== null && (await openUrl(installer)).opened,
  };
}

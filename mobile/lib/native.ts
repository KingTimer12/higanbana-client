// O núcleo de verdade: o crate Rust do app (`src/lib.rs`), que fala com a
// nuvem, o AniList e guarda sessão e progresso.
import process from "process";
import { platform } from "nsh";
import { Account, Catalog, Library, Player } from "higanbana/native";
import type {
  Backend,
  Chapter,
  Episode,
  EpisodeInfo,
  Media,
  PlayerSettings,
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
  return {
    user: () => Account.user() as Promise<User | null>,
    login: () => Account.login() as Promise<User>,
    loginUrl: () => Account.loginUrl(),
    cancelLogin: () => Account.cancelLogin(),
    logout: () => Account.logout(),

    season: () => Catalog.season() as Promise<Media[]>,
    find: (media) => Catalog.find(media),
    media: (id) => Catalog.media(id) as Promise<Media>,
    lookup: (names) => Catalog.lookup(names) as Promise<(Media | null)[]>,
    search: (query) => Catalog.search(query) as Promise<WorkEntry[]>,
    letters: () => Catalog.letters(),
    letter: (letter) => Catalog.letter(letter) as Promise<WorkEntry[]>,

    open: (path) => Library.open(path) as Promise<Work>,
    episodes: (path) => Library.episodes(path) as Promise<Episode[]>,
    episodeInfo: (media, folder, files) => Library.tmdbEpisodes(media.id, folder, files) as Promise<(EpisodeInfo | null)[]>,
    rememberVersion: (workPath, folder) => Library.rememberVersion(workPath, folder),

    streamUrl: (path, size) => Player.streamUrl(path, size),
    settings: () => Player.settings() as PlayerSettings,
    saveProgress: (path, position, duration, endingStart) => Player.saveProgress(path, position, duration, endingStart),
    pickTracks: (tracks: Track[], dir) => Player.pickTracks(tracks, dir) as TrackPick,
    rememberTracks: (tracks, dir, sub, audio) => Player.rememberTracks(tracks, dir, sub, audio),
    skipTimes: (chapters: Chapter[], duration, malId, episode) =>
      Player.skipTimes(chapters, duration, malId, episode) as Promise<SkipTimes>,
    continueWatching: () => Player.continueWatching() as Resume[],
    setPlaying: (playing) => Player.setPlaying(playing),
  };
}

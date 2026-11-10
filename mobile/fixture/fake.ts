// Um núcleo de mentira, com um acervo pequeno e fixo: para os testes das
// telas e para olhar o app no desktop sem conta nem rede da nuvem.
import type { Backend, Episode, Media, User, Work } from "../lib/backend";

const USER: User = { name: "Aaron", avatar: null };
const SAMPLE_VIDEO = "https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4";

function media(id: number, title: string, extra: Partial<Media> = {}): Media {
  return {
    id,
    idMal: id,
    format: "TV",
    title: { userPreferred: title },
    coverImage: { large: `https://picsum.photos/seed/higanbana-${id}/300/450` },
    averageScore: 80,
    episodes: 12,
    seasonYear: 2026,
    genres: ["Ação", "Fantasia"],
    ...extra,
  };
}

const SEASON: Media[] = [
  media(1, "Sousou no Frieren", { bannerImage: "https://picsum.photos/seed/higanbana-banner/1260/540" }),
  media(2, "Dandadan"),
  media(3, "Kusuriya no Hitorigoto"),
  media(4, "Fora do Acervo"),
  media(5, "Spy x Family"),
  media(6, "Bocchi the Rock!"),
];

const PATHS: Record<number, string> = {
  1: "Animes/Letra S/Sousou no Frieren",
  2: "Animes/Letra D/Dandadan",
  3: "Animes/Letra K/Kusuriya no Hitorigoto",
  5: "Animes/Letra S/Spy x Family",
  6: "Animes/Letra B/Bocchi the Rock!",
};

function episodesOf(dir: string, count = 3): Episode[] {
  return Array.from({ length: count }, (_, i) => {
    const name = `Ep ${String(i + 1).padStart(2, "0")} [1080p].mkv`;
    return {
      name,
      path: `${dir}/${name}`,
      size: 1,
      number: i + 1,
      label: `Ep ${i + 1}`,
      quality: "1080p",
      position: i === 0 ? 600 : 0,
      duration: i === 0 ? 1420 : 0,
      watched: false,
    };
  });
}

function work(path: string): Work {
  const title = path.split("/").pop() ?? path;
  const version = (folder: string, resolution: string) => ({
    folder,
    path: `${path}/${folder}`,
    name: "Grupo",
    resolution,
    source: "BD",
    audio: null,
  });
  if (path.endsWith("/Spy x Family")) {
    return {
      path,
      title,
      seasons: [
        { name: "[2022] Spy x Family", path: `${path}/[2022] Spy x Family`, title: "Spy x Family", year: 2022 },
        { name: "[2023] Spy x Family Season 2", path: `${path}/[2023] Spy x Family Season 2`, title: "Spy x Family Season 2", year: 2023 },
      ],
      versions: [],
      defaultVersion: null,
      episodes: [],
    };
  }
  if (path.includes("Frieren")) {
    return {
      path,
      title,
      seasons: [],
      versions: [version("[Grupo] Frieren [BD 1080p]", "1080p"), version("[Grupo] Frieren [BD 720p]", "720p")],
      defaultVersion: "[Grupo] Frieren [BD 1080p]",
      episodes: [],
    };
  }
  return { path, title, seasons: [], versions: [], defaultVersion: null, episodes: episodesOf(path) };
}

/** Um backend com dados fixos. `signedIn: false` começa na tela de login. */
export function fakeBackend(options: { signedIn: boolean }): Backend {
  let user: User | null = options.signedIn ? USER : null;
  const saved: Record<string, { position: number; duration: number }> = {};
  return {
    user: async () => user,
    login: async () => (user = USER),
    loginUrl: async () => null,
    cancelLogin: async () => {},
    logout: async () => {
      user = null;
    },

    season: async () => SEASON,
    find: async (m) => PATHS[m.id] ?? null,
    media: async (id) => ({
      ...(SEASON.find((m) => m.id === id) ?? media(id, "Anime")),
      description: "Uma elfa maga e seus companheiros seguem viagem depois do fim da aventura.<br><br>O tempo passa diferente para ela.",
    }),
    lookup: async (names) => names.map((name) => SEASON.find((m) => name.includes(m.title.userPreferred ?? "?")) ?? null),
    search: async (query) =>
      Object.values(PATHS)
        .filter((path) => path.toLowerCase().includes(query.toLowerCase()))
        .map((path) => ({ name: path.split("/").pop() ?? path, path, title: path.split("/").pop() ?? path, year: 2026 })),
    letters: () => ["0-9", "Letra A", "Letra B", "Letra D", "Letra K", "Letra S"],
    letter: async (letter) =>
      Object.values(PATHS)
        .filter((path) => path.split("/")[1] === letter)
        .map((path) => ({ name: path.split("/").pop() ?? path, path, title: path.split("/").pop() ?? path, year: 2026 })),

    open: async (path) => work(path),
    episodes: async (path) => episodesOf(path),
    rememberVersion: () => {},

    streamUrl: () => SAMPLE_VIDEO,
    settings: () => ({ autoNext: true, countdownSec: 5, showSkipIntro: true }),
    saveProgress: (path, position, duration) => {
      saved[path] = { position, duration };
      return duration > 0 && position >= duration * 0.9;
    },
    pickTracks: () => ({ sub: null, audio: null }),
    rememberTracks: () => {},
    skipTimes: async () => ({ op: null, ed: null }),
    continueWatching: () => [
      {
        path: "Animes/Letra D/Dandadan/Ep 01 [1080p].mkv",
        dir: "Animes/Letra D/Dandadan",
        name: "Ep 01 [1080p].mkv",
        title: "Dandadan",
        label: "Ep. 1",
        position: 600,
        duration: 1420,
      },
    ],
    setPlaying: () => {},
  };
}

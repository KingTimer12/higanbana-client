// Um núcleo de mentira, com um acervo pequeno e fixo: para os testes das
// telas e para olhar o app no desktop sem conta nem rede da nuvem.
import type { AppUpdate, Backend, Comic, Episode, FranchiseTab, Media, ReadProgress, User, Work, WorkEntry } from "../lib/backend";

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

const MANGAS: Media[] = [
  media(101, "Chainsaw Man", { format: "MANGA", episodes: null, chapters: 97 }),
  media(102, "Yotsuba to!", { format: "MANGA", episodes: null, volumes: 15 }),
];

const MANGA_PATHS: Record<number, string> = {
  101: "Mangás/Séries Mangás/Letra C/Chainsaw Man",
  102: "Mangás/Séries Mangás/Letra Y/Yotsuba to!",
};

/** `[2023.1] Título [FILME]` → ano, ordem e tipo, como o `parse_work_name` do núcleo. */
function parseWork(name: string): { year: number | null; order: number; kind: string | null } {
  const year = /^\[(\d{4})(?:\.(\d+))?\]/.exec(name);
  const kind = /\[(TV|FILME|MOVIE|OVA|ONA|ESPECIAL)\]\s*$/i.exec(name);
  return { year: year ? Number(year[1]) : null, order: year?.[2] ? Number(year[2]) : 0, kind: kind ? kind[1].toUpperCase() : null };
}

/** As abas da franquia como o núcleo as monta (`franchise.rs`), para o acervo de mentira. */
function franchiseTabs(names: string[], formats: (string | null)[]): FranchiseTab[] {
  const tabOf = (i: number) => {
    const w = parseWork(names[i]);
    if (w.year === null) return "others";
    const f = formats[i];
    const kind = f === "MOVIE" ? "FILME" : f === "OVA" ? "OVA" : f === "SPECIAL" || f === "MUSIC" ? "ESPECIAL" : f ? "TV" : w.kind;
    return kind === "FILME" || kind === "MOVIE" ? "movies" : kind === "OVA" || kind === "ESPECIAL" ? "specials" : "tv";
  };
  const labels: Record<string, string> = { tv: "Séries TV", movies: "Filmes", specials: "Especiais", others: "Outros" };
  return ["tv", "movies", "specials", "others"]
    .map((id) => {
      const works = names.map((_, i) => i).filter((i) => tabOf(i) === id);
      works.sort((a, b) => (parseWork(names[a]).year ?? 0) - (parseWork(names[b]).year ?? 0) || parseWork(names[a]).order - parseWork(names[b]).order);
      return { id, label: id === "others" ? "Outros" : `${labels[id]} · ${works.length}`, works };
    })
    .filter((t) => t.works.length > 0);
}

/** Uma obra da franquia de mentira. */
function season(path: string, name: string): WorkEntry {
  const w = parseWork(name);
  return { name, path: `${path}/${name}`, title: name.replace(/^\[[^\]]*\]\s*/, "").replace(/\s*\[[^\]]*\]$/, ""), year: w.year };
}

function chaptersOf(dir: string, read: Record<string, ReadProgress>): Comic[] {
  // Yotsuba vem numa coletânea: um arquivo com três capítulos dentro.
  const names = dir.includes("Yotsuba") ? ["Caps. 1-3.zip"] : ["Cap 01.cbz", "Cap 02.cbz", "Cap 03.cbz"];
  return names.map((name, i) => {
    const path = `${dir}/${name}`;
    const seen = read[path];
    return {
      name,
      path,
      size: 1,
      label: name.startsWith("Caps") ? "Caps. 1-3" : `Cap. ${i + 1}`,
      page: seen?.page ?? 0,
      total: seen?.total ?? 0,
      read: seen?.read ?? false,
      unitName: null,
    };
  });
}

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

function work(path: string, read: Record<string, ReadProgress>): Work {
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
        season(path, "[2022] Spy x Family [TV]"),
        season(path, "[2023.1] Spy x Family Code White [FILME]"),
        season(path, "[2023] Spy x Family Season 2 [TV]"),
      ],
      span: "Franquia · 2022–2023",
      versions: [],
      defaultVersion: null,
      episodes: [],
      chapters: [],
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
      chapters: [],
    };
  }
  if (path.startsWith("Mangás/")) {
    return { path, title, seasons: [], versions: [], defaultVersion: null, episodes: [], chapters: chaptersOf(path, read) };
  }
  return { path, title, seasons: [], versions: [], defaultVersion: null, episodes: episodesOf(path), chapters: [] };
}


/** Uma versão nova de mentira, oferecida quando o fixture pede (`update: true`). */
const UPDATE: AppUpdate = { version: "0.2.0", notes: "- Mangás no celular", size: 64 * 1024 * 1024 };

/** Um backend com dados fixos. `signedIn: false` começa na tela de login; `update: true` oferece uma versão nova. */
export function fakeBackend(options: { signedIn: boolean; update?: boolean }): Backend {
  let user: User | null = options.signedIn ? USER : null;
  let downloaded = 0;
  const saved: Record<string, { position: number; duration: number }> = {};
  const read: Record<string, ReadProgress> = {};
  const entry = (path: string): WorkEntry => ({ name: path.split("/").pop() ?? path, path, title: path.split("/").pop() ?? path, year: 2026 });
  return {
    user: async () => user,
    login: async () => (user = USER),
    loginUrl: async () => null,
    cancelLogin: async () => {},
    logout: async () => {
      user = null;
    },

    // As vitrines só trazem o que está no acervo; cada ranking em outra ordem.
    discover: async (ranking, n) => {
      const order = { popular: [1, 2, 3, 5, 6], topRated: [3, 1, 6, 2, 5], trending: [5, 6, 2] }[ranking];
      return order.slice(0, n).map((id) => ({
        media: { ...SEASON.find((m) => m.id === id)!, tags: ["Magia", "Jornada"], studio: "Madhouse" },
        path: PATHS[id],
      }));
    },
    find: async (m) => PATHS[m.id] ?? null,
    cover: async (m) => {
      return null;
    },
    media: async (id) => ({
      ...(SEASON.find((m) => m.id === id) ?? media(id, "Anime")),
      description: "Uma elfa maga e seus companheiros seguem viagem depois do fim da aventura.<br><br>O tempo passa diferente para ela.",
    }),
    // O filme da franquia casa com um MOVIE do AniList: vai para a aba Filmes.
    lookup: async (names, kind) =>
      names.map((name) =>
        kind === "manga"
          ? (MANGAS.find((m) => name.includes(m.title.userPreferred ?? "?")) ?? null)
          : name.includes("Code White")
            ? media(7, "Spy x Family Code: White", { format: "MOVIE", episodes: 1, seasonYear: 2023 })
            : (SEASON.find((m) => name.includes(m.title.userPreferred ?? "?")) ?? null),
      ),
    search: async (query, kind) =>
      Object.values(kind === "manga" ? MANGA_PATHS : PATHS)
        .filter((path) => path.toLowerCase().includes(query.toLowerCase()))
        .map(entry),
    letters: () => ["0-9", "Letra A", "Letra B", "Letra C", "Letra D", "Letra K", "Letra S", "Letra Y"],
    letter: async (letter, kind) =>
      Object.values(kind === "manga" ? MANGA_PATHS : PATHS)
        .filter((path) => path.split("/").includes(letter))
        .map(entry),

    open: async (path) => work(path, read),
    episodes: async (path) => episodesOf(path),
    chapters: async (path) => chaptersOf(path, read),
    franchiseTabs,
    // O primeiro episódio casa com o TMDB; os outros ficam com o rótulo do arquivo.
    episodeInfo: async (_media, _folder, files) =>
      files.map((_, i) =>
        i === 0
          ? {
              title: "O fim da jornada",
              overview: "O grupo do herói volta à capital depois de derrotar o Rei Demônio.",
              still: "https://picsum.photos/seed/higanbana-still/300/169",
              airDate: "2023-09-29",
              runtime: 24,
              vote: 8.7,
            }
          : null,
      ),
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

    readerUnits: async (path) =>
      path.endsWith(".zip")
        ? [1, 2, 3].map((n) => ({ key: `${path}::Cap ${n}.cbz`, name: `Cap ${n}.cbz`, label: `Cap. ${n}`, inner: `Cap ${n}.cbz`, group: null }))
        : [{ key: path, name: path.split("/").pop() ?? path, label: "Cap.", inner: null, group: null }],
    readerPages: async () => [0, 1, 2, 3],
    // A semente só com letras e números: o picsum não aceita `/` codificada no caminho.
    readerPage: async (_path, _size, unit, index) =>
      `https://picsum.photos/seed/${`${unit.key}${index}`.replace(/[^A-Za-z0-9]/g, "")}/700/1000`,
    saveReading: (key, page, total) => {
      read[key] = { page, total, read: (read[key]?.read ?? false) || page + 1 >= total };
    },
    saveReadingSummary: (path, index, count, done) => {
      read[path] = { page: index, total: count, read: (read[path]?.read ?? false) || (done && index + 1 >= count) };
    },
    readingOf: (key) => read[key] ?? null,

    version: () => "0.1.0",
    checkUpdate: async () => (options.update ? UPDATE : null),
    downloadUpdate: async () => {
      downloaded = 1;
    },
    updateProgress: () => downloaded,
    installUpdate: async () => downloaded === 1,
  };
}

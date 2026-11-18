import { computed, effect, Image, List, resource, Scroll, signal, Text, token, View } from "nsh";
import { Button, NavPage } from "nsh/base";
import { CHAPTER_HEIGHT, ChapterRow, inProgress, whereStopped } from "../components/Chapter";
import { EPISODE_HEIGHT, EpisodeCard } from "../components/Episode";
import { Chip, Cover, EmptyState, Loading, PageHeader, PrimaryButton, SectionTitle, SYSTEM_HEADER } from "../components/ui";
import { backend, type Comic, type Episode, type EpisodeInfo, type Media, type MediaKind, type Version, type Work } from "../lib/backend";
import { baseName, errorMessage, mediaCover, mediaFacts, mediaTitle, plainText } from "../lib/format";
import { coverOf, nav, readingOf, watchedOf, wantCover } from "../store";
import { Franchise } from "./Franchise";

/** Sinopse maior que isso começa recolhida, com "Mostrar mais". */
const LONG_SYNOPSIS = 240;

/** `1080p · BD · Dual`, com o que a versão tem. */
function versionLabel(version: Version): string {
  return [version.name, version.resolution, version.source, version.audio].filter(Boolean).join(" · ");
}

/** O episódio com o progresso desta sessão por cima do que veio da nuvem. */
function withProgress(episode: Episode): Episode {
  const seen = watchedOf(episode.path);
  return seen ? { ...episode, ...seen } : episode;
}

/** O capítulo com a leitura desta sessão por cima do que veio do disco. */
function withReading(chapter: Comic): Comic {
  const seen = readingOf(chapter.path);
  return seen ? { ...chapter, ...seen } : chapter;
}

/** Onde retomar: o primeiro episódio não visto (ou o primeiro, se todos foram vistos). */
function nextToWatch(episodes: Episode[]): Episode | undefined {
  return episodes.find((e) => !e.watched) ?? episodes[0];
}

/** Onde retomar a leitura: o primeiro capítulo não lido (ou o primeiro, se todos foram lidos). */
function nextToRead(chapters: Comic[]): Comic | undefined {
  return chapters.find((c) => !c.read) ?? chapters[0];
}

/** A página de uma obra. Franquia vira a grade das obras dela (`Franchise`);
 * o resto mostra capa e sinopse, versões e os episódios (anime) ou capítulos (mangá). */
export function Title(props: { path: string; media: Media | null; kind: MediaKind }) {
  const root = resource(() => backend().open(props.path));
  const franchise = () => {
    const w = root();
    return w && w.seasons.length > 0 ? w : null;
  };
  const heading = () => franchise()?.title ?? root()?.title ?? baseName(props.path);

  return (
    <NavPage title={heading()} headerClass={SYSTEM_HEADER} header={<PageHeader title={heading()} onBack={() => nav.pop()} />}>
      <View class="flex-1 bg-background">
        <If when={root.state() === "pending"}>
          <Loading label="Abrindo a obra" />
        </If>
        <If when={root.state() === "errored"}>
          <EmptyState title="Não foi possível abrir a obra" detail={errorMessage(root.error())} action="Tentar de novo" onAction={root.refetch} />
        </If>
        <If when={root.state() === "ready" || root.state() === "refreshing"}>
          <If when={franchise()}>{(w) => <Franchise work={w()} kind={props.kind} />}</If>
          <If when={franchise() ? undefined : root()}>{(w) => <WorkBody work={w()} media={props.media} kind={props.kind} />}</If>
        </If>
      </View>
    </NavPage>
  );
}

/** Uma obra aberta (não franquia): o cabeçalho e a lista de episódios ou capítulos. */
function WorkBody(props: { work: Work; media: Media | null; kind: MediaKind }) {
  const manga = props.kind === "manga";
  const work = () => props.work;

  // A versão (release) escolhida; sem escolha, a que o núcleo sugere.
  const chosenVersion = signal<string | null>(null);
  const versionFolder = () => chosenVersion() ?? work().defaultVersion;
  const version = () => work().versions.find((v) => v.folder === versionFolder()) ?? null;
  const releaseEpisodes = resource(
    () => (manga ? undefined : version()?.path),
    (path) => backend().episodes(path),
  );
  const releaseChapters = resource(
    () => (manga ? version()?.path : undefined),
    (path) => backend().chapters(path),
  );
  const episodesDir = () => version()?.path ?? work().path;
  // Os arquivos como vieram da nuvem; a lista os mostra e cada card aplica o próprio
  // progresso, então o player gravar um episódio só reavalia aquele card.
  const files = computed(() => (version() ? (releaseEpisodes() ?? []) : work().episodes));
  // Com o progresso de todos: só para o botão de retomar.
  const episodes = computed(() => files().map(withProgress));
  const chapterFiles = computed(() => (version() ? (releaseChapters() ?? []) : work().chapters));
  const chapters = computed(() => chapterFiles().map(withReading));

  // Metadados: o resumo que veio de quem abriu, ou o que casa com o nome da pasta.
  const matched = resource(
    () => (props.media ? undefined : work().path),
    async (path) => (await backend().lookup([baseName(path)], props.kind))[0] ?? null,
  );
  const media = () => props.media ?? matched() ?? null;
  const detail = resource(
    () => media()?.id,
    (id) => backend().media(id),
  );
  const title = () => {
    const m = media();
    return m ? mediaTitle(m) : work().title;
  };
  const synopsis = () => {
    const text = detail()?.description ?? media()?.description;
    return text ? plainText(text) : "";
  };
  // TMDB (só vídeo): título e miniatura de cada arquivo, casados como no desktop
  // (detalhe do AniList + nome da pasta). Chave em texto: a lista pode chegar de
  // novo com os mesmos arquivos, e isso não deve pedir de novo.
  const tmdbKey = () => {
    const d = detail();
    const names = files().map((e) => e.name);
    if (manga || !d || d.id !== media()?.id) return undefined;
    return names.length > 0 ? JSON.stringify([d.id, baseName(work().path), names]) : undefined;
  };
  const tmdb = resource(tmdbKey, async (key) => {
    const [, folder, names] = JSON.parse(key) as [number, string, string[]];
    try {
      const infos = await backend().episodeInfo(detail()!, folder, names);
      return new Map(names.map((name, i) => [name, infos[i] ?? null]));
    } catch (error) {
      // Sem TMDB os episódios seguem com o rótulo do arquivo; o motivo fica no log.
      console.warn(`TMDB: ${errorMessage(error)}`);
      throw error;
    }
  });
  /** `undefined` enquanto o TMDB responde; `null` sem episódio casado. */
  const infoOf = (name: string): EpisodeInfo | null | undefined => {
    const waiting = matched.state() === "pending" || detail.state() === "pending" || tmdb.state() === "pending";
    if (waiting) return undefined;
    return tmdb()?.get(name) ?? null;
  };
  // A capa do fanart (o pôster que abriu a página já a pediu; aqui cobre a entrada pelo acervo).
  effect(() => {
    const m = media();
    if (m) wantCover(m);
  });
  const expanded = signal(false);
  const backdrop = () => media()?.bannerImage ?? mediaCover(media());

  const play = (episode: Episode) =>
    nav.push({ name: "player", dir: episodesDir(), file: episode.name, title: title(), malId: media()?.idMal ?? null });
  const read = (chapter: Comic) => nav.push({ name: "reader", file: chapter, files: chapterFiles(), title: title() });
  const chooseVersion = (v: Version) => {
    chosenVersion.set(v.folder);
    backend().rememberVersion(work().path, v.folder);
  };

  const pending = () => (manga ? releaseChapters : releaseEpisodes).state() === "pending" && version() !== null;
  const failure = () => (manga ? releaseChapters : releaseEpisodes).error();
  const count = () => (manga ? chapterFiles().length : files().length);

  // A seção não muda com a página aberta: a escolha é feita uma vez, em JS.
  const resumeButton = manga ? (
    <If when={nextToRead(chapters())}>
      {(chapter) => (
        <PrimaryButton
          icon="book"
          label={inProgress(chapter()) ? `Continuar ${chapter().label} (${whereStopped(chapter())})` : `Ler ${chapter().label}`}
          onPress={() => read(chapter())}
        />
      )}
    </If>
  ) : (
    <If when={nextToWatch(episodes())}>
      {(episode) => (
        <PrimaryButton
          icon="play"
          label={
            episode().position > 0 && !episode().watched
              ? `Continuar ${episode().number !== null ? `episódio ${episode().number}` : ""}`.trim()
              : `Assistir ${episode().number !== null ? `episódio ${episode().number}` : ""}`.trim()
          }
          onPress={() => play(episode())}
        />
      )}
    </If>
  );

  const header = (
    <View class="gap-6 pb-2">
      {/* Como no desktop: o banner (ou a capa) apagado atrás, e um véu até o fundo. */}
      <View class="-mx-4 overflow-hidden">
        <If when={backdrop()}>
          {(src) => <Image src={src()} class="absolute inset-0 size-full object-cover" style={{ opacity: media()?.bannerImage ? 0.45 : 0.25 }} />}
        </If>
        <View class="absolute inset-0" style={{ backgroundImage: `linear-gradient(180deg, transparent 0%, ${token("background")} 92%)` }} />
        <View class="flex-row gap-4 px-4 pt-10 pb-2">
          <Cover src={coverOf(media())} title={title()} color={media()?.coverImage?.color} class="w-28" />
          <View class="flex-1 justify-end gap-2">
            <Text role="header" class="text-2xl font-semibold text-foreground" style={{ numberOfLines: 3 }}>
              {title()}
            </Text>
            <If when={media()}>{(m) => <Text class="text-sm text-muted-foreground">{mediaFacts(m())}</Text>}</If>
            <If when={(media()?.genres ?? []).length > 0}>
              <Text class="text-sm text-faint" style={{ numberOfLines: 1 }}>
                {(media()?.genres ?? []).slice(0, 3).join(" · ")}
              </Text>
            </If>
          </View>
        </View>
      </View>

      {resumeButton}

      <If when={synopsis()}>
        <Button onPress={() => expanded.update((e) => !e)} accessibilityLabel="Sinopse" class="gap-1 active:opacity-70">
          <Text class="text-sm leading-relaxed text-muted-foreground" style={{ numberOfLines: expanded() ? undefined : 4 }}>
            {synopsis()}
          </Text>
          <If when={synopsis().length > LONG_SYNOPSIS}>
            <Text class="text-sm font-medium text-brand-ink">{expanded() ? "Mostrar menos" : "Mostrar mais"}</Text>
          </If>
        </Button>
      </If>

      <If when={work().versions.length > 1}>
        <View class="gap-2">
          <SectionTitle>Versão</SectionTitle>
          <Scroll horizontal scrollbar={false} class="-mx-4" contentClass="flex-row gap-2 px-4">
            <Each of={work().versions} key={(v) => v.path}>
              {(v) => <Chip label={versionLabel(v())} selected={versionFolder() === v().folder} onPress={() => chooseVersion(v())} />}
            </Each>
          </Scroll>
        </View>
      </If>

      <SectionTitle
        aside={
          <If when={count() > 0}>
            <Text class="text-sm tabular-nums text-muted-foreground">{manga ? `${count()} arquivos` : `${count()} vídeos`}</Text>
          </If>
        }
      >
        {manga ? "Capítulos" : "Episódios"}
      </SectionTitle>
      <If when={pending()}>
        <View class="h-32">
          <Loading label={manga ? "Carregando capítulos" : "Carregando episódios"} />
        </View>
      </If>
      <If when={failure()}>
        {(error) => (
          <View class="h-48">
            <EmptyState title={manga ? "Não foi possível listar os capítulos" : "Não foi possível listar os episódios"} detail={errorMessage(error())} />
          </View>
        )}
      </If>
    </View>
  );

  const empty = (what: string) => (
    <If when={!pending() && !failure()}>
      <View class="h-40">
        <EmptyState title={what} />
      </View>
    </If>
  );
  const contentClass = "gap-y-2 px-4 pb-[calc(env(safe-area-inset-bottom)+24px)]";

  return manga ? (
    <List<Comic>
      each={chapterFiles()}
      keyExtractor={(chapter) => chapter.path}
      estimatedItemSize={CHAPTER_HEIGHT}
      class="flex-1"
      contentClass={contentClass}
      header={header}
      empty={empty("Nenhum capítulo nesta pasta")}
    >
      {(chapter) => <ChapterRow chapter={withReading(chapter())} onPress={() => read(chapter())} />}
    </List>
  ) : (
    <List<Episode>
      each={files()}
      keyExtractor={(episode) => episode.path}
      estimatedItemSize={EPISODE_HEIGHT}
      class="flex-1"
      contentClass={contentClass}
      header={header}
      empty={empty("Nenhum episódio nesta pasta")}
    >
      {(episode) => <EpisodeCard episode={withProgress(episode())} info={infoOf(episode().name)} onPress={() => play(episode())} />}
    </List>
  );
}

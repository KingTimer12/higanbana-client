import { computed, effect, Image, List, resource, Scroll, signal, Text, token, View } from "nsh";
import { Button, NavPage } from "nsh/base";
import { EPISODE_HEIGHT, EpisodeCard } from "../components/Episode";
import { Chip, Cover, EmptyState, Loading, PageHeader, PrimaryButton, SectionTitle } from "../components/ui";
import { backend, type Episode, type EpisodeInfo, type Media, type Version, type Work } from "../lib/backend";
import { baseName, errorMessage, mediaCover, mediaFacts, mediaTitle, plainText } from "../lib/format";
import { SYSTEM_HEADER } from "../components/ui";
import { coverOf, nav, watchedOf, wantCover } from "../store";

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

/** Onde retomar: o primeiro episódio não visto (ou o primeiro, se todos foram vistos). */
function nextToWatch(episodes: Episode[]): Episode | undefined {
  return episodes.find((e) => !e.watched) ?? episodes[0];
}

/** A página de uma obra: capa e sinopse, temporadas, versões e episódios. */
export function Title(props: { path: string; media: Media | null }) {
  const root = resource(() => backend().open(props.path));

  // Franquia: a temporada escolhida é outra pasta de obra, aberta do mesmo jeito.
  const seasonPath = signal<string | null>(null);
  const seasons = () => root()?.seasons ?? [];
  // Sem escolha, a temporada do pôster tocado (pelo ano); senão a primeira.
  const initialSeason = () =>
    seasons().find((s) => props.media?.seasonYear && s.year === props.media.seasonYear) ?? seasons()[0];
  const currentSeason = () => seasonPath() ?? initialSeason()?.path ?? null;
  const seasonWork = resource(currentSeason, (path) => backend().open(path));
  const work = (): Work | undefined => (seasons().length > 0 ? seasonWork() : root());

  // A versão (release) escolhida; sem escolha, a que o núcleo sugere.
  const chosenVersion = signal<{ work: string; folder: string } | null>(null);
  const versionFolder = () => {
    const w = work();
    if (!w) return null;
    const chosen = chosenVersion();
    return chosen && chosen.work === w.path ? chosen.folder : w.defaultVersion;
  };
  const version = () => work()?.versions.find((v) => v.folder === versionFolder()) ?? null;
  const releaseEpisodes = resource(
    () => version()?.path,
    (path) => backend().episodes(path),
  );
  const episodesDir = () => version()?.path ?? work()?.path ?? props.path;
  const episodes = computed(() => (version() ? (releaseEpisodes() ?? []) : (work()?.episodes ?? [])).map(withProgress));

  // Metadados: o resumo que veio da temporada, ou o que casa com o nome da pasta.
  // Numa franquia cada temporada casa com a própria entrada do AniList, como no
  // desktop: o pôster tocado é de uma temporada só.
  const seasonal = () => seasons().length > 0;
  const matched = resource(
    () => (props.media && !seasonal() ? undefined : work()?.path),
    async (path) => (await backend().lookup([baseName(path)]))[0] ?? null,
  );
  const media = () => (seasonal() ? matched() : props.media) ?? props.media ?? matched() ?? null;
  /** A mídia que é exatamente a pasta aberta (a do TMDB): na franquia, só a casada pela temporada. */
  const folderMedia = () => (seasonal() ? matched() : media()) ?? null;
  const detail = resource(
    () => media()?.id,
    (id) => backend().media(id),
  );
  const title = () => {
    const m = media();
    return m ? mediaTitle(m) : (work()?.title ?? root()?.title ?? baseName(props.path));
  };
  const synopsis = () => {
    const text = detail()?.description ?? media()?.description;
    return text ? plainText(text) : "";
  };
  // TMDB: título e miniatura de cada arquivo, casados como no desktop (detalhe do
  // AniList + nome da pasta). Chave em texto: o progresso muda `episodes()` sem
  // mudar os arquivos, e isso não deve pedir de novo.
  const tmdbKey = () => {
    const d = detail();
    const w = work();
    const names = episodes().map((e) => e.name);
    if (!d || d.id !== folderMedia()?.id) return undefined;
    return w && names.length > 0 ? JSON.stringify([d.id, baseName(w.path), names]) : undefined;
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
  const chooseVersion = (v: Version) => {
    const w = work();
    if (!w) return;
    chosenVersion.set({ work: w.path, folder: v.folder });
    backend().rememberVersion(w.path, v.folder);
  };

  const loadingEpisodes = () =>
    seasonWork.state() === "pending" || (version() !== null && releaseEpisodes.state() === "pending");
  const failure = () => seasonWork.error() ?? releaseEpisodes.error();

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

      <If when={seasons().length > 0}>
        <View class="gap-2">
          <SectionTitle>Temporadas</SectionTitle>
          <Scroll horizontal scrollbar={false} class="-mx-4" contentClass="flex-row gap-2 px-4">
            <Each of={seasons()} key={(s) => s.path}>
              {(s) => (
                <Chip
                  label={s().year ? `${s().title} (${s().year})` : s().title}
                  selected={currentSeason() === s().path}
                  onPress={() => seasonPath.set(s().path)}
                />
              )}
            </Each>
          </Scroll>
        </View>
      </If>

      <If when={(work()?.versions.length ?? 0) > 1}>
        <View class="gap-2">
          <SectionTitle>Versão</SectionTitle>
          <Scroll horizontal scrollbar={false} class="-mx-4" contentClass="flex-row gap-2 px-4">
            <Each of={work()?.versions ?? []} key={(v) => v.path}>
              {(v) => <Chip label={versionLabel(v())} selected={versionFolder() === v().folder} onPress={() => chooseVersion(v())} />}
            </Each>
          </Scroll>
        </View>
      </If>

      <SectionTitle aside={<If when={episodes().length > 0}><Text class="text-sm tabular-nums text-muted-foreground">{episodes().length} vídeos</Text></If>}>
        Episódios
      </SectionTitle>
      <If when={loadingEpisodes()}>
        <View class="h-32">
          <Loading label="Carregando episódios" />
        </View>
      </If>
      <If when={failure()}>
        {(error) => (
          <View class="h-48">
            <EmptyState title="Não foi possível listar os episódios" detail={errorMessage(error())} />
          </View>
        )}
      </If>
    </View>
  );

  return (
    <NavPage title={title()} headerClass={SYSTEM_HEADER} header={<PageHeader title={title()} onBack={() => nav.pop()} />}>
      <View class="flex-1 bg-background">
      <If when={root.state() === "pending"}>
        <Loading label="Abrindo a obra" />
      </If>
      <If when={root.state() === "errored"}>
        <EmptyState title="Não foi possível abrir a obra" detail={errorMessage(root.error())} action="Tentar de novo" onAction={root.refetch} />
      </If>
      <If when={root.state() === "ready" || root.state() === "refreshing"}>
        <List<Episode>
          each={episodes()}
          keyExtractor={(episode) => episode.path}
          numColumns={2}
          estimatedItemSize={EPISODE_HEIGHT}
          class="flex-1"
          contentClass="gap-x-3 gap-y-6 px-4 pb-[calc(env(safe-area-inset-bottom)+24px)]"
          header={header}
          empty={
            <If when={!loadingEpisodes() && !failure()}>
              <View class="h-40">
                <EmptyState title="Nenhum episódio nesta pasta" />
              </View>
            </If>
          }
        >
          {(episode) => <EpisodeCard episode={episode()} info={infoOf(episode().name)} onPress={() => play(episode())} />}
        </List>
      </If>
      </View>
    </NavPage>
  );
}

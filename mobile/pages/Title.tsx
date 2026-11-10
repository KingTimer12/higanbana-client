import { computed, Image, List, resource, Scroll, signal, Text, View } from "nsh";
import { Button, NavPage } from "nsh/base";
import { Chip, Cover, EmptyState, Icon, Loading, PageHeader, PrimaryButton, SectionTitle, WatchBar } from "../components/ui";
import { backend, type Episode, type Media, type Version, type Work } from "../lib/backend";
import { baseName, errorMessage, formatTime, mediaCover, mediaFacts, mediaTitle, plainText } from "../lib/format";
import { SYSTEM_HEADER } from "../components/ui";
import { nav, watchedOf } from "../store";

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

function EpisodeRow(props: { episode: Episode; onPress: () => void }) {
  const inProgress = () => !props.episode.watched && props.episode.position > 0 && props.episode.duration > 0;
  const detail = () => {
    if (props.episode.watched) return "Visto";
    if (inProgress()) return `Faltam ${formatTime(props.episode.duration - props.episode.position)}`;
    return props.episode.quality ?? "";
  };
  return (
    <Button
      onPress={props.onPress}
      testID={`episode-${props.episode.name}`}
      class="min-h-16 flex-row items-center gap-3 border-b border-border px-4 py-3 active:bg-accent"
    >
      <View class="size-11 items-center justify-center rounded-(--radius) bg-secondary">
        <If when={props.episode.watched} else={<Text class="text-base font-semibold tabular-nums text-secondary-foreground">{props.episode.number ?? "•"}</Text>}>
          <Icon name="check" class="size-5 tint-success" />
        </If>
      </View>
      <View class="flex-1 gap-1">
        <Text class="text-base text-foreground" classList={{ "text-muted-foreground": props.episode.watched }} style={{ numberOfLines: 2 }}>
          {props.episode.number !== null ? `Episódio ${props.episode.number}` : props.episode.label}
        </Text>
        <If when={detail()}>
          <Text class="text-sm text-muted-foreground" style={{ numberOfLines: 1 }}>
            {detail()}
          </Text>
        </If>
        <If when={inProgress()}>
          <WatchBar value={(props.episode.position / props.episode.duration) * 100} class="mt-1" />
        </If>
      </View>
      <Icon name="play" class="size-5 tint-faint" />
    </Button>
  );
}

/** A página de uma obra: capa e sinopse, temporadas, versões e episódios. */
export function Title(props: { path: string; media: Media | null }) {
  const root = resource(() => backend().open(props.path));

  // Franquia: a temporada escolhida é outra pasta de obra, aberta do mesmo jeito.
  const seasonPath = signal<string | null>(null);
  const seasons = () => root()?.seasons ?? [];
  const currentSeason = () => seasonPath() ?? seasons()[0]?.path ?? null;
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
  const matched = resource(
    () => (props.media ? undefined : work()?.path),
    async (path) => (await backend().lookup([baseName(path)]))[0] ?? null,
  );
  const media = () => props.media ?? matched() ?? null;
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
  const expanded = signal(false);

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
    <View class="gap-5 pb-2">
      <If when={media()?.bannerImage}>
        {(banner) => <Image src={banner()} class="aspect-21/9 w-full bg-card object-cover" />}
      </If>
      <View class="flex-row gap-4 px-4 pt-4">
        <Cover src={mediaCover(media())} title={title()} class="w-28" />
        <View class="flex-1 justify-end gap-2">
          <Text role="header" class="text-2xl font-bold text-foreground" style={{ numberOfLines: 3 }}>
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
            class="mx-4"
          />
        )}
      </If>

      <If when={synopsis()}>
        <Button onPress={() => expanded.update((e) => !e)} accessibilityLabel="Sinopse" class="gap-1 px-4 active:opacity-70">
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
          <SectionTitle class="px-4">Temporadas</SectionTitle>
          <Scroll horizontal scrollbar={false} contentClass="flex-row gap-2 px-4">
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
          <SectionTitle class="px-4">Versão</SectionTitle>
          <Scroll horizontal scrollbar={false} contentClass="flex-row gap-2 px-4">
            <Each of={work()?.versions ?? []} key={(v) => v.path}>
              {(v) => <Chip label={versionLabel(v())} selected={versionFolder() === v().folder} onPress={() => chooseVersion(v())} />}
            </Each>
          </Scroll>
        </View>
      </If>

      <SectionTitle class="px-4">Episódios</SectionTitle>
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
          estimatedItemSize={72}
          class="flex-1"
          contentClass="pb-[calc(env(safe-area-inset-bottom)+16px)]"
          header={header}
          empty={
            <If when={!loadingEpisodes() && !failure()}>
              <View class="h-40">
                <EmptyState title="Nenhum episódio nesta pasta" />
              </View>
            </If>
          }
        >
          {(episode) => <EpisodeRow episode={episode()} onPress={() => play(episode())} />}
        </List>
      </If>
      </View>
    </NavPage>
  );
}

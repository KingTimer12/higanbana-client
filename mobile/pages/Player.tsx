import { cleanup, computed, effect, lifecycle, resource, Scroll, signal, Text, untracked, View } from "nsh";
import { Button, NavPage, Progress, Sheet, Slider } from "nsh/base";
import { Icon, IconButton, type IconName, OutlineButton } from "../components/ui";
import { hasMpv, type VideoHandle, type VideoLoad, VideoSurface } from "../components/VideoSurface";
import { backend, type Episode, type SkipTimes, type Track } from "../lib/backend";
import { errorMessage, formatTime } from "../lib/format";
import { nav, recordWatched, watchedOf } from "../store";
import { refreshResume } from "./Home";

/** Quanto os botões e o toque duplo pulam, em segundos. */
const SEEK_STEP = 10;
/** Tempo parado até os controles sumirem, com o vídeo tocando. */
const CONTROLS_TIMEOUT_MS = 3500;
/** Janela do toque duplo. */
const DOUBLE_TAP_MS = 300;
/** De quanto em quanto tempo a posição vai para o disco. */
const SAVE_EVERY_MS = 5000;
/** Menos que isso do começo ou do fim não vale retomar. */
const RESUME_MARGIN_SEC = 30;

const NO_SKIP: SkipTimes = { op: null, ed: null };

/** O canto do que flutua sobre o vídeo: dentro da área segura e acima da barra de posição. Só os filhos recebem toques. */
const CORNER =
  "absolute inset-0 items-end justify-end pr-[calc(env(safe-area-inset-right)+16px)] pb-[calc(env(safe-area-inset-bottom)+88px)]";

/** Onde um episódio recomeça: de onde parou, se parou no meio. */
function resumeAt(episode: Episode): number {
  const seen = watchedOf(episode.path) ?? episode;
  const middle = seen.position >= RESUME_MARGIN_SEC && seen.position < seen.duration - RESUME_MARGIN_SEC;
  return !seen.watched && middle ? seen.position : 0;
}

function episodeLabel(episode: Episode): string {
  return episode.number !== null ? `Episódio ${episode.number}` : episode.label;
}

function trackName(track: Track): string {
  return track.title || track.lang?.toUpperCase() || `Faixa ${track.id}`;
}

/** Um botão redondo sobre o vídeo. */
function VideoButton(props: { icon: IconName; label: string; onPress: () => void; large?: boolean }) {
  return (
    <Button
      onPress={props.onPress}
      accessibilityLabel={props.label}
      class="items-center justify-center rounded-full active:bg-on-video/20"
      classList={{ "size-20": !!props.large, "size-14": !props.large }}
    >
      <Icon name={props.icon} class={props.large ? "size-11 tint-on-video" : "size-7 tint-on-video"} />
    </Button>
  );
}

/** Uma escolha de faixa na folha de legendas e áudio. */
function TrackRow(props: { label: string; selected: boolean; onPress: () => void }) {
  return (
    <Button
      onPress={props.onPress}
      selected={props.selected}
      class="min-h-12 flex-row items-center gap-3 rounded-(--radius) px-3 py-2 active:bg-accent"
    >
      <Text class="flex-1 text-base text-popover-foreground" style={{ numberOfLines: 1 }}>
        {props.label}
      </Text>
      <If when={props.selected}>
        <Icon name="check" class="size-5 tint-primary" />
      </If>
    </Button>
  );
}

/** O player: vídeo em tela cheia, controles por toque, faixas, pular abertura e próximo episódio. */
export function Player(props: { dir: string; file: string; title: string; malId: number | null }) {
  const core = backend();
  const settings = core.settings();
  const episodes = resource(() => core.episodes(props.dir));

  // O arquivo que toca; muda ao ir para o próximo episódio.
  const playing = signal(props.file);
  const current = computed(() => episodes()?.find((e) => e.name === playing()) ?? null);
  const next = computed(() => {
    const list = episodes() ?? [];
    const at = list.findIndex((e) => e.name === playing());
    return at >= 0 ? (list[at + 1] ?? null) : null;
  });

  const paused = signal(false);
  const position = signal(0);
  const duration = signal(0);
  const buffering = signal(true);
  const failure = signal<string | null>(null);
  const tracks = signal<Track[]>([]);
  const sub = signal<number | "off" | null>(null);
  const audio = signal<number | null>(null);
  const skip = signal<SkipTimes>(NO_SKIP);
  /** A posição que o dedo arrasta na barra, enquanto arrasta. */
  const scrub = signal<number | null>(null);
  const controls = signal(true);
  const tracksOpen = signal(false);
  const nextDismissed = signal(false);

  let video: VideoHandle | undefined;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let tapTimer: ReturnType<typeof setTimeout> | undefined;
  let lastTap: { side: "left" | "right"; at: number } | undefined;
  let savedAt = 0;

  // --- Progresso ---

  const save = () => {
    const episode = untracked(current);
    const [at, length] = [untracked(position), untracked(duration)];
    if (!episode || length <= 0) return;
    savedAt = Date.now();
    const watched = core.saveProgress(episode.path, at, length, untracked(skip).ed?.start ?? null);
    recordWatched(episode.path, { position: at, duration: length, watched });
  };

  core.setPlaying(true);
  cleanup(() => {
    clearTimeout(hideTimer);
    clearTimeout(tapTimer);
    save();
    core.setPlaying(false);
    refreshResume();
  });

  // O app saiu da frente (outra tela, tela apagada): pausa e guarda onde parou.
  effect(() => {
    if (lifecycle() === "active") return;
    paused.set(true);
    save();
  });

  // --- Controles ---

  const keepControls = () => {
    clearTimeout(hideTimer);
    controls.set(true);
    hideTimer = setTimeout(() => {
      if (!untracked(paused) && !untracked(tracksOpen) && untracked(scrub) === null) controls.set(false);
    }, CONTROLS_TIMEOUT_MS);
  };
  const toggleControls = () => {
    if (controls()) {
      clearTimeout(hideTimer);
      controls.set(false);
    } else keepControls();
  };
  keepControls();

  const seekTo = (seconds: number) => {
    const to = Math.min(Math.max(seconds, 0), Math.max(duration() - 1, 0));
    position.set(to);
    video?.seek(to);
  };
  const seekBy = (delta: number) => {
    seekTo(position() + delta);
    keepControls();
  };
  const togglePaused = () => {
    paused.update((p) => !p);
    if (paused()) save();
    keepControls();
  };

  /** Um toque mostra ou esconde os controles; dois do mesmo lado pulam 10 s. */
  const tap = (side: "left" | "right") => {
    const now = Date.now();
    clearTimeout(tapTimer);
    if (lastTap && lastTap.side === side && now - lastTap.at < DOUBLE_TAP_MS) {
      seekBy(side === "left" ? -SEEK_STEP : SEEK_STEP);
    } else {
      tapTimer = setTimeout(toggleControls, DOUBLE_TAP_MS);
    }
    lastTap = { side, at: now };
  };

  // --- Arquivo ---

  const play = (episode: Episode) => {
    save();
    position.set(0);
    duration.set(0);
    buffering.set(true);
    failure.set(null);
    tracks.set([]);
    sub.set(null);
    audio.set(null);
    skip.set(NO_SKIP);
    nextDismissed.set(false);
    paused.set(false);
    playing.set(episode.name);
    keepControls();
  };

  const onLoad = (e: VideoLoad) => {
    const episode = current();
    duration.set(e.duration);
    buffering.set(false);
    tracks.set(e.tracks);
    if (e.tracks.length > 0) {
      const pick = core.pickTracks(e.tracks, props.dir);
      sub.set(pick.sub);
      audio.set(pick.audio);
    }
    if (!episode) return;
    core
      .skipTimes(e.chapters, e.duration, props.malId, episode.number)
      .then((times) => {
        // O arquivo pode ter trocado enquanto a resposta vinha.
        if (playing() === episode.name) skip.set(times);
      })
      .catch(() => {});
  };

  const onProgress = (e: { position: number; duration: number }) => {
    if (scrub() === null) position.set(e.position);
    if (e.duration > 0) duration.set(e.duration);
    if (Date.now() - savedAt >= SAVE_EVERY_MS) save();
  };

  const onEnd = () => {
    position.set(duration());
    save();
    const following = next();
    if (settings.autoNext && following && !nextDismissed()) play(following);
    else {
      paused.set(true);
      keepControls();
    }
  };

  // --- Faixas ---

  const subs = () => tracks().filter((t) => t.type === "sub");
  const audios = () => tracks().filter((t) => t.type === "audio");
  const activeSub = (): number | "off" => sub() ?? subs().find((t) => t.selected)?.id ?? "off";
  const activeAudio = (): number | null => audio() ?? audios().find((t) => t.selected)?.id ?? null;
  const choose = (nextSub: number | "off", nextAudio: number | null) => {
    sub.set(nextSub);
    audio.set(nextAudio);
    core.rememberTracks(tracks(), props.dir, nextSub === "off" ? null : nextSub, nextAudio);
  };

  // --- O que aparece sobre o vídeo ---

  const shown = () => scrub() ?? position();
  const inOpening = () => {
    const op = skip().op;
    return settings.showSkipIntro && op !== null && position() >= op.start && position() < op.end - 1;
  };
  const nextOffered = () => {
    const length = duration();
    if (!next() || nextDismissed() || length <= 0) return false;
    return position() >= (skip().ed?.start ?? length - settings.countdownSec);
  };
  const close = () => nav.pop();

  return (
    <NavPage title={props.title} headerVisible={false} statusBar={{ hidden: true, foreground: "light" }} class="bg-video">
      <View class="flex-1 bg-video">
        <If when={current() && !failure()}>
          <VideoSurface
            src={current() ? core.streamUrl(current()!.path, current()!.size) : ""}
            paused={paused()}
            startAt={current() ? resumeAt(current()!) : 0}
            sub={sub()}
            audio={audio()}
            class="absolute inset-0"
            handle={(handle) => (video = handle)}
            onLoad={onLoad}
            onProgress={onProgress}
            onBuffer={(e) => buffering.set(e.buffering)}
            onEnd={onEnd}
            onError={(e) => failure.set(e.message)}
          />
        </If>

        {/* Toque: um mostra os controles, dois do mesmo lado pulam. */}
        <View class="absolute inset-0 flex-row">
          <View class="flex-1" accessibilityLabel="Voltar 10 segundos" onPress={() => tap("left")} />
          <View class="flex-1" accessibilityLabel="Avançar 10 segundos" onPress={() => tap("right")} />
        </View>

        <If when={episodes.state() === "pending" || (buffering() && !failure() && episodes.state() !== "errored")}>
          <View class="absolute inset-0 items-center justify-center" style={{ pointerEvents: "none" }}>
            <Progress
              indeterminate
              accessibilityLabel="Carregando o vídeo"
              class="w-24"
              trackClass="h-1 overflow-hidden rounded-full bg-on-video/20"
              indicatorClass="h-1 w-1/3 rounded-full bg-on-video"
            />
          </View>
        </If>

        <If when={failure() ?? (episodes.error() ? errorMessage(episodes.error()) : null)}>
          {(message) => (
            <View class="absolute inset-0 items-center justify-center gap-3 bg-video p-8">
              <Text class="text-center text-base font-medium text-on-video">Não foi possível tocar o vídeo</Text>
              <Text class="text-center text-sm text-on-video-muted">{message()}</Text>
              <View class="mt-2 flex-row gap-2">
                <OutlineButton label="Voltar" onPress={close} />
                <OutlineButton
                  label="Tentar de novo"
                  onPress={() => {
                    failure.set(null);
                    buffering.set(true);
                    if (episodes.error()) episodes.refetch();
                  }}
                />
              </View>
            </View>
          )}
        </If>

        <If when={controls() && !failure()}>
          <View
            class="absolute inset-0 justify-between bg-scrim/50 pt-[env(safe-area-inset-top)] pr-[env(safe-area-inset-right)] pb-[env(safe-area-inset-bottom)] pl-[env(safe-area-inset-left)]"
            onPress={toggleControls}
          >
            <View class="flex-row items-center gap-1 px-2 pt-2">
              <IconButton icon="back" label="Fechar o player" onPress={close} class="active:bg-on-video/20" iconClass="tint-on-video" />
              <View class="flex-1">
                <Text class="text-base font-semibold text-on-video" style={{ numberOfLines: 1 }}>
                  {props.title}
                </Text>
                <If when={current()}>
                  {(episode) => (
                    <Text class="text-sm text-on-video-muted" style={{ numberOfLines: 1 }}>
                      {episodeLabel(episode())}
                    </Text>
                  )}
                </If>
              </View>
              <If when={hasMpv && tracks().length > 0}>
                <IconButton
                  icon="subtitles"
                  label="Legendas e áudio…"
                  onPress={() => {
                    tracksOpen.set(true);
                    keepControls();
                  }}
                  class="active:bg-on-video/20"
                  iconClass="tint-on-video"
                />
              </If>
            </View>

            <View class="flex-row items-center justify-center gap-10">
              <VideoButton icon="rewind" label="Voltar 10 segundos" onPress={() => seekBy(-SEEK_STEP)} />
              <VideoButton icon={paused() ? "play" : "pause"} label={paused() ? "Tocar" : "Pausar"} onPress={togglePaused} large />
              <VideoButton icon="forward" label="Avançar 10 segundos" onPress={() => seekBy(SEEK_STEP)} />
            </View>

            <View class="gap-1 px-4 pb-3">
              <View class="flex-row items-center gap-3">
                <Text class="min-w-12 text-sm tabular-nums text-on-video">{formatTime(shown())}</Text>
                <Slider
                  value={shown()}
                  min={0}
                  max={Math.max(duration(), 1)}
                  step={1}
                  disabled={duration() <= 0}
                  accessibilityLabel="Posição do vídeo"
                  class="flex-1"
                  trackClass="h-11 justify-center"
                  indicatorClass="h-1 rounded-full bg-on-video/30"
                  rangeClass="h-1 rounded-full bg-primary"
                  thumbClass="size-4 rounded-full bg-on-video"
                  onChange={({ value }) => {
                    scrub.set(value as number);
                    keepControls();
                  }}
                  onChangeEnd={({ value }) => {
                    seekTo(value as number);
                    scrub.set(null);
                    keepControls();
                  }}
                />
                <Text class="min-w-12 text-right text-sm tabular-nums text-on-video-muted">{formatTime(duration())}</Text>
                <If when={next()}>
                  {(following) => (
                    <IconButton
                      icon="next"
                      label="Próximo episódio"
                      onPress={() => play(following())}
                      class="active:bg-on-video/20"
                      iconClass="tint-on-video"
                    />
                  )}
                </If>
              </View>
            </View>
          </View>
        </If>

        <If when={inOpening() && !failure()}>
          <View class={CORNER} style={{ pointerEvents: "box-none" }}>
            <Button
              onPress={() => seekTo(skip().op?.end ?? position())}
              class="min-h-11 items-center justify-center rounded-(--radius) border border-on-video/40 bg-scrim/70 px-4 py-2 active:bg-on-video/20"
            >
              <Text class="text-base font-medium text-on-video">Pular abertura</Text>
            </Button>
          </View>
        </If>

        <If when={nextOffered() && !inOpening() && !failure()}>
          <View class={CORNER} style={{ pointerEvents: "box-none" }}>
            <View class="w-72 gap-3 rounded-(--radius) border border-border bg-popover p-3">
            <View class="gap-0.5">
              <Text class="text-sm text-muted-foreground">{settings.autoNext ? "A seguir" : "Próximo episódio"}</Text>
              <If when={next()}>
                {(following) => (
                  <Text class="text-base font-medium text-popover-foreground" style={{ numberOfLines: 1 }}>
                    {episodeLabel(following())}
                  </Text>
                )}
              </If>
            </View>
            <View class="flex-row gap-2">
              <Button
                onPress={() => nextDismissed.set(true)}
                class="min-h-11 flex-1 items-center justify-center rounded-(--radius) border border-input px-3 active:bg-accent"
              >
                <Text class="text-sm font-medium text-popover-foreground">Cancelar</Text>
              </Button>
              <Button
                onPress={() => {
                  const following = next();
                  if (following) play(following);
                }}
                class="min-h-11 flex-1 items-center justify-center rounded-(--radius) bg-primary px-3 active:bg-primary/85"
              >
                <Text class="text-sm font-semibold text-primary-foreground">Assistir agora</Text>
              </Button>
              </View>
            </View>
          </View>
        </If>
      </View>

      <Sheet
        open={tracksOpen()}
        onOpenChange={(e) => tracksOpen.set(e.open)}
        side="right"
        popupClass="h-full w-80 border-l border-border bg-popover pt-[env(safe-area-inset-top)] pr-[env(safe-area-inset-right)] pb-[env(safe-area-inset-bottom)]"
        backdropClass="bg-scrim/50"
      >
        <Scroll class="flex-1" contentClass="gap-4 p-3">
          <View class="gap-1">
            <Text role="header" class="px-3 py-1 text-sm font-semibold text-muted-foreground">
              Legendas
            </Text>
            <TrackRow label="Desligadas" selected={activeSub() === "off"} onPress={() => choose("off", activeAudio())} />
            <Each of={subs()} key={(t) => t.id}>
              {(t) => <TrackRow label={trackName(t())} selected={activeSub() === t().id} onPress={() => choose(t().id, activeAudio())} />}
            </Each>
          </View>
          <If when={audios().length > 1}>
            <View class="gap-1">
              <Text role="header" class="px-3 py-1 text-sm font-semibold text-muted-foreground">
                Áudio
              </Text>
              <Each of={audios()} key={(t) => t.id}>
                {(t) => <TrackRow label={trackName(t())} selected={activeAudio() === t().id} onPress={() => choose(activeSub(), t().id)} />}
              </Each>
            </View>
          </If>
        </Scroll>
      </Sheet>
    </NavPage>
  );
}

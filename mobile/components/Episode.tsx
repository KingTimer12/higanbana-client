// O episódio no telefone: uma linha, como nas listas de episódios dos apps de
// vídeo, com a miniatura de cantos de pétala (a do desktop) à esquerda e o número,
// o título e o estado à direita, numa coluna só, para o título caber inteiro.
// O toque troca os cantos e cobre a imagem com o véu vermelho, como o hover no desktop.
// A miniatura e o título vêm do TMDB; sem episódio casado, quadro neutro com o
// ▶ ao centro e "Episódio N", como no desktop.
import { Image, Text, View } from "nsh";
import { Button } from "nsh/base";
import type { Episode, EpisodeInfo } from "../lib/backend";
import { episodeNumber, formatTime } from "../lib/format";
import { Icon, Skeleton } from "./ui";

/** Altura estimada de uma linha de episódio (a lista mede a real). */
export const EPISODE_HEIGHT = 96;

/** Cantos da pétala em repouso (4 e 22 pt no desktop) e trocados no toque. */
const PETAL = "rounded-tl-sm rounded-tr-3xl rounded-br-sm rounded-bl-3xl";
const PETAL_PRESSED = "group-active:rounded-tl-3xl group-active:rounded-tr-sm group-active:rounded-br-3xl group-active:rounded-bl-sm";

/** A miniatura 16:9 de um episódio, com o quanto já foi visto embaixo (0 a 1).
 * `still` é a imagem do TMDB; `loading`, esqueleto enquanto ele responde. */
export function EpisodeFrame(props: { seen?: number; still?: string | null; loading?: boolean; class?: string }) {
  return (
    <View class={`aspect-video w-full overflow-hidden border border-border bg-muted ${PETAL} ${PETAL_PRESSED} ${props.class ?? ""}`}>
      <If
        when={props.still}
        else={
          <If when={props.loading} else={
            <View class="flex-1 items-center justify-center">
              <Icon name="play" class="size-7 tint-faint group-active:tint-on-video" />
            </View>
          }>
            <Skeleton class="flex-1" />
          </If>
        }
      >
        {(src) => <Image src={src()} fadeDuration={200} class="size-full object-cover transition-transform duration-300 group-active:scale-[1.06] motion-reduce:group-active:scale-100" />}
      </If>
      {/* Véu vermelho do toque, como o do hover no desktop. */}
      <View class="absolute inset-0 bg-primary/0 group-active:bg-primary/30" />
      <If when={(props.seen ?? 0) > 0}>
        <View class="absolute right-0 bottom-0 left-0 h-[3px] bg-on-video/20">
          <View class="h-full bg-primary" style={{ width: `${Math.min(1, props.seen ?? 0) * 100}%` }} />
        </View>
      </If>
    </View>
  );
}

/** Nota do TMDB ao lado do resto da linha de baixo. */
function Vote(props: { vote: number }) {
  return (
    <View class="flex-row items-center gap-1">
      <Icon name="star" class="size-3 tint-score" />
      <Text class="text-xs font-semibold tabular-nums text-foreground">{props.vote.toFixed(1)}</Text>
    </View>
  );
}

/** Um episódio na lista da obra. `info`: o do TMDB; `undefined` enquanto ele responde. */
export function EpisodeCard(props: { episode: Episode; info: EpisodeInfo | null | undefined; onPress: () => void }) {
  const inProgress = () => !props.episode.watched && props.episode.position > 0 && props.episode.duration > 0;
  // Como o `card_text` do desktop: o título do TMDB; sem ele, "Episódio N" ou o nome do arquivo.
  const title = () =>
    props.info?.title || (props.episode.number !== null ? `Episódio ${props.episode.number}` : props.episode.label);
  const seen = () => (props.episode.watched ? 1 : inProgress() ? props.episode.position / props.episode.duration : 0);
  return (
    <Button
      onPress={props.onPress}
      testID={`episode-${props.episode.name}`}
      accessibilityLabel={props.episode.number !== null ? `Episódio ${props.episode.number}, ${title()}` : title()}
      class="group flex-row items-center gap-3 py-1"
    >
      <View class="w-36">
        <EpisodeFrame seen={seen()} still={props.info?.still} loading={props.info === undefined} />
      </View>
      <View class="flex-1 gap-1">
        <If when={props.episode.number}>
          {(n) => <Text class="text-xs font-bold tabular-nums text-primary">EP {episodeNumber(n())}</Text>}
        </If>
        <Text
          class="text-sm font-semibold text-foreground"
          classList={{ "text-muted-foreground": props.episode.watched }}
          style={{ numberOfLines: 2 }}
        >
          {title()}
        </Text>
        <View class="flex-row flex-wrap items-center gap-x-2.5 gap-y-1">
          <If when={props.info?.vote}>{(vote) => <Vote vote={vote()} />}</If>
          <If when={props.info?.runtime}>
            {(min) => <Text class="text-xs tabular-nums text-muted-foreground">{min()} min</Text>}
          </If>
          <If when={props.episode.watched}>
            <View class="flex-row items-center gap-1">
              <Icon name="check" class="size-3.5 tint-success" />
              <Text class="text-xs text-success">Visto</Text>
            </View>
          </If>
          <If when={inProgress()}>
            <Text class="text-xs tabular-nums text-muted-foreground">Faltam {formatTime(props.episode.duration - props.episode.position)}</Text>
          </If>
          <If when={!props.episode.watched && !inProgress() && props.episode.quality}>
            <Text class="text-xs text-muted-foreground">{props.episode.quality}</Text>
          </If>
        </View>
      </View>
    </Button>
  );
}

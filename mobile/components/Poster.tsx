// O pôster do acervo, o mesmo card do desktop (`components::poster`): capa 2:3
// com a nota do AniList e a marca de "está no acervo", e embaixo o título e
// `formato · ano`. Quem usa decide o toque e de onde vêm os metadados.
import { effect, Text, View } from "nsh";
import { Button } from "nsh/base";
import type { Media, WorkEntry } from "../lib/backend";
import { mediaTitle, posterMeta } from "../lib/format";
import { cloudPathOf, coverOf, lookupOf, nav, openingMedia, openMedia, wantCloudPath, wantCover, wantLookup } from "../store";
import { Cover, Icon, Skeleton } from "./ui";

/** Colunas das grades de pôsteres no telefone. */
export const POSTER_COLUMNS = 3;
/** Altura estimada de um pôster com título (a lista mede o real). */
export const POSTER_HEIGHT = 230;
/** Espaço entre pôsteres, como no desktop: menos entre colunas, mais entre linhas. */
export const POSTER_GRID = "gap-x-3 gap-y-6";

export interface PosterProps {
  title: string;
  /** `undefined` enquanto os metadados chegam (capa em esqueleto). */
  media: Media | null | undefined;
  /** Linha de baixo; sem ela, formato · ano. */
  meta?: string;
  /** Ano da pasta, quando o AniList não tem. */
  year?: number | null;
  /** Está no acervo (marca no canto). */
  inLibrary?: boolean;
  /** Fora do acervo: o card fica apagado. */
  dimmed?: boolean;
  /** Abrindo: espera a nuvem responder. */
  busy?: boolean;
  onPress: () => void;
  testID?: string;
}

/** Nota do AniList sobre a capa. */
function ScoreMark(props: { score: number }) {
  return (
    <View class="absolute top-2 right-2 flex-row items-center gap-1 rounded-(--radius) bg-scrim/70 px-1.5 py-0.5">
      <Icon name="star" class="size-3 tint-score" />
      <Text class="text-xs font-semibold tabular-nums text-on-video">{props.score}</Text>
    </View>
  );
}

/** Marca de "está no acervo", sobre a capa. */
function LibraryMark() {
  return (
    <View accessibilityLabel="No acervo" class="absolute top-2 left-2 size-6 items-center justify-center rounded-full bg-primary">
      <Icon name="folder-check" class="size-3.5 tint-primary-foreground" />
    </View>
  );
}

export function Poster(props: PosterProps) {
  // A capa nítida do fanart chega depois; até lá fica a do AniList.
  effect(() => {
    if (props.media) wantCover(props.media);
  });
  const meta = () => props.meta ?? posterMeta(props.media, props.year ?? null);
  return (
    <Button
      onPress={props.onPress}
      testID={props.testID}
      accessibilityLabel={props.title}
      class="flex-1 gap-2 transition-transform duration-150 active:scale-[0.97] motion-reduce:active:scale-100"
      classList={{ "opacity-55": !!props.dimmed, "opacity-50": !!props.busy }}
    >
      <If when={props.media !== undefined} else={<Skeleton class="aspect-2/3 w-full rounded-(--radius-lg)" />}>
        <Cover src={coverOf(props.media)} title={props.title} color={props.media?.coverImage?.color} class="w-full">
          <If when={props.inLibrary}>
            <LibraryMark />
          </If>
          <If when={props.media?.averageScore}>{(score) => <ScoreMark score={score()} />}</If>
        </Cover>
      </If>
      <View class="gap-0.5">
        <Text class="text-sm font-semibold text-foreground" style={{ numberOfLines: 2 }}>
          {props.title}
        </Text>
        <If when={meta()}>
          <Text class="text-xs text-muted-foreground" style={{ numberOfLines: 1 }}>
            {meta()}
          </Text>
        </If>
      </View>
    </Button>
  );
}

/** Pôster de uma pasta da nuvem (catálogo, busca): pede os metadados quando aparece e abre a obra. */
export function WorkPoster(props: { work: WorkEntry }) {
  // A lista só monta os cards perto da tela, e recicla: o pedido segue o item atual.
  effect(() => wantLookup(props.work.name));
  const media = () => lookupOf(props.work.name);
  return (
    <Poster
      title={props.work.title}
      media={media()}
      year={props.work.year}
      onPress={() => nav.push({ name: "title", path: props.work.path, media: media() ?? null })}
    />
  );
}

/** Pôster de um anime da temporada: descobre se está no acervo quando aparece. */
export function SeasonPoster(props: { media: Media }) {
  effect(() => wantCloudPath(props.media));
  const known = () => cloudPathOf(props.media.id);
  return (
    <Poster
      title={mediaTitle(props.media)}
      media={props.media}
      inLibrary={!!known()}
      dimmed={known() === null}
      busy={openingMedia() === props.media.id}
      onPress={() => openMedia(props.media, mediaTitle(props.media))}
    />
  );
}

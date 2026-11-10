// Os controles do app: `nsh/base` dá o comportamento, aqui fica o visual.
// As telas compõem estes componentes; nenhuma estiliza um controle base por conta própria.
import { Image, type ImageAsset, type JSX, Text, View } from "nsh";
import { Button, Progress } from "nsh/base";
import back from "../assets/icons/back.svg";
import check from "../assets/icons/check.svg";
import chevron from "../assets/icons/chevron.svg";
import clapperboard from "../assets/icons/clapperboard.svg";
import close from "../assets/icons/close.svg";
import folderCheck from "../assets/icons/folder-check.svg";
import forward from "../assets/icons/forward.svg";
import grid from "../assets/icons/grid.svg";
import next from "../assets/icons/next.svg";
import pause from "../assets/icons/pause.svg";
import play from "../assets/icons/play.svg";
import rewind from "../assets/icons/rewind.svg";
import search from "../assets/icons/search.svg";
import star from "../assets/icons/star.svg";
import subtitles from "../assets/icons/subtitles.svg";
import tv from "../assets/icons/tv.svg";

const ICONS = {
  back,
  check,
  chevron,
  clapperboard,
  close,
  "folder-check": folderCheck,
  forward,
  grid,
  next,
  pause,
  play,
  rewind,
  search,
  star,
  subtitles,
  tv,
} satisfies Record<string, ImageAsset>;

export type IconName = keyof typeof ICONS;

/** Um ícone da família do app, na cor de `class` (`tint-*`) e no tamanho de `class` (`size-*`). */
export function Icon(props: { name: IconName; class?: string }) {
  return <Image src={ICONS[props.name]} class={`size-6 tint-foreground ${props.class ?? ""}`} />;
}

/** O compromisso principal de uma área de decisão. */
export function PrimaryButton(props: { label: string; icon?: IconName; disabled?: boolean; onPress: () => void; class?: string }) {
  return (
    <Button
      onPress={props.onPress}
      disabled={props.disabled}
      class={`min-h-12 flex-row items-center justify-center gap-2 rounded-(--radius) bg-primary px-5 py-3 active:bg-primary/85 disabled:opacity-50 ${props.class ?? ""}`}
    >
      <If when={props.icon}>{(icon) => <Icon name={icon()} class="size-5 tint-primary-foreground" />}</If>
      <Text class="text-base font-semibold text-primary-foreground">{props.label}</Text>
    </Button>
  );
}

/** Uma ação comum, com contorno. */
export function OutlineButton(props: { label: string; icon?: IconName; disabled?: boolean; onPress: () => void; class?: string }) {
  return (
    <Button
      onPress={props.onPress}
      disabled={props.disabled}
      class={`min-h-11 flex-row items-center justify-center gap-2 rounded-(--radius) border border-input px-4 py-2 active:bg-accent disabled:opacity-50 ${props.class ?? ""}`}
    >
      <If when={props.icon}>{(icon) => <Icon name={icon()} class="size-5" />}</If>
      <Text class="text-base font-medium text-foreground">{props.label}</Text>
    </Button>
  );
}

/** Uma ação discreta, só texto. */
export function GhostButton(props: { label: string; onPress: () => void; class?: string }) {
  return (
    <Button
      onPress={props.onPress}
      class={`min-h-11 items-center justify-center rounded-(--radius) px-3 py-2 active:bg-accent ${props.class ?? ""}`}
    >
      <Text class="text-base font-medium text-muted-foreground">{props.label}</Text>
    </Button>
  );
}

/** Um botão só de ícone, com nome para leitores de tela e alvo de toque de 44 pt. */
export function IconButton(props: { icon: IconName; label: string; onPress: () => void; class?: string; iconClass?: string }) {
  return (
    <Button
      onPress={props.onPress}
      accessibilityLabel={props.label}
      class={`size-11 items-center justify-center rounded-full active:bg-accent ${props.class ?? ""}`}
    >
      <Icon name={props.icon} class={props.iconClass} />
    </Button>
  );
}

/** Uma escolha entre irmãs (temporada, versão, letra): a selecionada fica preenchida. */
export function Chip(props: { label: string; selected: boolean; onPress: () => void; testID?: string }) {
  return (
    <Button
      onPress={props.onPress}
      selected={props.selected}
      role="tab"
      testID={props.testID}
      class="group min-h-11 items-center justify-center rounded-full border border-border px-4 py-2 active:bg-accent selected:border-primary selected:bg-primary"
    >
      <Text class="text-sm font-medium text-muted-foreground group-selected:text-primary-foreground">{props.label}</Text>
    </Button>
  );
}

/** O título de uma seção da tela, com algo à direita (uma contagem, um estado). */
export function SectionTitle(props: { children: string; aside?: JSX.Element; class?: string }) {
  return (
    // O que vai à direita desce para a linha de baixo quando o título não deixa espaço.
    <View class={`flex-row flex-wrap items-end justify-between gap-x-4 gap-y-1 ${props.class ?? ""}`}>
      <Text role="header" class="text-xl font-semibold text-foreground">
        {props.children}
      </Text>
      {props.aside}
    </View>
  );
}

/** Uma entrada do acervo (como as seções do início no desktop): ícone, nome, o que tem dentro e a seta. */
export function SectionCard(props: { icon: IconName; label: string; blurb: string; onPress: () => void; testID?: string }) {
  return (
    <Button
      onPress={props.onPress}
      testID={props.testID}
      class="min-h-20 flex-row items-center gap-4 rounded-(--radius-lg) border border-border bg-card px-4 transition-transform duration-150 active:scale-[0.98] active:bg-accent motion-reduce:active:scale-100"
    >
      <View class="size-10 items-center justify-center rounded-(--radius) bg-muted">
        <Icon name={props.icon} class="size-5" />
      </View>
      <View class="flex-1 gap-0.5">
        <Text class="text-base font-semibold text-card-foreground">{props.label}</Text>
        <Text class="text-sm text-muted-foreground" style={{ numberOfLines: 1 }}>
          {props.blurb}
        </Text>
      </View>
      <Icon name="chevron" class="size-4 tint-muted-foreground" />
    </Button>
  );
}

/** Lugar de algo que ainda chega da rede, no tamanho e na forma dele. */
export function Skeleton(props: { class?: string }) {
  return <View accessibilityLabel="Carregando" class={`bg-muted ${props.class ?? ""}`} />;
}

/** Quanto de um episódio já foi visto. */
export function WatchBar(props: { value: number; class?: string }) {
  return (
    <Progress
      value={props.value}
      accessibilityLabel="Progresso"
      class={props.class}
      trackClass="h-1 overflow-hidden rounded-full bg-on-video/20"
      indicatorClass="h-1 rounded-full bg-primary"
    />
  );
}

/** Espera de verdade (rede): uma barra sem valor e o que está sendo carregado. */
export function Loading(props: { label: string }) {
  return (
    <View class="flex-1 items-center justify-center gap-3 p-8">
      <Progress
        indeterminate
        accessibilityLabel={props.label}
        class="w-32"
        trackClass="h-1 overflow-hidden rounded-full bg-muted"
        indicatorClass="h-1 w-1/3 rounded-full bg-primary"
      />
      <Text class="text-sm text-muted-foreground">{props.label}</Text>
    </View>
  );
}

/** Nada para mostrar, ou algo falhou: o que houve e como seguir. */
export function EmptyState(props: { title: string; detail?: string; action?: string; onAction?: () => void }) {
  return (
    <View class="flex-1 items-center justify-center gap-2 p-8">
      <Text class="text-center text-base font-medium text-foreground">{props.title}</Text>
      <If when={props.detail}>{(detail) => <Text class="text-center text-sm text-muted-foreground">{detail()}</Text>}</If>
      <If when={props.action}>
        {(action) => <OutlineButton label={action()} onPress={() => props.onAction?.()} class="mt-4" />}
      </If>
    </View>
  );
}

/** A capa de uma obra, 2:3, como no desktop: a imagem do AniList sobre a cor dela;
 * sem imagem, uma capa tipográfica com o nome inteiro. `color` é dado (a cor da capa). */
export function Cover(props: { src: string | null; title: string; color?: string | null; class?: string; children?: JSX.Element }) {
  return (
    <View
      class={`aspect-2/3 overflow-hidden rounded-(--radius-lg) bg-muted ${props.class ?? ""}`}
      style={props.color ? { backgroundColor: props.color } : undefined}
    >
      <If
        when={props.src}
        else={
          <View class="flex-1 justify-between bg-muted p-3">
            <Icon name="clapperboard" class="size-4 tint-muted-foreground" />
            <Text class="text-base font-semibold leading-tight text-foreground opacity-80" style={{ numberOfLines: 5 }}>
              {props.title}
            </Text>
          </View>
        }
      >
        {(src) => <Image src={src()} fadeDuration={200} class="size-full object-cover" accessibilityLabel={props.title} />}
      </If>
      {props.children}
    </View>
  );
}

/** As cores do cabeçalho do sistema (iOS e Android), as mesmas do `PageHeader`. */
export const SYSTEM_HEADER = "bg-background text-foreground tint-foreground";

/** O cabeçalho que o app desenha onde não há o do sistema (desktops e web). */
export function PageHeader(props: { title: string; onBack?: () => void; trailing?: JSX.Element }) {
  return (
    <View class="h-14 flex-row items-center gap-1 border-b border-border bg-background px-2">
      <If when={props.onBack} else={<View class="w-2" />}>
        {(onBack) => <IconButton icon="back" label="Voltar" onPress={() => onBack()()} />}
      </If>
      <Text class="flex-1 text-lg font-semibold text-foreground" style={{ numberOfLines: 1 }}>
        {props.title}
      </Text>
      {props.trailing}
    </View>
  );
}

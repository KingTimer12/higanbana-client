// As vitrines do início, as mesmas do desktop (destaques do ano e descobrir),
// feitas para o polegar: o destaque é um carrossel que passa de página em página
// com o dedo, sem trocar sozinho; o descobrir é uma fileira de pôsteres que rola
// de lado, com a lista escolhida acima. Só entra o que está no acervo.
import { effect, Image, type Resource, resource, Scroll, signal, Text, token, View, windowSize } from "nsh";
import { Button } from "nsh/base";
import { backend, type Featured, type Ranking } from "../lib/backend";
import { errorMessage, highlightsHeading, mediaFacts, mediaTitle } from "../lib/format";
import { coverOf, nav, wantCover } from "../store";
import { Poster } from "./Poster";
import { Chip, Cover, EmptyState, SectionTitle, Skeleton } from "./ui";

/** Obras no carrossel de destaques (como no desktop). */
const HIGHLIGHTS = 6;
/** Pôsteres por lista do descobrir. */
const DISCOVER = 12;

const open = (item: Featured) => nav.push({ name: "title", path: item.path, media: item.media, kind: "anime" });

/** Um destaque: o banner da obra (ou a capa) tingido pela cor dela, a capa e o essencial. */
function HighlightCard(props: { item: Featured; rank: number }) {
  const media = () => props.item.media;
  effect(() => wantCover(media()));
  const backdrop = () => media().bannerImage ?? coverOf(media());
  const detail = () => [media().studio, ...(media().tags ?? [])].filter(Boolean).join(" · ");
  return (
    <Button
      onPress={() => open(props.item)}
      accessibilityLabel={`${mediaTitle(media())}, destaque ${props.rank}`}
      testID={`highlight-${media().id}`}
      class="h-64 overflow-hidden rounded-(--radius-lg) bg-card transition-transform duration-150 active:scale-[0.98] motion-reduce:active:scale-100"
      // A cor da capa é dado do AniList: o fundo do cartão enquanto o banner chega.
      style={media().coverImage?.color ? { backgroundColor: media().coverImage!.color! } : undefined}
    >
      <If when={backdrop()}>
        {(src) => <Image src={src()} fadeDuration={250} class="absolute inset-0 size-full object-cover" style={{ opacity: 0.55 }} />}
      </If>
      {/* O véu até o fundo da página: o texto fica legível em qualquer banner. */}
      <View
        class="absolute inset-0"
        style={{ backgroundImage: `linear-gradient(180deg, transparent 10%, ${token("background")} 96%)` }}
      />
      <View class="flex-1 flex-row items-end gap-4 p-4">
        <Cover src={coverOf(media())} title={mediaTitle(media())} color={media().coverImage?.color} class="w-24 shadow-lg" />
        <View class="flex-1 gap-1 pb-1">
          <Text class="text-xs font-bold tabular-nums text-brand-ink">Nº {props.rank}</Text>
          <Text class="text-xl font-bold leading-tight text-foreground" style={{ numberOfLines: 2 }}>
            {mediaTitle(media())}
          </Text>
          <Text class="text-sm text-muted-foreground" style={{ numberOfLines: 1 }}>
            {mediaFacts(media())}
          </Text>
          <If when={detail()}>
            <Text class="text-xs text-faint" style={{ numberOfLines: 1 }}>
              {detail()}
            </Text>
          </If>
        </View>
      </View>
    </Button>
  );
}

/** Onde o carrossel está: o ponto da página atual se alonga. */
function PageDots(props: { count: number; current: number }) {
  return (
    <View class="flex-row items-center justify-center gap-1.5" accessibilityLabel={`Destaque ${props.current + 1} de ${props.count}`}>
      <Each of={Array.from({ length: props.count }, (_, i) => i)}>
        {(i) => (
          <View
            class="h-1.5 rounded-full transition-all duration-200 motion-reduce:transition-none"
            classList={{ "w-5 bg-primary": props.current === i(), "w-1.5 bg-muted-foreground/40": props.current !== i() }}
          />
        )}
      </Each>
    </View>
  );
}

/** Destaques do ano: os mais populares do ano que estão no acervo, uma página por obra. */
export function Highlights(props: { list: Featured[] | undefined; state: string; error: unknown; onRetry: () => void }) {
  const page = signal(0);
  // Cada página ocupa a largura da janela; o cartão leva a margem da tela dentro dela.
  const width = () => windowSize().width;
  return (
    <View class="gap-3">
      <View class="px-4">
        <SectionTitle>{highlightsHeading()}</SectionTitle>
      </View>
      <If when={props.state === "pending"}>
        <View class="px-4">
          <Skeleton class="h-64 rounded-(--radius-lg)" />
        </View>
      </If>
      <If when={props.state === "errored"}>
        <View class="h-48">
          <EmptyState title="Não foi possível carregar os destaques" detail={errorMessage(props.error)} action="Tentar de novo" onAction={props.onRetry} />
        </View>
      </If>
      <If when={(props.state === "ready" || props.state === "refreshing") && (props.list ?? []).length === 0}>
        <View class="h-32">
          <EmptyState title="Nenhum destaque do ano no acervo ainda" detail="As obras chegam ao acervo na temporada seguinte." />
        </View>
      </If>
      <If when={(props.list ?? []).length > 0}>
        <Scroll
          horizontal
          pagingEnabled
          scrollbar={false}
          testID="highlights"
          onScrollEnd={({ offset }) => page.set(Math.round(offset.x / Math.max(width(), 1)))}
        >
          <View class="flex-row">
            <Each of={props.list ?? []} key={(item) => item.path}>
              {(item, index) => (
                <View class="px-4" style={{ width: width() }}>
                  <HighlightCard item={item()} rank={index() + 1} />
                </View>
              )}
            </Each>
          </View>
        </Scroll>
        <PageDots count={(props.list ?? []).length} current={page()} />
      </If>
    </View>
  );
}

const LISTS: readonly { ranking: Ranking; label: string }[] = [
  { ranking: "topRated", label: "Mais bem avaliados" },
  { ranking: "popular", label: "Populares" },
  { ranking: "trending", label: "Em alta" },
];

/** Uma fileira de pôsteres esperando a nuvem: o formato do que vem. */
function RowSkeleton() {
  return (
    <View class="flex-row gap-3 px-4">
      <Each of={[0, 1, 2]}>
        {() => (
          <View class="w-32 gap-2">
            <Skeleton class="aspect-2/3 w-full rounded-(--radius-lg)" />
            <Skeleton class="h-3 w-3/4 rounded-(--radius)" />
          </View>
        )}
      </Each>
    </View>
  );
}

/** Descobrir: três listas do AniList filtradas pelo acervo; cada uma confere na primeira vez que abre.
 * `ready` segura o começo até os destaques terminarem, que usam o mesmo saldo de listagens da nuvem. */
export function Discover(props: { ready: boolean }) {
  const current = signal<Ranking>("topRated");
  const opened = signal<ReadonlySet<Ranking>>(new Set());
  effect(() => {
    if (props.ready && !opened().has(current())) opened.set(new Set([...opened(), current()]));
  });
  const lists = Object.fromEntries(
    LISTS.map(({ ranking }) => [
      ranking,
      resource(
        () => (opened().has(ranking) ? ranking : undefined),
        (r) => backend().discover(r, DISCOVER),
      ),
    ]),
  ) as Record<Ranking, Resource<Featured[]>>;
  const list = () => lists[current()];
  const waiting = () => !props.ready || list().state() === "pending" || list().state() === "unresolved";

  return (
    <View class="gap-3">
      <View class="px-4">
        <SectionTitle>Descobrir</SectionTitle>
      </View>
      <Scroll horizontal scrollbar={false} contentClass="flex-row gap-2 px-4">
        <Each of={LISTS}>
          {(l) => (
            <Chip label={l().label} selected={current() === l().ranking} onPress={() => current.set(l().ranking)} testID={`discover-${l().ranking}`} />
          )}
        </Each>
      </Scroll>
      <If when={waiting()}>
        <RowSkeleton />
      </If>
      <If when={!waiting() && list().state() === "errored"}>
        <View class="h-40">
          <EmptyState title="Não foi possível carregar esta lista" detail={errorMessage(list().error())} action="Tentar de novo" onAction={list().refetch} />
        </View>
      </If>
      <If when={!waiting() && list().state() !== "errored" && (list()() ?? []).length === 0}>
        <View class="h-40">
          <EmptyState title="Nada desta lista no acervo" detail="Tente outra lista." />
        </View>
      </If>
      <If when={!waiting() && (list()() ?? []).length > 0}>
        <Scroll horizontal scrollbar={false} contentClass="flex-row gap-3 px-4">
          <Each of={list()() ?? []} key={(item) => item.path}>
            {(item) => (
              <View class="w-32">
                <Poster title={mediaTitle(item().media)} media={item().media} onPress={() => open(item())} />
              </View>
            )}
          </Each>
        </Scroll>
      </If>
    </View>
  );
}

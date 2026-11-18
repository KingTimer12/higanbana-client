// A franquia, como no desktop (`views/franchise.rs`): pasta cujas subpastas são
// obras datadas (`[2004] Futari wa Precure [TV]`, `[2005.1] Movie 1 [FILME]`).
// No topo, a capa e o nome da franquia com os anos; embaixo, as obras em
// pôsteres, separadas em Séries TV · Filmes · Especiais · Outros. O formato do
// AniList decide a aba, então as abas se acertam conforme os metadados chegam.
import { computed, effect, List, Scroll, signal, Text, View } from "nsh";
import { POSTER_COLUMNS, POSTER_GRID, POSTER_HEIGHT, WorkPoster } from "../components/Poster";
import { Chip, Cover } from "../components/ui";
import { backend, type MediaKind, type Work, type WorkEntry } from "../lib/backend";
import { baseName } from "../lib/format";
import { coverOf, lookupOf, wantCover, wantLookup } from "../store";

/** `2004 · 49 ep`: o ano da pasta (ou do AniList) e os episódios, quando é série. */
function workMeta(work: WorkEntry, kind: MediaKind): string {
  const media = lookupOf(work.name, kind);
  const year = work.year ?? media?.seasonYear ?? null;
  const episodes = media?.episodes && media.episodes > 1 ? `${media.episodes} ep` : null;
  return [year ? String(year) : null, episodes].filter(Boolean).join(" · ");
}

/** O corpo da página de uma franquia; quem a abre põe a moldura (`Title`). */
export function Franchise(props: { work: Work; kind: MediaKind }) {
  const works = () => props.work.seasons;
  const folder = () => baseName(props.work.path);

  // Todas as obras de uma vez (não só as visíveis), como no desktop: o formato
  // decide a aba, então as abas já nascem certas. Os pedidos saem em lote.
  effect(() => {
    wantLookup(folder(), props.kind);
    for (const w of works()) wantLookup(w.name, props.kind);
  });
  const hero = () => lookupOf(folder(), props.kind) ?? null;
  effect(() => {
    const m = hero();
    if (m) wantCover(m);
  });

  const tabs = computed(() =>
    backend().franchiseTabs(
      works().map((w) => w.name),
      works().map((w) => lookupOf(w.name, props.kind)?.format ?? null),
    ),
  );
  // A aba escolhida; sem escolha (ou se ela sumiu), a primeira que existir.
  const chosen = signal<string | null>(null);
  const active = () => tabs().find((t) => t.id === chosen()) ?? tabs()[0];
  const shown = () => (active()?.works ?? []).map((i) => works()[i]).filter((w): w is WorkEntry => !!w);

  const header = (
    <View class="gap-6 pb-2">
      <View class="flex-row items-end gap-4 pt-4">
        <Cover src={coverOf(hero())} title={props.work.title} color={hero()?.coverImage?.color} class="w-28" />
        <View class="flex-1 gap-1">
          <Text role="header" class="text-2xl font-bold text-foreground" style={{ numberOfLines: 3 }}>
            {props.work.title}
          </Text>
          <Text class="text-sm text-muted-foreground">{props.work.span ?? "Franquia"}</Text>
        </View>
      </View>
      {/* Uma aba só dispensa a barra. */}
      <If when={tabs().length > 1}>
        <Scroll horizontal scrollbar={false} class="-mx-4" contentClass="flex-row gap-2 px-4">
          <Each of={tabs()} key={(t) => t.id}>
            {(t) => (
              <Chip label={t().label} selected={active()?.id === t().id} onPress={() => chosen.set(t().id)} testID={`franchise-${t().id}`} />
            )}
          </Each>
        </Scroll>
      </If>
    </View>
  );

  return (
    <List<WorkEntry>
      each={shown()}
      keyExtractor={(w) => w.path}
      numColumns={POSTER_COLUMNS}
      estimatedItemSize={POSTER_HEIGHT}
      class="flex-1"
      contentClass={`${POSTER_GRID} px-4 pb-[calc(env(safe-area-inset-bottom)+24px)]`}
      header={header}
    >
      {(w) => <WorkPoster work={w()} kind={props.kind} meta={workMeta(w(), props.kind)} testID={`work-${w().name}`} />}
    </List>
  );
}

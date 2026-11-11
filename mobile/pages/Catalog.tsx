import { List, resource, Scroll, signal, Text, View } from "nsh";
import { Input, NavPage } from "nsh/base";
import { POSTER_COLUMNS, POSTER_GRID, POSTER_HEIGHT, WorkPoster } from "../components/Poster";
import { Chip, EmptyState, Icon, PageHeader, Skeleton, SYSTEM_HEADER } from "../components/ui";
import { backend, type WorkEntry } from "../lib/backend";
import { errorMessage, letterLabel } from "../lib/format";
import { nav } from "../store";

/** Esqueleto da grade enquanto a letra carrega: duas linhas de pôsteres. */
export function PosterGridSkeleton() {
  return (
    <View class="gap-6 px-4 pt-2">
      <Each of={[0, 1]}>
        {() => (
          <View class="flex-row gap-3">
            <Each of={Array.from({ length: POSTER_COLUMNS }, (_, i) => i)}>
              {() => (
                <View class="flex-1 gap-2">
                  <Skeleton class="aspect-2/3 w-full rounded-(--radius-lg)" />
                  <Skeleton class="h-3 w-3/4 rounded-(--radius)" />
                </View>
              )}
            </Each>
          </View>
        )}
      </Each>
    </View>
  );
}

/** O campo de filtrar ou buscar das grades, com a lupa. */
export function SearchField(props: { value: string; onChange: (value: string) => void; placeholder: string; label: string; autoFocus?: boolean }) {
  return (
    <View class="min-h-12 flex-row items-center gap-2 rounded-(--radius) border border-input bg-card px-3 focus-within:border-ring">
      <Icon name="search" class="size-4 tint-muted-foreground" />
      <Input
        value={props.value}
        onChange={({ value }) => props.onChange(value)}
        placeholder={props.placeholder}
        accessibilityLabel={props.label}
        autoFocus={props.autoFocus}
        autoCapitalize="none"
        autoCorrect={false}
        returnKeyType="search"
        class="flex-1 py-3 text-base text-foreground placeholder:text-faint"
      />
    </View>
  );
}

/** Catálogo de animes da nuvem: o índice A–Z e a grade de pôsteres da letra. */
export function Catalog() {
  const letters = backend().letters();
  const letter = signal(letters[1] ?? letters[0] ?? "");
  const works = resource(letter, (l) => backend().letter(l));
  const filter = signal("");
  const shown = () => {
    const q = filter().trim().toLowerCase();
    const all = works() ?? [];
    return q ? all.filter((w) => w.title.toLowerCase().includes(q)) : all;
  };
  const choose = (l: string) => {
    filter.set("");
    letter.set(l);
  };

  // A lista já tem a margem lateral; fora dela (carregando), quem usa põe a sua.
  const Header = () => (
    <View class="gap-4 pb-2">
      <View class="flex-row items-end justify-between gap-4 pt-4">
        <Text role="header" class="text-2xl font-semibold text-foreground">
          {letterLabel(letter())}
        </Text>
        <If when={works.state() === "ready" || works.state() === "refreshing"}>
          <Text class="text-sm tabular-nums text-muted-foreground">{(works() ?? []).length} títulos</Text>
        </If>
      </View>
      <SearchField value={filter()} onChange={(v) => filter.set(v)} placeholder={`Filtrar em ${letterLabel(letter())}`} label="Filtrar títulos" />
    </View>
  );

  return (
    <NavPage title="Animes" headerClass={SYSTEM_HEADER} header={<PageHeader title="Animes" onBack={() => nav.pop()} />}>
      <View class="flex-1 bg-background">
        <View class="border-b border-border">
          <Scroll horizontal scrollbar={false} contentClass="flex-row gap-2 px-4 py-3">
            <Each of={letters}>
              {(l) => (
                <Chip label={letterLabel(l())} selected={letter() === l()} onPress={() => choose(l())} testID={`letter-${letterLabel(l())}`} />
              )}
            </Each>
          </Scroll>
        </View>

        <If when={works.state() === "pending"}>
          <View class="px-4">
            <Header />
          </View>
          <PosterGridSkeleton />
        </If>
        <If when={works.state() === "errored"}>
          <EmptyState
            title="Não foi possível abrir esta letra"
            detail={errorMessage(works.error())}
            action="Tentar de novo"
            onAction={works.refetch}
          />
        </If>
        <If when={works.state() === "ready" || works.state() === "refreshing"}>
          <List<WorkEntry>
            each={shown()}
            keyExtractor={(work) => work.path}
            numColumns={POSTER_COLUMNS}
            estimatedItemSize={POSTER_HEIGHT}
            keyboardDismissMode="on-drag"
            class="flex-1"
            contentClass={`${POSTER_GRID} px-4 pb-[calc(env(safe-area-inset-bottom)+24px)]`}
            header={Header}
            empty={
              <EmptyState
                title={filter().trim() ? `Nenhum título com “${filter().trim()}” em ${letterLabel(letter())}` : "Nenhuma obra nesta letra"}
              />
            }
          >
            {(work) => <WorkPoster work={work()} />}
          </List>
        </If>
      </View>
    </NavPage>
  );
}

import { cleanup, effect, List, resource, signal, View } from "nsh";
import { NavPage } from "nsh/base";
import { POSTER_COLUMNS, POSTER_GRID, POSTER_HEIGHT, WorkPoster } from "../components/Poster";
import { EmptyState, PageHeader, SYSTEM_HEADER } from "../components/ui";
import { backend, type WorkEntry } from "../lib/backend";
import { errorMessage } from "../lib/format";
import { nav } from "../store";
import { PosterGridSkeleton, SearchField } from "./Catalog";

/** Menor texto que vale uma busca na nuvem. */
const MIN_QUERY = 2;

/** Busca de animes na nuvem pelo nome, em pôsteres como o catálogo. */
export function Search() {
  const text = signal("");
  // A nuvem limita buscas seguidas: só procura depois de uma pausa na digitação.
  const query = signal("");
  effect(() => {
    const next = text().trim();
    const timer = setTimeout(() => query.set(next), 400);
    cleanup(() => clearTimeout(timer));
  });
  const results = resource(
    () => (query().length >= MIN_QUERY ? query() : undefined),
    (q) => backend().search(q),
  );

  return (
    <NavPage title="Buscar" headerClass={SYSTEM_HEADER} header={<PageHeader title="Buscar" onBack={() => nav.pop()} />}>
      <View class="flex-1 bg-background">
        <View class="border-b border-border px-4 py-3">
          <SearchField value={text()} onChange={(v) => text.set(v)} placeholder="Nome do anime" label="Buscar anime" autoFocus />
        </View>

        <If when={results.state() === "unresolved"}>
          <EmptyState title="Busque um anime pelo nome" detail="A busca procura nas pastas da nuvem." />
        </If>
        <If when={results.state() === "pending"}>
          <View class="pt-4">
            <PosterGridSkeleton />
          </View>
        </If>
        <If when={results.state() === "errored"}>
          <EmptyState title="A busca falhou" detail={errorMessage(results.error())} action="Tentar de novo" onAction={results.refetch} />
        </If>
        <If when={results.state() === "ready" || results.state() === "refreshing"}>
          <List<WorkEntry>
            each={results() ?? []}
            keyExtractor={(work) => work.path}
            numColumns={POSTER_COLUMNS}
            estimatedItemSize={POSTER_HEIGHT}
            keyboardDismissMode="on-drag"
            class="flex-1"
            contentClass={`${POSTER_GRID} px-4 pt-4 pb-[calc(env(safe-area-inset-bottom)+24px)]`}
            empty={<EmptyState title="Nenhum anime encontrado" detail={`Nada na nuvem com “${query()}”.`} />}
          >
            {(work) => <WorkPoster work={work()} />}
          </List>
        </If>
      </View>
    </NavPage>
  );
}

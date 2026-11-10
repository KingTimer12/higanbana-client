import { cleanup, effect, List, resource, signal, View } from "nsh";
import { Input, NavPage } from "nsh/base";
import { EmptyState, Loading, PageHeader } from "../components/ui";
import { backend, type WorkEntry } from "../lib/backend";
import { errorMessage } from "../lib/format";
import { SYSTEM_HEADER } from "../components/ui";
import { nav } from "../store";
import { WorkRow } from "./Catalog";

/** Menor texto que vale uma busca na nuvem. */
const MIN_QUERY = 2;

/** Busca de animes na nuvem pelo nome. */
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
        <Input
          value={text()}
          onChange={({ value }) => text.set(value)}
          placeholder="Nome do anime"
          accessibilityLabel="Buscar anime"
          autoCapitalize="none"
          autoCorrect={false}
          returnKeyType="search"
          class="min-h-12 rounded-(--radius) border border-input bg-card px-4 py-3 text-base text-foreground placeholder:text-faint focus:border-ring"
        />
      </View>

      <If when={results.state() === "unresolved"}>
        <EmptyState title="Busque um anime pelo nome" detail="A busca procura nas pastas da nuvem." />
      </If>
      <If when={results.state() === "pending"}>
        <Loading label="Buscando" />
      </If>
      <If when={results.state() === "errored"}>
        <EmptyState title="A busca falhou" detail={errorMessage(results.error())} action="Tentar de novo" onAction={results.refetch} />
      </If>
      <If when={results.state() === "ready" || results.state() === "refreshing"}>
        <List<WorkEntry>
          each={results() ?? []}
          keyExtractor={(work) => work.path}
          estimatedItemSize={64}
          keyboardDismissMode="on-drag"
          class="flex-1"
          contentClass="pb-[env(safe-area-inset-bottom)]"
          empty={<EmptyState title="Nenhum anime encontrado" detail={`Nada na nuvem com “${query()}”.`} />}
        >
          {(work) => <WorkRow work={work()} />}
        </List>
      </If>
      </View>
    </NavPage>
  );
}

import { List, resource, Scroll, signal, Text, View } from "nsh";
import { Button, NavPage } from "nsh/base";
import { Chip, EmptyState, Icon, Loading, PageHeader } from "../components/ui";
import { backend, type WorkEntry } from "../lib/backend";
import { errorMessage, letterLabel } from "../lib/format";
import { SYSTEM_HEADER } from "../components/ui";
import { nav } from "../store";

/** Uma obra numa lista: título, ano e a seta de abrir. */
export function WorkRow(props: { work: WorkEntry }) {
  return (
    <Button
      onPress={() => nav.push({ name: "title", path: props.work.path, media: null })}
      class="min-h-14 flex-row items-center gap-3 border-b border-border px-4 py-3 active:bg-accent"
    >
      <View class="flex-1 gap-0.5">
        <Text class="text-base text-foreground" style={{ numberOfLines: 2 }}>
          {props.work.title}
        </Text>
        <If when={props.work.year}>{(year) => <Text class="text-sm text-muted-foreground">{year()}</Text>}</If>
      </View>
      <Icon name="chevron" class="size-5 tint-faint" />
    </Button>
  );
}

/** Catálogo de animes da nuvem, por letra. */
export function Catalog() {
  const letters = backend().letters();
  const letter = signal(letters[1] ?? letters[0] ?? "");
  const works = resource(letter, (l) => backend().letter(l));

  return (
    <NavPage title="Catálogo" headerClass={SYSTEM_HEADER} header={<PageHeader title="Catálogo" onBack={() => nav.pop()} />}>
      <View class="flex-1 bg-background">
      <View class="border-b border-border">
        <Scroll horizontal scrollbar={false} contentClass="flex-row gap-2 px-4 py-3">
          <Each of={letters}>
            {(l) => (
              <Chip label={letterLabel(l())} selected={letter() === l()} onPress={() => letter.set(l())} testID={`letter-${letterLabel(l())}`} />
            )}
          </Each>
        </Scroll>
      </View>

      <If when={works.state() === "pending"}>
        <Loading label={`Carregando ${letterLabel(letter())}`} />
      </If>
      <If when={works.state() === "errored"}>
        <EmptyState
          title="Não foi possível carregar o catálogo"
          detail={errorMessage(works.error())}
          action="Tentar de novo"
          onAction={works.refetch}
        />
      </If>
      <If when={works.state() === "ready" || works.state() === "refreshing"}>
        <List<WorkEntry>
          each={works() ?? []}
          keyExtractor={(work) => work.path}
          estimatedItemSize={64}
          class="flex-1"
          contentClass="pb-[env(safe-area-inset-bottom)]"
          empty={<EmptyState title="Nenhuma obra nesta letra" />}
        >
          {(work) => <WorkRow work={work()} />}
        </List>
      </If>
      </View>
    </NavPage>
  );
}

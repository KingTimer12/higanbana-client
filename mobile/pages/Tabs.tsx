// A base da pilha: o conteúdo da aba aberta e a barra de abas por cima.
// Obra, busca e player entram por cima desta página e cobrem a barra; o voltar
// do sistema tira essas páginas e devolve as abas como estavam.
import { effect, signal, untracked, View } from "nsh";
import { NavPage } from "nsh/base";
import { BackHandler } from "nsh/mobile";
import { TAB_BAR_CLEARANCE, TabBar } from "../components/TabBar";
import { EmptyState, type IconName, Icon, TabHeader } from "../components/ui";
import { currentTab, nav, selectTab, type Tab } from "../store";
import { Catalog } from "./Catalog";
import { Home } from "./Home";

/** Uma seção que o desktop já tem e o telefone ainda não. */
function NotYet(props: { title: string; icon: IconName; what: string }) {
  return (
    <View class="flex-1 bg-background">
      <TabHeader title={props.title} />
      <View class={`flex-1 items-center justify-center gap-4 ${TAB_BAR_CLEARANCE}`}>
        <View class="size-16 items-center justify-center rounded-full bg-muted">
          <Icon name={props.icon} class="size-8 tint-muted-foreground" />
        </View>
        <View class="h-32 w-full">
          <EmptyState title={`${props.title} ainda não chegaram ao celular`} detail={`Por enquanto, ${props.what} no Higanbana do computador.`} />
        </View>
      </View>
    </View>
  );
}

function content(tab: Tab) {
  switch (tab) {
    case "animes":
      return <Catalog />;
    case "mangas":
      return <NotYet title="Mangás" icon="book" what="leia mangás" />;
    case "doramas":
      return <NotYet title="Doramas" icon="drama" what="assista doramas" />;
    case "musicas":
      return <NotYet title="Músicas" icon="music" what="ouça as músicas" />;
    default:
      return <Home />;
  }
}

const ORDER: readonly Tab[] = ["home", "animes", "mangas", "doramas", "musicas"];

export function Tabs() {
  // Cada aba monta na primeira visita e fica montada: a letra, o filtro e a
  // rolagem continuam onde estavam ao voltar a ela.
  const visited = signal<ReadonlySet<Tab>>(new Set([currentTab()]));
  effect(() => {
    const tab = currentTab();
    if (!untracked(visited).has(tab)) visited.set(new Set([...untracked(visited), tab]));
  });

  // Voltar numa aba que não é o Início leva ao Início antes de sair do app,
  // como nas barras de navegação do Android.
  BackHandler.on("press", () => {
    if (nav.routes().length > 1 || currentTab() === "home") return false;
    selectTab("home");
    return true;
  });

  return (
    <NavPage title="Higanbana" headerVisible={false}>
      <View class="flex-1 bg-background">
        <Each of={ORDER}>
          {(tab) => (
            <If when={visited().has(tab())}>
              <View class={currentTab() === tab() ? "flex-1" : "hidden"}>{content(tab())}</View>
            </If>
          )}
        </Each>
        <TabBar />
      </View>
    </NavPage>
  );
}

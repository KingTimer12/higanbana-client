// A barra de abas do telefone: uma pílula de vidro que flutua sobre o conteúdo,
// presa ao fundo da área segura. Liquid Glass no iOS 26; no Android e no resto,
// um desfoque do que passa por trás com uma tinta translúcida para manter a leitura.
import { Text, View } from "nsh";
import { Button } from "nsh/base";
import { currentTab, selectTab, type Tab } from "../store";
import { Icon, type IconName } from "./ui";

interface TabItem {
  tab: Tab;
  label: string;
  icon: IconName;
}

export const TABS: readonly TabItem[] = [
  { tab: "home", label: "Início", icon: "house" },
  { tab: "animes", label: "Animes", icon: "tv" },
  { tab: "mangas", label: "Mangás", icon: "book" },
  { tab: "doramas", label: "Doramas", icon: "drama" },
  { tab: "musicas", label: "Músicas", icon: "music" },
];

/** Espaço que o conteúdo das abas deixa embaixo para a barra não cobrir o fim da lista. */
export const TAB_BAR_CLEARANCE = "pb-[calc(env(safe-area-inset-bottom)+104px)]";

function TabButton(props: { item: TabItem }) {
  const selected = () => currentTab() === props.item.tab;
  return (
    <Button
      onPress={() => selectTab(props.item.tab)}
      selected={selected()}
      role="tab"
      accessibilityLabel={props.item.label}
      testID={`tab-${props.item.tab}`}
      class="group min-h-14 flex-1 items-center justify-center gap-0.5 rounded-full px-1 py-1.5 transition-colors duration-150 active:bg-glass-selected selected:bg-glass-selected"
    >
      <Icon name={props.item.icon} class="size-6 tint-muted-foreground group-selected:tint-primary" />
      <Text
        class="text-xs font-medium text-muted-foreground group-selected:font-semibold group-selected:text-foreground"
        style={{ numberOfLines: 1 }}
        maxFontSizeMultiplier={1.3}
      >
        {props.item.label}
      </Text>
    </Button>
  );
}

/** A barra em si; quem a usa a põe por cima do conteúdo da aba. A faixa de fora só a
 * posiciona (área segura e margens) e deixa os toques passarem ao conteúdo atrás. */
export function TabBar() {
  return (
    <View class="absolute right-0 bottom-0 left-0 px-4 pb-[calc(env(safe-area-inset-bottom)+12px)]" style={{ pointerEvents: "box-none" }}>
      <View
        accessibilityLabel="Seções"
        class="w-full max-w-lg flex-row gap-1 self-center rounded-full border border-glass-border bg-glass p-1.5 shadow-lg ios:border-0 ios:bg-transparent"
        style={{ glassEffect: "regular", backdropFilter: "blur(24px)", borderCurve: "continuous" }}
      >
        <Each of={TABS}>{(item) => <TabButton item={item()} />}</Each>
      </View>
    </View>
  );
}

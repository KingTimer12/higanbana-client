import { Image, List, resource, Scroll, signal, Text, View } from "nsh";
import { Button, Menu, MenuItem, NavPage } from "nsh/base";
import { Cover, EmptyState, IconButton, Loading, OutlineButton, SectionTitle, WatchBar } from "../components/ui";
import { backend, type Media, type Resume, type User } from "../lib/backend";
import { errorMessage, mediaCover, mediaTitle } from "../lib/format";
import { nav, openingMedia, openMedia, sessionState, signOut } from "../store";

const resume = signal<Resume[]>([]);

/** Relê os episódios em andamento (ao abrir o início e ao sair do player). */
export function refreshResume() {
  try {
    resume.set(backend().continueWatching());
  } catch {
    resume.set([]);
  }
}

function AccountMenu(props: { user: User }) {
  return (
    <Menu
      offset={4}
      popupClass="min-w-48 rounded-(--radius) border border-border bg-popover p-1 shadow-lg"
      backdropClass="bg-scrim/60"
      trigger={
        <View accessibilityLabel="Conta" class="size-11 items-center justify-center rounded-full active:bg-accent">
          <If
            when={props.user.avatar}
            else={
              <View class="size-9 items-center justify-center rounded-full bg-secondary">
                <Text class="text-sm font-semibold text-secondary-foreground">{props.user.name.slice(0, 1).toUpperCase()}</Text>
              </View>
            }
          >
            {(avatar) => <Image src={avatar()} class="size-9 rounded-full" />}
          </If>
        </View>
      }
    >
      <MenuItem onSelect={signOut} class="min-h-11 justify-center rounded-(--radius) px-3 py-2 active:bg-accent">
        <Text class="text-base text-popover-foreground">Sair</Text>
      </MenuItem>
    </Menu>
  );
}

function ResumeCard(props: { item: Resume }) {
  const open = () =>
    nav.push({ name: "player", dir: props.item.dir, file: props.item.name, title: props.item.title, malId: null });
  return (
    <Button
      onPress={open}
      accessibilityLabel={`Continuar ${props.item.title}, ${props.item.label}`}
      class="w-56 gap-2 rounded-(--radius) bg-card p-3 active:bg-accent"
    >
      <Text class="text-base font-medium text-card-foreground" style={{ numberOfLines: 1 }}>
        {props.item.title}
      </Text>
      <Text class="text-sm text-muted-foreground" style={{ numberOfLines: 1 }}>
        {props.item.label}
      </Text>
      <WatchBar value={(props.item.position / Math.max(props.item.duration, 1)) * 100} />
    </Button>
  );
}

function HomeHeader(props: { user: User | null; seasonState: string; seasonError: unknown; onRetry: () => void }) {
  return (
    <View class="gap-6 pb-4">
      <View class="flex-row items-center gap-1">
        <View class="flex-1">
          <Text class="text-sm text-muted-foreground">Olá, {props.user?.name ?? ""}</Text>
          <Text role="header" class="text-2xl font-bold text-foreground">
            Higanbana
          </Text>
        </View>
        <IconButton icon="search" label="Buscar" onPress={() => nav.push({ name: "search" })} />
        <If when={props.user}>{(user) => <AccountMenu user={user()} />}</If>
      </View>

      <If when={resume().length > 0}>
        <View class="gap-3">
          <SectionTitle>Continuar assistindo</SectionTitle>
          <Scroll horizontal scrollbar={false} class="-mx-4" contentClass="flex-row gap-3 px-4">
            <Each of={resume()} key={(item) => item.path}>
              {(item) => <ResumeCard item={item()} />}
            </Each>
          </Scroll>
        </View>
      </If>

      <OutlineButton label="Catálogo A–Z" icon="grid" onPress={() => nav.push({ name: "catalog" })} />

      <View class="gap-1">
        <SectionTitle>Temporada</SectionTitle>
        <Text class="text-sm text-muted-foreground">Animes em exibição agora</Text>
      </View>

      <If when={props.seasonState === "pending"}>
        <View class="h-40">
          <Loading label="Carregando a temporada" />
        </View>
      </If>
      <If when={props.seasonState === "errored"}>
        <View class="h-48">
          <EmptyState
            title="Não foi possível carregar a temporada"
            detail={errorMessage(props.seasonError)}
            action="Tentar de novo"
            onAction={props.onRetry}
          />
        </View>
      </If>
    </View>
  );
}

/** Início: o que continuar, a entrada do catálogo e os animes da temporada. */
export function Home() {
  const season = resource(() => backend().season());
  refreshResume();

  const user = () => {
    const state = sessionState();
    return state.status === "signedIn" ? state.user : null;
  };

  return (
    <NavPage title="Início" headerVisible={false}>
      <View class="flex-1 bg-background pt-[env(safe-area-inset-top)]">
      <List<Media>
        each={season() ?? []}
        keyExtractor={(media) => String(media.id)}
        numColumns={3}
        estimatedItemSize={220}
        class="flex-1"
        contentClass="gap-3 px-4 pt-3 pb-[calc(env(safe-area-inset-bottom)+16px)]"
        header={<HomeHeader user={user()} seasonState={season.state()} seasonError={season.error()} onRetry={season.refetch} />}
      >
        {(media) => (
          <Button
            onPress={() => openMedia(media(), mediaTitle(media()))}
            accessibilityLabel={mediaTitle(media())}
            class="flex-1 gap-2 active:opacity-70"
            classList={{ "opacity-50": openingMedia() === media().id }}
          >
            <Cover src={mediaCover(media())} title={mediaTitle(media())} />
            <Text class="text-sm text-foreground" style={{ numberOfLines: 2 }}>
              {mediaTitle(media())}
            </Text>
          </Button>
        )}
      </List>
      </View>
    </NavPage>
  );
}

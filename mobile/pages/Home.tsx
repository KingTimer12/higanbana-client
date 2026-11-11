import { Image, List, resource, Scroll, signal, Text, View } from "nsh";
import { Button, Menu, MenuItem, NavPage } from "nsh/base";
import { POSTER_COLUMNS, POSTER_GRID, POSTER_HEIGHT, SeasonPoster } from "../components/Poster";
import { EpisodeFrame } from "../components/Episode";
import { EmptyState, IconButton, SectionCard, SectionTitle, Skeleton } from "../components/ui";
import { backend, type Media, type Resume, type User } from "../lib/backend";
import { errorMessage, seasonHeading } from "../lib/format";
import { cloudPathOf, nav, sessionState, signOut } from "../store";

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
    <Button onPress={open} accessibilityLabel={`Continuar ${props.item.title}, ${props.item.label}`} class="group w-60 gap-2">
      <EpisodeFrame seen={props.item.position / Math.max(props.item.duration, 1)} />
      <View class="gap-0.5">
        <Text class="text-sm font-semibold text-foreground" style={{ numberOfLines: 1 }}>
          {props.item.title}
        </Text>
        <Text class="text-xs text-muted-foreground" style={{ numberOfLines: 1 }}>
          {props.item.label}
        </Text>
      </View>
    </Button>
  );
}

/** `3 de 12 no acervo`, contando os que já se sabe; o resto ainda está sendo procurado. */
function SeasonCount(props: { season: Media[] }) {
  const known = () => props.season.map((m) => cloudPathOf(m.id)).filter((p) => p !== undefined);
  const inLibrary = () => known().filter(Boolean).length;
  return (
    <If when={props.season.length > 0}>
      <Text class="text-sm tabular-nums text-muted-foreground">
        {known().length < props.season.length ? "Procurando… " : ""}
        {inLibrary()} de {props.season.length} no acervo
      </Text>
    </If>
  );
}

function HomeHeader(props: { user: User | null; season: Media[]; seasonState: string; seasonError: unknown; onRetry: () => void }) {
  return (
    <View class="gap-8 pb-4">
      <View class="flex-row items-center gap-1">
        <View class="flex-1 gap-1">
          <Text role="header" class="text-2xl font-semibold text-foreground" style={{ numberOfLines: 1 }}>
            {props.user ? `Olá, ${props.user.name}` : "Bem-vindo de volta"}
          </Text>
          <Text class="text-sm text-muted-foreground">Busque um anime ou abra o catálogo.</Text>
        </View>
        <IconButton icon="search" label="Buscar" onPress={() => nav.push({ name: "search" })} />
        <If when={props.user}>{(user) => <AccountMenu user={user()} />}</If>
      </View>

      <SectionCard
        icon="tv"
        label="Animes"
        blurb="Catálogo de A a Z, por letra"
        onPress={() => nav.push({ name: "catalog" })}
        testID="section-animes"
      />

      <If when={resume().length > 0}>
        <View>
          <SectionTitle class="mb-4">Continuar assistindo</SectionTitle>
          <Scroll horizontal scrollbar={false} class="-mx-4" contentClass="flex-row gap-3 px-4">
            <Each of={resume()} key={(item) => item.path}>
              {(item) => <ResumeCard item={item()} />}
            </Each>
          </Scroll>
        </View>
      </If>

      <View>
        <SectionTitle aside={<SeasonCount season={props.season} />}>{seasonHeading()}</SectionTitle>
        <If when={props.seasonState === "pending"}>
          <View class="mt-4 flex-row gap-3">
            <Each of={[0, 1, 2]}>{() => <Skeleton class="aspect-2/3 flex-1 rounded-(--radius-lg)" />}</Each>
          </View>
        </If>
        <If when={props.seasonState === "errored"}>
          <View class="h-48">
            <EmptyState
              title="Não foi possível carregar a temporada agora"
              detail={errorMessage(props.seasonError)}
              action="Tentar de novo"
              onAction={props.onRetry}
            />
          </View>
        </If>
      </View>
    </View>
  );
}

/** Início: saudação, a entrada do catálogo, o que continuar e os animes da temporada. */
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
          numColumns={POSTER_COLUMNS}
          estimatedItemSize={POSTER_HEIGHT}
          class="flex-1"
          contentClass={`${POSTER_GRID} px-4 pt-4 pb-[calc(env(safe-area-inset-bottom)+24px)]`}
          header={
            <HomeHeader
              user={user()}
              season={season() ?? []}
              seasonState={season.state()}
              seasonError={season.error()}
              onRetry={season.refetch}
            />
          }
        >
          {(media) => <SeasonPoster media={media()} />}
        </List>
      </View>
    </NavPage>
  );
}

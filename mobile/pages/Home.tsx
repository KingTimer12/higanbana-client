import { Image, resource, Scroll, signal, Text, View } from "nsh";
import { Button, Menu, MenuItem } from "nsh/base";
import { EpisodeFrame } from "../components/Episode";
import { Discover, Highlights } from "../components/Showcase";
import { TAB_BAR_CLEARANCE } from "../components/TabBar";
import { UpdateBanner } from "../components/UpdateBanner";
import { IconButton, SectionTitle, TabHeader } from "../components/ui";
import { backend, type Resume, type User } from "../lib/backend";
import { checkForUpdate, currentUpdate, installUpdate, nav, sessionState, signOut } from "../store";

const resume = signal<Resume[]>([]);

/** Relê os episódios em andamento (ao abrir o início e ao sair do player). */
export function refreshResume() {
  try {
    resume.set(backend().continueWatching());
  } catch {
    resume.set([]);
  }
}

/** O item de atualização do menu: procurar, ou instalar a versão já achada. */
function updateLabel() {
  const state = currentUpdate();
  switch (state.status) {
    case "checking":
      return "Procurando atualizações…";
    case "available":
    case "ready":
      return `Atualizar para ${state.update.version}`;
    case "downloading":
      return `Baixando ${state.update.version}…`;
    default:
      return "Procurar atualizações";
  }
}

function updateAction() {
  const status = currentUpdate().status;
  if (status === "available" || status === "ready") installUpdate();
  else checkForUpdate(true);
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
      <MenuItem onSelect={updateAction} class="min-h-11 justify-center rounded-(--radius) px-3 py-2 active:bg-accent">
        <Text class="text-base text-popover-foreground">{updateLabel()}</Text>
      </MenuItem>
      <MenuItem onSelect={signOut} class="min-h-11 justify-center rounded-(--radius) px-3 py-2 active:bg-accent">
        <Text class="text-base text-popover-foreground">Sair</Text>
      </MenuItem>
      <View class="mt-1 border-t border-border px-3 pt-2 pb-1">
        <Text class="text-xs tabular-nums text-muted-foreground">Higanbana {backend().version()}</Text>
      </View>
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

/** Os episódios em andamento, do mais recente; some quando não há nenhum. */
function ContinueRow() {
  return (
    <If when={resume().length > 0}>
      <View class="gap-3">
        <View class="px-4">
          <SectionTitle>Continuar assistindo</SectionTitle>
        </View>
        <Scroll horizontal scrollbar={false} contentClass="flex-row gap-3 px-4">
          <Each of={resume()} key={(item) => item.path}>
            {(item) => <ResumeCard item={item()} />}
          </Each>
        </Scroll>
      </View>
    </If>
  );
}

/** A aba Início, na ordem do desktop: destaques do ano, o que continuar e o descobrir. */
export function Home() {
  const highlights = resource(() => backend().discover("popular", 6));
  refreshResume();

  const user = () => {
    const state = sessionState();
    return state.status === "signedIn" ? state.user : null;
  };

  return (
    <View class="flex-1 bg-background">
      <TabHeader
        title={user() ? `Olá, ${user()!.name}` : "Início"}
        trailing={
          <>
            <IconButton icon="search" label="Buscar" onPress={() => nav.push({ name: "search" })} />
            <If when={user()}>{(u) => <AccountMenu user={u()} />}</If>
          </>
        }
      />
      <Scroll class="flex-1" contentClass={`gap-8 pt-2 ${TAB_BAR_CLEARANCE}`} testID="home">
        <UpdateBanner />
        <Highlights list={highlights()} state={highlights.state()} error={highlights.error()} onRetry={highlights.refetch} />
        <ContinueRow />
        {/* As listas do descobrir esperam os destaques: dividem o saldo de listagens da nuvem. */}
        <Discover ready={highlights.state() !== "pending" && highlights.state() !== "unresolved"} />
      </Scroll>
    </View>
  );
}

// Estado do app: sessão, navegação e avisos. As telas leem os sinais e
// chamam as funções; ninguém escreve o estado de outra tela.
import { openUrl, signal } from "nsh";
import { createNavStack } from "nsh/base";
import { type AppUpdate, backend, type Comic, type Media, type MediaKind, type ReadProgress, type User } from "./lib/backend";
import { errorMessage, mediaCover } from "./lib/format";
import { KeyedSignals } from "./lib/keyed";

export type Route =
  | { name: "tabs" }
  | { name: "search"; kind: MediaKind }
  | { name: "title"; path: string; media: Media | null; kind: MediaKind }
  | { name: "player"; dir: string; file: string; title: string; malId: number | null }
  /** `file`: o capítulo aberto; `files`: os da pasta, para seguir ao próximo. */
  | { name: "reader"; file: Comic; files: Comic[]; title: string };

/** A pilha de telas: as abas embaixo, o que se abre a partir delas por cima. */
export const nav = createNavStack<Route>([{ name: "tabs" }]);

// --- Abas ---

/** As seções do app, na ordem da barra de abas. */
export type Tab = "home" | "animes" | "mangas" | "doramas" | "musicas";

const tab = signal<Tab>("home");

/** A aba aberta. */
export const currentTab = () => tab();

/** Abre uma aba (as já visitadas voltam como estavam). */
export function selectTab(next: Tab) {
  tab.set(next);
}

// --- Sessão ---

type SessionState = { status: "loading" } | { status: "signedOut" } | { status: "signedIn"; user: User };

const session = signal<SessionState>({ status: "loading" });
const loginPending = signal(false);
const loginError = signal<string | null>(null);

/** A sessão (acompanhada onde é lida). */
export const sessionState = () => session();
/** Há um login esperando o navegador voltar. */
export const isLoginPending = () => loginPending();
/** Por que o último login falhou, se falhou. */
export const loginFailure = () => loginError();

/** Lê a sessão guardada ao abrir o app. */
export async function restoreSession() {
  try {
    const user = await backend().user();
    session.set(user ? { status: "signedIn", user } : { status: "signedOut" });
  } catch {
    session.set({ status: "signedOut" });
  }
}

/** Espera a URL do login em andamento aparecer e a abre no navegador. */
async function openLoginPage() {
  for (let attempt = 0; attempt < 20 && loginPending(); attempt++) {
    const url = await backend().loginUrl();
    if (url) {
      await openUrl(url);
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

/** Login pelo Discord no navegador do sistema. */
export async function signIn() {
  if (loginPending()) return;
  loginPending.set(true);
  loginError.set(null);
  const done = backend().login();
  openLoginPage().catch(() => {});
  try {
    const user = await done;
    session.set({ status: "signedIn", user });
  } catch (error) {
    loginError.set(errorMessage(error));
  } finally {
    loginPending.set(false);
  }
}

/** Abre de novo a página de login (a aba foi fechada sem querer). */
export function reopenLogin() {
  openLoginPage().catch(() => {});
}

/** Desiste do login em andamento. */
export function cancelSignIn() {
  backend().cancelLogin().catch(() => {});
}

/** Sai da conta e volta ao início. */
export async function signOut() {
  await backend().logout().catch(() => {});
  nav.popToRoot();
  tab.set("home");
  session.set({ status: "signedOut" });
}

// --- Atualização ---

export type UpdateState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "available"; update: AppUpdate }
  | { status: "downloading"; update: AppUpdate; progress: number }
  | { status: "ready"; update: AppUpdate };

const updateState = signal<UpdateState>({ status: "idle" });
/** O aviso de versão nova foi fechado nesta sessão (o menu da conta ainda oferece). */
const updateDismissed = signal(false);

/** Onde está a atualização do app. */
export const currentUpdate = () => updateState();
/** O aviso de versão nova aparece no início. */
export const showUpdateBanner = () => updateState().status !== "idle" && updateState().status !== "checking" && !updateDismissed();

/** Procura uma versão nova. `manual`: a pessoa pediu, e ouve a resposta mesmo sem versão nova. */
export async function checkForUpdate(manual = false) {
  const state = updateState();
  if (state.status !== "idle") {
    // Já achada: o pedido manual mostra o aviso de novo.
    if (manual) updateDismissed.set(false);
    return;
  }
  updateState.set({ status: "checking" });
  try {
    const found = await backend().checkUpdate();
    updateState.set(found ? { status: "available", update: found } : { status: "idle" });
    if (!found && manual) notify(`Você está na versão mais nova (${backend().version()}).`);
  } catch (error) {
    updateState.set({ status: "idle" });
    if (manual) notify(`Não foi possível procurar atualizações. ${errorMessage(error)}`);
  }
}

/** Baixa a versão nova (se ainda não baixou) e abre o instalador do sistema. */
export async function installUpdate() {
  const state = updateState();
  if (state.status !== "available" && state.status !== "ready") return;
  const update = state.update;
  if (state.status === "available") {
    updateState.set({ status: "downloading", update, progress: 0 });
    const timer = setInterval(() => {
      const now = updateState();
      if (now.status === "downloading") updateState.set({ ...now, progress: backend().updateProgress() });
    }, 250);
    try {
      await backend().downloadUpdate();
      updateState.set({ status: "ready", update });
    } catch (error) {
      updateState.set({ status: "available", update });
      notify(`Não foi possível baixar a atualização. ${errorMessage(error)}`);
      return;
    } finally {
      clearInterval(timer);
    }
  }
  const opened = await backend()
    .installUpdate()
    .catch(() => false);
  if (!opened) notify("Não foi possível abrir o instalador.");
}

/** Esconde o aviso de versão nova até o app abrir de novo. */
export function dismissUpdate() {
  updateDismissed.set(true);
}

// --- Avisos ---

export interface Notice {
  id: number;
  text: string;
}

const notices = signal<Notice[]>([]);
let noticeId = 0;

/** Os avisos na tela, do mais antigo ao mais novo. */
export const currentNotices = () => notices();

/** Mostra um aviso curto, que some sozinho. */
export function notify(text: string) {
  const id = ++noticeId;
  notices.update((list) => [...list, { id, text }]);
  setTimeout(() => dismissNotice(id), 3500);
}

export function dismissNotice(id: number) {
  notices.update((list) => list.filter((n) => n.id !== id));
}

// --- O que se sabe de cada card, pedido quando ele aparece ---
//
// Como no desktop: cada pôster pede o que falta ao entrar na tela (a lista só
// monta os cards perto da área visível), e os pedidos saem em lote. O estado
// fica aqui, pela chave do item, nunca no card: a lista recicla os cards.

/** Metadados por seção e nome de pasta: `null` = sem correspondência no AniList; ausente = ainda não se sabe. */
const lookups = new KeyedSignals<string, Media | null>();
const lookupQueue: { kind: MediaKind; name: string }[] = [];
const lookupAsked = new Set<string>();
let lookupRunning = false;
/** Nomes por pedido ao núcleo (ele casa e guarda em cache). */
const LOOKUP_BATCH = 24;

/** Um mesmo nome de pasta pode ser anime e mangá: a chave leva a seção. */
const lookupKey = (name: string, kind: MediaKind) => `${kind}:${name}`;

/** A mídia de uma pasta de obra: `undefined` enquanto não se sabe, `null` sem correspondência. */
export const lookupOf = (name: string, kind: MediaKind = "anime"): Media | null | undefined => lookups.get(lookupKey(name, kind));

/** Pede os metadados de uma pasta (uma vez por nome e seção). */
export function wantLookup(name: string, kind: MediaKind = "anime") {
  const key = lookupKey(name, kind);
  if (lookupAsked.has(key)) return;
  lookupAsked.add(key);
  lookupQueue.push({ kind, name });
  if (!lookupRunning) {
    lookupRunning = true;
    // Junta os cards que aparecem no mesmo quadro num pedido só.
    setTimeout(runLookups, 60);
  }
}

async function runLookups() {
  while (lookupQueue.length > 0) {
    // Um pedido por seção: os primeiros da fila decidem qual.
    const kind = lookupQueue[0].kind;
    const batch: string[] = [];
    for (let i = 0; i < lookupQueue.length && batch.length < LOOKUP_BATCH; ) {
      if (lookupQueue[i].kind === kind) batch.push(lookupQueue.splice(i, 1)[0].name);
      else i++;
    }
    try {
      const found = await backend().lookup(batch, kind);
      lookups.setMany(batch.map((name, i): [string, Media | null] => [lookupKey(name, kind), found[i] ?? null]));
    } catch {
      // Sem rede: o card fica com a capa tipográfica e tenta de novo ao reaparecer.
      lookups.setMany(batch.map((name): [string, Media | null] => [lookupKey(name, kind), null]));
      for (const name of batch) lookupAsked.delete(lookupKey(name, kind));
    }
  }
  lookupRunning = false;
}

/** Capas do fanart.tv por id do AniList: `null` = sem capa de lá; ausente = ainda não se sabe. */
const covers = new KeyedSignals<number, string | null>();
const coverQueue: Media[] = [];
const coverAsked = new Set<number>();
let coverRunning = false;

/** A capa para mostrar: a do fanart se já chegou, senão a do AniList. */
export const coverOf = (media: Media | null | undefined): string | null => (media && covers.get(media.id)) || mediaCover(media);

/** Pede a capa do fanart de uma mídia (uma vez por id; um pedido de cada vez, como a nuvem). */
export function wantCover(media: Media) {
  if (coverAsked.has(media.id)) return;
  coverAsked.add(media.id);
  coverQueue.push(media);
  if (!coverRunning) {
    coverRunning = true;
    runCovers().catch(() => {});
  }
}

async function runCovers() {
  while (coverQueue.length > 0) {
    const media = coverQueue.shift()!;
    try {
      const uri = await backend().cover(media);
      covers.set(media.id, uri);
    } catch {
      // Fica com a capa do AniList; o pedido é liberado e pode ser refeito se o card pedir de novo.
      coverAsked.delete(media.id);
    }
  }
  coverRunning = false;
}

// --- Progresso visto nesta sessão ---

export interface Watched {
  position: number;
  duration: number;
  watched: boolean;
}

const progress = new KeyedSignals<string, Watched>();

/** O progresso mais recente de um arquivo, se ele tocou desde que o app abriu. */
export const watchedOf = (path: string): Watched | undefined => progress.get(path);

/** O player avisa onde parou, para as listas de episódios mostrarem sem recarregar. */
export function recordWatched(path: string, watched: Watched) {
  progress.set(path, watched);
}

// --- Leitura nesta sessão ---

/** No arquivo do capítulo: a página (ou, numa coletânea, o capítulo interno) e se terminou. */
export interface Reading extends ReadProgress {
  unitName: string | null;
}

const reading = new KeyedSignals<string, Reading>();

/** O progresso mais recente de um arquivo de mangá, se ele foi lido desde que o app abriu. */
export const readingOf = (path: string): Reading | undefined => reading.get(path);

/** O leitor avisa onde parou, para a lista de capítulos mostrar sem recarregar. */
export function recordReading(path: string, value: Reading) {
  reading.set(path, value);
}

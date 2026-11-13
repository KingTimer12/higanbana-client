// Estado do app: sessão, navegação e avisos. As telas leem os sinais e
// chamam as funções; ninguém escreve o estado de outra tela.
import { openUrl, signal } from "nsh";
import { createNavStack } from "nsh/base";
import { backend, type Media, type User } from "./lib/backend";
import { errorMessage, mediaCover } from "./lib/format";
import { KeyedSignals } from "./lib/keyed";

export type Route =
  | { name: "home" }
  | { name: "catalog" }
  | { name: "search" }
  | { name: "title"; path: string; media: Media | null }
  | { name: "player"; dir: string; file: string; title: string; malId: number | null };

/** A pilha de telas. */
export const nav = createNavStack<Route>([{ name: "home" }]);

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
  session.set({ status: "signedOut" });
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

// --- Abrir obras ---

const opening = signal<number | null>(null);

/** Id da mídia do AniList que está sendo procurada na nuvem. */
export const openingMedia = () => opening();

/** Abre a obra de uma mídia do AniList, se ela está na nuvem. */
export async function openMedia(media: Media, title: string) {
  const known = cloudPathOf(media.id);
  if (known) {
    nav.push({ name: "title", path: known, media });
    return;
  }
  if (known === null) {
    notify(`“${title}” ainda não está na nuvem.`);
    return;
  }
  if (opening() !== null) return;
  opening.set(media.id);
  try {
    const path = await backend().find(media);
    cloudPaths.set(media.id, path);
    if (path) nav.push({ name: "title", path, media });
    else notify(`“${title}” ainda não está na nuvem.`);
  } catch (error) {
    notify(`Não foi possível abrir “${title}”. ${errorMessage(error)}`);
  } finally {
    opening.set(null);
  }
}

// --- O que se sabe de cada card, pedido quando ele aparece ---
//
// Como no desktop: cada pôster pede o que falta ao entrar na tela (a lista só
// monta os cards perto da área visível), e os pedidos saem em lote. O estado
// fica aqui, pela chave do item, nunca no card: a lista recicla os cards.

/** Metadados por nome de pasta: `null` = sem correspondência no AniList; ausente = ainda não se sabe. */
const lookups = new KeyedSignals<string, Media | null>();
const lookupQueue: string[] = [];
const lookupAsked = new Set<string>();
let lookupRunning = false;
/** Nomes por pedido ao núcleo (ele casa e guarda em cache). */
const LOOKUP_BATCH = 24;

/** A mídia de uma pasta de obra: `undefined` enquanto não se sabe, `null` sem correspondência. */
export const lookupOf = (name: string): Media | null | undefined => lookups.get(name);

/** Pede os metadados de uma pasta (uma vez por nome). */
export function wantLookup(name: string) {
  if (lookupAsked.has(name)) return;
  lookupAsked.add(name);
  lookupQueue.push(name);
  if (!lookupRunning) {
    lookupRunning = true;
    // Junta os cards que aparecem no mesmo quadro num pedido só.
    setTimeout(runLookups, 60);
  }
}

async function runLookups() {
  while (lookupQueue.length > 0) {
    const batch = lookupQueue.splice(0, LOOKUP_BATCH);
    try {
      const found = await backend().lookup(batch);
      lookups.setMany(batch.map((name, i): [string, Media | null] => [name, found[i] ?? null]));
    } catch {
      // Sem rede: o card fica com a capa tipográfica e tenta de novo ao reaparecer.
      lookups.setMany(batch.map((name): [string, Media | null] => [name, null]));
      for (const name of batch) lookupAsked.delete(name);
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

/** Caminho na nuvem por id do AniList: `null` = fora do acervo; ausente = ainda não se sabe. */
const cloudPaths = new KeyedSignals<number, string | null>();
const cloudQueue: Media[] = [];
const cloudAsked = new Set<number>();
let cloudRunning = false;

/** Onde está a obra de uma mídia: `undefined` enquanto não se sabe, `null` fora do acervo. */
export const cloudPathOf = (id: number): string | null | undefined => cloudPaths.get(id);

/** Pede onde está a obra de uma mídia da temporada (um de cada vez: a nuvem limita buscas seguidas). */
export function wantCloudPath(media: Media) {
  if (cloudAsked.has(media.id)) return;
  cloudAsked.add(media.id);
  cloudQueue.push(media);
  if (!cloudRunning) {
    cloudRunning = true;
    runCloudPaths().catch(() => {});
  }
}

async function runCloudPaths() {
  while (cloudQueue.length > 0) {
    const media = cloudQueue.shift()!;
    try {
      const path = await backend().find(media);
      cloudPaths.set(media.id, path);
    } catch {
      // Fica sem marca; tenta de novo quando o card reaparecer.
      cloudAsked.delete(media.id);
    }
  }
  cloudRunning = false;
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

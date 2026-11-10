// Estado do app: sessão, navegação e avisos. As telas leem os sinais e
// chamam as funções; ninguém escreve o estado de outra tela.
import { openUrl, signal } from "nsh";
import { createNavStack } from "nsh/base";
import { backend, type Media, type User } from "./lib/backend";
import { errorMessage } from "./lib/format";

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
  if (opening() !== null) return;
  opening.set(media.id);
  try {
    const path = await backend().find(media);
    if (path) nav.push({ name: "title", path, media });
    else notify(`“${title}” ainda não está na nuvem.`);
  } catch (error) {
    notify(`Não foi possível abrir “${title}”. ${errorMessage(error)}`);
  } finally {
    opening.set(null);
  }
}

// --- Progresso visto nesta sessão ---

export interface Watched {
  position: number;
  duration: number;
  watched: boolean;
}

const progress = signal<Record<string, Watched>>({});

/** O progresso mais recente de um arquivo, se ele tocou desde que o app abriu. */
export const watchedOf = (path: string): Watched | undefined => progress()[path];

/** O player avisa onde parou, para as listas de episódios mostrarem sem recarregar. */
export function recordWatched(path: string, watched: Watched) {
  progress.update((all) => ({ ...all, [path]: watched }));
}

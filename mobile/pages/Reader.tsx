// O leitor de mangá no telefone: as páginas do capítulo uma embaixo da outra,
// na largura da tela, como nos apps de leitura. Um toque mostra ou esconde a
// barra de cima (voltar, título) e a de baixo (página e próximo capítulo).
//
// O núcleo abre o `.cbz` da nuvem por pedaços e guarda cada página no cache;
// a página chega aqui como `file://`. Um arquivo pode ter vários capítulos
// dentro (coletâneas): eles se leem em sequência, e depois vem o próximo arquivo.
import { cleanup, effect, Image, List, resource, signal, Text, untracked, View } from "nsh";
import { Button, NavPage } from "nsh/base";
import { EmptyState, IconButton, Loading, OutlineButton, PrimaryButton } from "../components/ui";
import { backend, type Comic, type ReadUnit } from "../lib/backend";
import { errorMessage } from "../lib/format";
import { KeyedSignals } from "../lib/keyed";
import { nav, readingOf, recordReading } from "../store";

/** Largura/altura de uma página antes de ela carregar (a de um mangá comum). */
const PAGE_RATIO = 0.7;
/** Páginas pedidas adiante da última visível: chegam antes de o dedo alcançá-las. */
const AHEAD = 6;
/** Espera depois da última rolagem antes de gravar onde parou. */
const SAVE_DELAY = 800;

type PageState = { uri: string } | { error: string };

export function Reader(props: { file: Comic; files: Comic[]; title: string }) {
  const file = signal(props.file);
  /** Capítulo interno escolhido; `null` = onde parou (ou o primeiro). */
  const unitChoice = signal<number | null>(null);
  const units = resource(file, (f) => backend().readerUnits(f.path, f.size));
  const unitList = () => (units.state() === "ready" ? (units() ?? []) : []);
  const unitIndex = () => {
    const chosen = unitChoice();
    if (chosen !== null) return chosen;
    const stopped = readingOf(file().path)?.unitName ?? file().unitName;
    return Math.max(0, stopped ? unitList().findIndex((u) => u.name === stopped) : 0);
  };
  const unit = (): ReadUnit | undefined => unitList()[unitIndex()];
  // As páginas levam a chave do capítulo: ao trocar de capítulo, as do anterior
  // não valem nem pelo instante antes do pedido novo sair.
  const pages = resource(unit, async (u) => ({
    key: u.key,
    list: await backend().readerPages(untracked(file).path, untracked(file).size, u),
  }));
  const ready = () => pages.state() === "ready" && pages()?.key === unit()?.key;
  const pageList = () => (ready() ? (pages()?.list ?? []) : []);

  // As páginas por capítulo e posição, fora das células: a lista as recicla.
  const loaded = new KeyedSignals<string, PageState>();
  const ratios = new KeyedSignals<string, number>();
  const asked = new Set<string>();
  const pageKey = (u: ReadUnit, index: number) => `${u.key}#${index}`;
  const want = (u: ReadUnit, index: number) => {
    const key = pageKey(u, index);
    if (asked.has(key)) return;
    asked.add(key);
    const f = untracked(file);
    backend()
      .readerPage(f.path, f.size, u, index)
      .then(
        (page) => {
          // A proporção antes da imagem: a célula já nasce na altura certa e não pula.
          if (page.ratio) ratios.set(key, page.ratio);
          loaded.set(key, { uri: page.uri });
        },
        (error) => {
          asked.delete(key);
          loaded.set(key, { error: errorMessage(error) });
        },
      );
  };

  // Onde a leitura está: a última página visível do capítulo aberto.
  const current = signal(0);
  const chrome = signal(true);

  /** Grava a página atual no núcleo e avisa a lista de capítulos. */
  const save = () => {
    const u = unit();
    const total = pageList().length;
    if (!u || total === 0) return;
    const page = Math.min(current(), total - 1);
    const done = page + 1 >= total;
    const f = file();
    const count = unitList().length;
    const before = readingOf(f.path)?.read ?? f.read;
    backend().saveReading(u.key, page, total);
    if (count > 1) {
      backend().saveReadingSummary(f.path, unitIndex(), count, done, u);
      recordReading(f.path, { page: unitIndex(), total: count, read: before || (done && unitIndex() + 1 >= count), unitName: u.name });
    } else {
      recordReading(f.path, { page, total, read: before || done, unitName: null });
    }
  };
  let timer: ReturnType<typeof setTimeout> | undefined;
  const saveSoon = () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(save, SAVE_DELAY);
  };
  cleanup(() => {
    if (timer) {
      clearTimeout(timer);
      save();
    }
  });

  // Ao abrir um capítulo, volta para a página onde parou (se não terminou).
  const startPage = () => {
    const u = unit();
    const seen = u ? backend().readingOf(u.key) : null;
    return seen && !seen.read ? Math.min(seen.page, Math.max(0, pageList().length - 1)) : 0;
  };
  effect(() => {
    if (ready()) current.set(untracked(startPage));
  });

  /** O que vem depois: o próximo capítulo do arquivo ou o próximo arquivo da pasta. */
  const nextFile = () => {
    const at = props.files.findIndex((c) => c.path === file().path);
    return at >= 0 ? props.files[at + 1] : undefined;
  };
  const hasNext = () => unitIndex() + 1 < unitList().length || !!nextFile();
  const nextLabel = () => {
    const inner = unitList()[unitIndex() + 1];
    return inner?.label ?? nextFile()?.label ?? "";
  };
  const goNext = () => {
    if (timer) {
      clearTimeout(timer);
      timer = undefined;
    }
    // Seguir adiante conta este capítulo como lido.
    current.set(Math.max(0, pageList().length - 1));
    save();
    if (unitIndex() + 1 < unitList().length) {
      unitChoice.set(unitIndex() + 1);
      return;
    }
    const next = nextFile();
    if (!next) return;
    unitChoice.set(0);
    file.set(next);
  };
  const heading = () => {
    const u = unit();
    // Numa coletânea, o capítulo interno; senão, o rótulo do arquivo.
    return u && unitList().length > 1 ? u.label : file().label;
  };

  const footer = (
    <View class="items-center gap-3 px-4 pt-8 pb-[calc(env(safe-area-inset-bottom)+32px)]">
      <Text class="text-sm text-muted-foreground">Fim de {heading()}</Text>
      <If when={hasNext()} else={<OutlineButton label="Voltar à obra" onPress={() => nav.pop()} />}>
        <PrimaryButton icon="next" label={`Próximo: ${nextLabel()}`} onPress={goNext} />
      </If>
    </View>
  );

  return (
    <NavPage title={props.title} headerVisible={false} statusBar={{ hidden: !chrome(), foreground: "light" }} class="bg-video">
      <View class="flex-1 bg-video">
        <If when={units.state() === "pending" || pages.state() === "pending"}>
          <Loading label="Abrindo o capítulo" />
        </If>
        <If when={units.error() ?? pages.error()}>
          {(error) => (
            <EmptyState
              title="Não foi possível abrir o capítulo"
              detail={errorMessage(error())}
              action="Tentar de novo"
              onAction={() => (units.error() ? units.refetch() : pages.refetch())}
            />
          )}
        </If>
        {/* Uma lista por capítulo: trocar de capítulo monta outra, já na página certa. */}
        <If when={ready() ? unit() : undefined}>
          {(u) => (
            <List<number>
              each={pageList()}
              keyExtractor={(index) => pageKey(u(), index)}
              estimatedItemSize={560}
              initialScrollIndex={untracked(startPage)}
              viewabilityConfig={{ viewAreaCoverageThreshold: 0.5 }}
              onViewableItemsChange={({ viewableItems }) => {
                if (viewableItems.length === 0) return;
                const last = Math.max(...viewableItems.map((t) => t.index));
                for (let i = last + 1; i <= last + AHEAD && i < pageList().length; i++) want(u(), pageList()[i]);
                if (last !== current()) {
                  current.set(last);
                  saveSoon();
                }
              }}
              class="flex-1"
              footer={footer}
              empty={<EmptyState title="Nenhuma página neste capítulo" />}
            >
              {(index) => (
                <PageView
                  state={loaded.get(pageKey(u(), index()))}
                  ratio={ratios.get(pageKey(u(), index())) ?? PAGE_RATIO}
                  label={`Página ${pageList().indexOf(index()) + 1}`}
                  onShow={() => want(u(), index())}
                  onSize={(ratio) => {
                    // Só quando o núcleo não soube (AVIF): mudar a altura à toa faz a lista pular.
                    const key = pageKey(u(), index());
                    if (Math.abs((ratios.get(key) ?? 0) - ratio) > 0.01) ratios.set(key, ratio);
                  }}
                  onFail={(message) => {
                    // O arquivo do cache não abriu (formato que o sistema não lê, como AVIF em Android antigo).
                    asked.delete(pageKey(u(), index()));
                    loaded.set(pageKey(u(), index()), { error: message });
                  }}
                  onRetry={() => want(u(), index())}
                  onPress={() => chrome.update((c) => !c)}
                />
              )}
            </List>
          )}
        </If>

        <If when={chrome()}>
          <View class="absolute top-0 right-0 left-0 flex-row items-center gap-1 bg-scrim/70 px-2 pt-[calc(env(safe-area-inset-top)+4px)] pb-2">
            <IconButton icon="back" label="Fechar o leitor" onPress={() => nav.pop()} class="active:bg-on-video/20" iconClass="tint-on-video" />
            <View class="flex-1">
              <Text class="text-base font-semibold text-on-video" style={{ numberOfLines: 1 }}>
                {props.title}
              </Text>
              <Text class="text-sm text-on-video-muted" style={{ numberOfLines: 1 }}>
                {heading()}
              </Text>
            </View>
          </View>
          <If when={pageList().length > 0}>
            <View class="absolute right-0 bottom-0 left-0 flex-row items-center justify-between gap-3 bg-scrim/70 px-4 pt-2 pb-[calc(env(safe-area-inset-bottom)+8px)]">
              <Text class="text-sm tabular-nums text-on-video" testID="reader-page">
                {`Página ${Math.min(current(), pageList().length - 1) + 1} de ${pageList().length}`}
              </Text>
              <If when={hasNext()}>
                <IconButton icon="next" label="Próximo capítulo" onPress={goNext} class="active:bg-on-video/20" iconClass="tint-on-video" />
              </If>
            </View>
          </If>
        </If>
      </View>
    </NavPage>
  );
}

/** Uma página na largura da tela, na proporção dela (a de um mangá até carregar). */
function PageView(props: {
  state: PageState | undefined;
  ratio: number;
  label: string;
  onShow: () => void;
  onSize: (ratio: number) => void;
  onFail: (message: string) => void;
  onRetry: () => void;
  onPress: () => void;
}) {
  // A célula é reciclada: o pedido segue a página que ela mostra agora.
  effect(() => {
    if (!props.state) props.onShow();
  });
  return (
    // A proporção fica no `View` de fora: o botão só cobre a página para o toque.
    <View class="w-full" style={{ aspectRatio: props.ratio }}>
    <Button onPress={props.onPress} accessibilityLabel={props.label} class="absolute inset-0">
      <If when={props.state && "uri" in props.state ? props.state.uri : undefined}>
        {(uri) => (
          <Image
            src={uri()}
            fadeDuration={0}
            class="size-full object-contain"
            onLoad={({ size }) => {
              if (size.width > 0 && size.height > 0) props.onSize(size.width / size.height);
            }}
            onError={({ message }) => props.onFail(message || "a imagem não abriu")}
          />
        )}
      </If>
      {/* Fundo parado, sem o pulso do esqueleto: rolando, várias páginas piscariam juntas. */}
      <If when={!props.state}>
        <View class="absolute inset-0 items-center justify-center">
          <Text class="text-sm text-on-video-muted">{props.label}</Text>
        </View>
      </If>
      <If when={props.state && "error" in props.state ? props.state.error : undefined}>
        {(error) => (
          <View class="absolute inset-0 items-center justify-center gap-3 p-6">
            <Text class="text-center text-sm text-on-video-muted">{`${props.label}: ${error()}`}</Text>
            <OutlineButton label="Tentar de novo" onPress={props.onRetry} />
          </View>
        )}
      </If>
    </Button>
    </View>
  );
}

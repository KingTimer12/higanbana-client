// Os baixados: episódios e capítulos guardados no aparelho, por obra, para ver
// sem internet. Abrir um baixado vai direto ao player ou ao leitor, que usam a
// cópia; daqui também se apaga uma cópia ou se tenta de novo o que falhou.
import { List, Text, View } from "nsh";
import { Button, NavPage } from "nsh/base";
import { offlinePercent } from "../components/Download";
import { EmptyState, Icon, IconButton, PageHeader, ProgressBar, SYSTEM_HEADER } from "../components/ui";
import type { Comic, OfflineItem } from "../lib/backend";
import { downloadOffline, nav, offlineItems, refreshOffline, removeOffline } from "../store";

/** Altura estimada de uma linha (a lista mede a real). */
const ROW_HEIGHT = 64;

type Row = { kind: "work"; key: string; title: string; count: number } | { kind: "file"; key: string; item: OfflineItem };

const parentOf = (path: string) => path.slice(0, Math.max(0, path.lastIndexOf("/")));
const byName = (a: OfflineItem, b: OfflineItem) => a.name.localeCompare(b.name, undefined, { numeric: true });

/** `1,2 GB`, `350 MB`. */
function formatSize(bytes: number): string {
  const gb = bytes / 1024 ** 3;
  return gb >= 1 ? `${gb.toFixed(1).replace(".", ",")} GB` : `${Math.max(1, Math.round(bytes / 1024 ** 2))} MB`;
}

/** As obras em ordem alfabética; dentro de cada uma, os arquivos em ordem natural. */
function rowsOf(items: OfflineItem[]): Row[] {
  const works = new Map<string, OfflineItem[]>();
  for (const item of items) works.set(item.workPath, [...(works.get(item.workPath) ?? []), item]);
  const groups = [...works.values()].sort((a, b) => a[0].workTitle.localeCompare(b[0].workTitle));
  return groups.flatMap((files) => [
    { kind: "work" as const, key: `work:${files[0].workPath}`, title: files[0].workTitle, count: files.length },
    ...[...files].sort(byName).map((item) => ({ kind: "file" as const, key: item.path, item })),
  ]);
}

/** Um capítulo baixado como o leitor o recebe (o progresso o leitor busca pela chave). */
const comicOf = (item: OfflineItem): Comic => ({
  name: item.name,
  path: item.path,
  size: item.size,
  label: item.label,
  page: 0,
  total: 0,
  read: false,
  unitName: null,
});

function open(item: OfflineItem) {
  if (item.kind === "manga") {
    // Seguir ao próximo capítulo só pelos baixados da mesma pasta: sem rede, os outros não abrem.
    const files = offlineItems()
      .filter((o) => o.kind === "manga" && o.status === "done" && parentOf(o.path) === parentOf(item.path))
      .sort(byName)
      .map(comicOf);
    nav.push({ name: "reader", file: comicOf(item), files, title: item.workTitle });
  } else {
    nav.push({ name: "player", dir: parentOf(item.path), file: item.name, title: item.workTitle, malId: null });
  }
}

function status(item: OfflineItem): string {
  switch (item.status) {
    case "done":
      return formatSize(item.size);
    case "downloading":
      return `${offlinePercent(item.downloaded, item.size)}% de ${formatSize(item.size)}`;
    case "queued":
      return "Na fila";
    default:
      return item.error ? `Falhou: ${item.error}` : "Falhou";
  }
}

function FileRow(props: { item: OfflineItem }) {
  const done = () => props.item.status === "done";
  return (
    <View class="flex-row items-center gap-1">
      <Button
        onPress={() => open(props.item)}
        disabled={!done()}
        accessibilityLabel={done() ? `Abrir ${props.item.label}` : `${props.item.label}, ${status(props.item)}`}
        testID={`offline-${props.item.name}`}
        class="min-h-14 flex-1 flex-row items-center gap-3 rounded-(--radius-lg) border border-border bg-card px-4 py-3 active:bg-accent"
      >
        <View class="size-9 items-center justify-center rounded-full bg-muted">
          <Icon name={props.item.kind === "manga" ? "book" : "play"} class="size-4 tint-muted-foreground" />
        </View>
        <View class="flex-1 gap-1">
          <Text class="text-sm font-semibold text-foreground" style={{ numberOfLines: 1 }}>
            {props.item.label}
          </Text>
          <If when={props.item.status === "downloading"}>
            <ProgressBar value={props.item.size > 0 ? props.item.downloaded / props.item.size : 0} label="Progresso do download" />
          </If>
          <Text
            class="text-xs tabular-nums text-muted-foreground"
            classList={{ "text-destructive": props.item.status === "failed" }}
            style={{ numberOfLines: 1 }}
          >
            {status(props.item)}
          </Text>
        </View>
      </Button>
      <If when={props.item.status === "failed"}>
        <IconButton icon="download" label={`Tentar baixar ${props.item.label} de novo`} onPress={() => downloadOffline(props.item)} />
      </If>
      <IconButton
        icon="trash"
        label={done() ? `Apagar ${props.item.label} do aparelho` : `Cancelar ${props.item.label}`}
        onPress={() => removeOffline(props.item.path)}
        iconClass="size-5 tint-muted-foreground"
      />
    </View>
  );
}

export function Downloads() {
  refreshOffline();
  const rows = () => rowsOf(offlineItems());
  const used = () => offlineItems().filter((i) => i.status === "done").reduce((sum, i) => sum + i.size, 0);
  return (
    <NavPage title="Baixados" headerClass={SYSTEM_HEADER} header={<PageHeader title="Baixados" onBack={() => nav.pop()} />}>
      <List<Row>
        each={rows()}
        keyExtractor={(row) => row.key}
        estimatedItemSize={ROW_HEIGHT}
        class="flex-1 bg-background"
        contentClass="gap-y-2 px-4 pt-2 pb-[calc(env(safe-area-inset-bottom)+24px)]"
        header={
          <If when={used() > 0}>
            <Text class="pb-2 text-sm tabular-nums text-muted-foreground">{`${formatSize(used())} no aparelho`}</Text>
          </If>
        }
        empty={
          <View class="h-80">
            <EmptyState
              title="Nada baixado ainda"
              detail="Toque no botão de baixar ao lado de um episódio ou capítulo para ver sem internet."
            />
          </View>
        }
      >
        {(row) => (
          // A célula é reciclada: lê a linha a cada vez, nunca guarda a de quando nasceu.
          <>
            <If when={row().kind === "work" ? (row() as Extract<Row, { kind: "work" }>) : undefined}>
              {(work) => (
                <View class="flex-row items-baseline justify-between pt-4">
                  <Text role="header" class="flex-1 text-base font-semibold text-foreground" style={{ numberOfLines: 1 }}>
                    {work().title}
                  </Text>
                  <Text class="text-sm tabular-nums text-muted-foreground">{work().count === 1 ? "1 arquivo" : `${work().count} arquivos`}</Text>
                </View>
              )}
            </If>
            <If when={row().kind === "file" ? (row() as Extract<Row, { kind: "file" }>).item : undefined}>
              {(item) => <FileRow item={item()} />}
            </If>
          </>
        )}
      </List>
    </NavPage>
  );
}

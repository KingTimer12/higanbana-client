// O botão de baixar de um episódio ou capítulo, ao lado da linha: baixa para ver
// sem internet e mostra onde o download está (na fila, quanto falta, pronto).
// O estado vem do núcleo pela chave do arquivo: a lista recicla as linhas.
import { Text, View } from "nsh";
import { Button } from "nsh/base";
import type { OfflineRequest } from "../lib/backend";
import { downloadOffline, offlineOf } from "../store";
import { Icon } from "./ui";

/** Quanto já baixou, em por cento inteiro. */
export function offlinePercent(downloaded: number, size: number): number {
  return size > 0 ? Math.min(99, Math.floor((downloaded / size) * 100)) : 0;
}

export function DownloadButton(props: { item: OfflineRequest }) {
  const state = () => offlineOf(props.item.path);
  const status = () => state()?.status;
  const label = () => {
    switch (status()) {
      case "done":
        return `${props.item.label} baixado`;
      case "downloading":
        return `Baixando ${props.item.label}, ${offlinePercent(state()?.downloaded ?? 0, props.item.size)}%`;
      case "queued":
        return `${props.item.label} na fila`;
      case "failed":
        return `Tentar baixar ${props.item.label} de novo`;
      default:
        return `Baixar ${props.item.label}`;
    }
  };
  // Baixando ou pronto, o toque não faz nada: apagar fica na tela de baixados.
  const busy = () => status() === "queued" || status() === "downloading" || status() === "done";
  return (
    <Button
      onPress={() => downloadOffline(props.item)}
      disabled={busy()}
      accessibilityLabel={label()}
      testID={`download-${props.item.name}`}
      class="size-11 items-center justify-center rounded-full active:bg-accent"
    >
      <If when={status() === "done"}>
        <Icon name="check" class="size-5 tint-success" />
      </If>
      <If when={status() === "downloading"}>
        <Text class="text-xs font-semibold tabular-nums text-primary">{`${offlinePercent(state()?.downloaded ?? 0, props.item.size)}%`}</Text>
      </If>
      <If when={status() === "queued"}>
        <View class="size-2 rounded-full bg-muted-foreground" />
      </If>
      <If when={status() === undefined || status() === "failed"}>
        <Icon name="download" class={status() === "failed" ? "size-5 tint-destructive" : "size-5 tint-muted-foreground"} />
      </If>
    </Button>
  );
}

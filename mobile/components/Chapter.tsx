// Um capítulo de mangá na lista da obra: o rótulo (`Cap. 12`) em destaque, o nome
// do arquivo embaixo quando diz mais, e onde a leitura parou. O mesmo ritmo da
// linha de episódio, sem a miniatura: o arquivo não tem capa própria.
import { Text, View } from "nsh";
import { Button } from "nsh/base";
import type { Comic } from "../lib/backend";
import { Icon, ProgressBar } from "./ui";

/** Altura estimada de uma linha de capítulo (a lista mede a real). */
export const CHAPTER_HEIGHT = 68;

/** Começou e não terminou: mostra a página. */
export function inProgress(chapter: Comic): boolean {
  return !chapter.read && chapter.total > 0 && (chapter.page > 0 || chapter.unitName !== null);
}

/** `pág. 5 de 20`, ou o capítulo interno da coletânea onde parou. */
export function whereStopped(chapter: Comic): string {
  return chapter.unitName ?? `pág. ${chapter.page + 1} de ${chapter.total}`;
}

/** Só letras e números, sem zeros à esquerda: `Cap 01` e `Cap. 1` dão o mesmo. */
const bare = (text: string) => text.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "").replace(/(^|\D)0+(\d)/g, "$1$2");

export function ChapterRow(props: { chapter: Comic; onPress: () => void }) {
  const name = () => props.chapter.name.replace(/\.[a-z0-9]{2,4}$/i, "");
  // O nome do arquivo embaixo só quando diz mais que o rótulo (um título, a fonte).
  const extra = () => bare(name()) !== bare(props.chapter.label);
  return (
    <Button
      onPress={props.onPress}
      testID={`chapter-${props.chapter.name}`}
      accessibilityLabel={props.chapter.label}
      class="min-h-14 flex-row items-center gap-3 rounded-(--radius-lg) border border-border bg-card px-4 py-3 active:bg-accent"
    >
      <View class="size-9 items-center justify-center rounded-full bg-muted">
        <Icon name="book" class="size-4 tint-muted-foreground" />
      </View>
      <View class="flex-1 gap-1">
        <Text
          class="text-sm font-semibold text-foreground"
          classList={{ "text-muted-foreground": props.chapter.read }}
          style={{ numberOfLines: 1 }}
        >
          {props.chapter.label}
        </Text>
        <If when={extra()}>
          <Text class="text-xs text-faint" style={{ numberOfLines: 1 }}>
            {name()}
          </Text>
        </If>
        <If when={inProgress(props.chapter)}>
          <View class="flex-row items-center gap-2">
            <ProgressBar value={props.chapter.total > 0 ? (props.chapter.page + 1) / props.chapter.total : 0} label="Progresso da leitura" class="flex-1" />
            <Text class="text-xs tabular-nums text-muted-foreground">{whereStopped(props.chapter)}</Text>
          </View>
        </If>
      </View>
      <If when={props.chapter.read}>
        <View class="flex-row items-center gap-1">
          <Icon name="check" class="size-3.5 tint-success" />
          <Text class="text-xs text-success">Lido</Text>
        </View>
      </If>
    </Button>
  );
}

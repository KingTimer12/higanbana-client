import { announce, effect, Text, View } from "nsh";
import { currentNotices } from "../store";

/** Avisos curtos sobre a tela (algo não está na nuvem, uma falha): somem sozinhos. */
export function Notices() {
  // Leitores de tela ouvem o aviso mais novo.
  effect(() => {
    const list = currentNotices();
    const newest = list[list.length - 1];
    if (newest) announce(newest.text);
  });
  return (
    <View
      class="absolute inset-x-0 bottom-0 items-center gap-2 px-4 pb-[calc(env(safe-area-inset-bottom)+16px)]"
      style={{ pointerEvents: "none" }}
    >
      <Each of={currentNotices()} key={(notice) => notice.id}>
        {(notice) => (
          <View class="max-w-md rounded-(--radius) border border-border bg-popover px-4 py-3 shadow-lg">
            <Text class="text-sm text-popover-foreground">{notice().text}</Text>
          </View>
        )}
      </Each>
    </View>
  );
}

// O aviso de versão nova no início: o que chegou, quanto pesa e um botão que baixa
// e abre o instalador do sistema. Some quando fechado (o menu da conta ainda oferece).
import { Text, View } from "nsh";
import { currentUpdate, dismissUpdate, installUpdate, showUpdateBanner } from "../store";
import { Icon, IconButton, PrimaryButton, ProgressBar } from "./ui";

const megabytes = (bytes: number) => `${Math.max(1, Math.round(bytes / (1024 * 1024)))} MB`;

export function UpdateBanner() {
  const state = () => currentUpdate();
  const update = () => {
    const s = state();
    return s.status === "available" || s.status === "downloading" || s.status === "ready" ? s.update : null;
  };
  const progress = () => {
    const s = state();
    return s.status === "downloading" ? s.progress : 0;
  };
  const detail = () => {
    const s = state();
    if (s.status === "downloading") return `Baixando… ${Math.round(s.progress * 100)}%`;
    if (s.status === "ready") return "Pronta para instalar.";
    return `${megabytes(update()?.size ?? 0)} · O Android pede para confirmar a instalação.`;
  };

  return (
    <If when={showUpdateBanner() ? update() : null}>
      {(u) => (
        <View class="mx-4 gap-3 rounded-(--radius-lg) border border-border bg-card p-4" testID="update-banner">
          <View class="flex-row items-start gap-3">
            <View class="size-10 items-center justify-center rounded-full bg-primary/15">
              <Icon name="download" class="size-5 tint-primary" />
            </View>
            <View class="flex-1 gap-0.5">
              <Text class="text-base font-semibold text-foreground">Versão {u().version} disponível</Text>
              <Text class="text-sm text-muted-foreground">{detail()}</Text>
            </View>
            <IconButton icon="close" label="Agora não" onPress={dismissUpdate} class="-me-2 -mt-2" iconClass="size-5 tint-muted-foreground" />
          </View>
          <If
            when={state().status === "downloading"}
            else={<PrimaryButton label={state().status === "ready" ? "Instalar" : "Atualizar"} icon="download" onPress={installUpdate} />}
          >
            <ProgressBar value={progress()} label="Baixando a atualização" />
          </If>
        </View>
      )}
    </If>
  );
}

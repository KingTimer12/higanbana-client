import { Text, View } from "nsh";
import { GhostButton, OutlineButton, PrimaryButton } from "../components/ui";
import { cancelSignIn, isLoginPending, loginFailure, reopenLogin, signIn } from "../store";

/** Entrada do app para quem não tem sessão: login pelo Discord no navegador. */
export function Login() {
  return (
    <View class="flex-1 justify-center gap-8 bg-background px-6 pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)]">
      <View class="gap-2">
        <Text class="text-sm font-medium tracking-widest text-brand-ink">彼岸花</Text>
        <Text role="header" class="text-4xl font-bold text-foreground">
          Higanbana
        </Text>
        <Text class="text-base text-muted-foreground">Entre com a conta do Discord que você usa no Anitsu.</Text>
      </View>

      <If
        when={isLoginPending()}
        else={
          <View class="gap-3">
            <PrimaryButton label="Entrar com Discord" onPress={signIn} />
            <If when={loginFailure()}>
              {(failure) => <Text class="text-sm text-destructive">Não foi possível entrar. {failure()}</Text>}
            </If>
          </View>
        }
      >
        <View class="gap-3">
          <Text class="text-base text-foreground">Conclua o login no navegador e volte para o app.</Text>
          <OutlineButton label="Abrir a página de novo" onPress={reopenLogin} />
          <GhostButton label="Cancelar" onPress={cancelSignIn} />
        </View>
      </If>
    </View>
  );
}

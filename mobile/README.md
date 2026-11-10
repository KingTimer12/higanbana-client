# Higanbana mobile

Porte mobile do higanbana em [Native Shell](https://nsh.dev/docs/) (NSH): telas em TSX sobre o
`higanbana-core`, exposto por um crate Rust do próprio app. Por enquanto só animes: login, início,
catálogo, busca, página da obra e player.

```text
App.tsx            entrada: núcleo de verdade (lib/native.ts) + Root
Fixture*.tsx       entradas com o acervo de mentira (fixture/fake.ts), para testes e prévia
Root.tsx           casca: login ou a pilha de telas, e os avisos
store.ts           sessão, navegação, avisos, progresso visto na sessão
pages/             uma tela por arquivo
components/        controles do app (ui.tsx), pôster e card de episódio do desktop, o vídeo
lib/               contrato com o núcleo (backend.ts), formatação
src/               crate Rust: `higanbana/native` (Account, Catalog, Library, Player)
theme.json         tokens de cor e raios (os do tema escuro do desktop)
android/           host Android com libmpv e o script que monta o APK
```

## Dia a dia

Precisa da chave do Supabase (`ANITSU_SUPABASE_ANON_KEY` ou `.env` na raiz do repositório), como o desktop.

De qualquer pasta do repositório, `scripts/mobile.sh` faz o mesmo sem entrar em `mobile/`:

```sh
scripts/mobile.sh verify          # check + build + test
scripts/mobile.sh run --fake      # prévia no desktop com o acervo de mentira
scripts/mobile.sh install         # APK de release no aparelho
scripts/mobile.sh dev             # hot reload no aparelho (servidor do nsh + APK --dev)
scripts/mobile.sh log             # logcat só do app
```

Ou, dentro de `mobile/`:

```sh
nsh check                 # cargo check + tsc
nsh build                 # avisos do bundler (classes desconhecidas etc.)
nsh test                  # testes das telas, headless, sobre o acervo de mentira
nsh run Fixture.tsx       # o app no desktop com dados de mentira
nsh run                   # o app no desktop com o núcleo de verdade
```

No desktop o vídeo usa o `Video` do NSH (o que o sistema toca, sem faixas); serve para desenvolver as telas.

## Android

O player é o libmpv, um componente nativo (`higanbana.mpv`) que só um host Android próprio consegue
registrar; o `nsh package --target android` não leva Java nem bibliotecas extras. Por isso o APK é
montado por `android/build.sh`, com as ferramentas do SDK (sem Gradle):

```sh
android/build.sh --install            # release: .nsh/out/higanbana.apk, instala e abre
android/build.sh --dev --install      # telas vindas de `nsh dev --server-only` (hot reload)
ENTRY=Fixture.tsx android/build.sh    # APK com o acervo de mentira
```

Requisitos: SDK do Android (build-tools, uma plataforma, NDK), JDK e
`rustup target add aarch64-linux-android`. O libmpv e o runtime do Kotlin vêm do Maven Central na
primeira vez.

Em emulador cujo decodificador de hardware não responde, abra com decodificação por software:

```sh
adb shell am start -n dev.kingtimer12.higanbana/.MainActivity --es hwdec no
```

## O que falta

- iOS: o `Video` do NSH (AVPlayer) não abre mkv; precisa do mesmo componente com libmpv/MPVKit.
- Mangás, música e downloads ficaram fora desta etapa. Os episódios já trazem título e miniatura do TMDB (chaves `TMDB_TOKEN`/`TMDB_KEY` no `.env`, como no desktop).
- Depois do login no navegador é preciso voltar ao app à mão.

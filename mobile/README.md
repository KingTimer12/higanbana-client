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
android/           projeto Gradle do host Android: Java (NSH + libmpv), C++ (NDK) e o libnsh.so do Rust
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
registrar; o `nsh package --target android` não leva Java nem bibliotecas extras. Por isso o APK sai
de um projeto Gradle em `android/`, que junta três linguagens:

- **Rust**: `libnsh.so`, o runtime do NSH ligado ao crate do app (`.nsh/rust`), compilado pelo cargo
  com o clang do NDK (tarefa `cargoBuild<Flavor>`).
- **C++** (`app/src/main/cpp`, CMake): sonda os decodificadores do aparelho (NDK MediaCodec) e escolhe
  o `hwdec` do mpv: hardware (`mediacodec-copy`) quando há, software (`no`) no emulador.
- **Java** (`app/src/main/java`): a atividade, a `NshView` e o componente do player sobre o libmpv
  (`dev.jdtech.mpv:libmpv`, do Maven Central).

Dois flavors: `bundled` leva as telas no APK (`nsh build`); `devServer` as busca no
`nsh dev --server-only` (hot reload, via `adb reverse`).

```sh
cd android
./gradlew :app:runBundledRelease                              # monta, instala e abre
./gradlew :app:runDevServerRelease                            # telas do `nsh dev --server-only`
./gradlew :app:runBundledRelease -Phiganbana.entry=Fixture.tsx   # acervo de mentira
./gradlew :app:assembleBundledRelease -Phiganbana.abis=arm64-v8a,x86_64
```

Para medir desempenho, use sempre um `*Release`: os `*Debug` são `debuggable`, e o ART roda a
interface (as views do NSH são Java) bem mais devagar. No Android Studio, troque a *Build Variant*
para `bundledRelease`. Tocando um vídeo, o logcat (`scripts/mobile.sh log`) mostra a cada 10 s o
decodificador em uso e os quadros perdidos (`higanbana.mpv`).

O APK fica em `android/app/build/outputs/apk/<flavor>/<tipo>/`. Opções em `android/gradle.properties`
(`higanbana.*`): entrada TSX, ABIs, porta do `nsh dev`, caminhos do `nsh`/`cargo` e a chave de
assinatura do release (sem ela, a de debug).

Requisitos: SDK do Android com o NDK 28.2 e o CMake 3.22.1, JDK, `nsh`, e
`rustup target add aarch64-linux-android` (mais um alvo por ABI extra). O Android Studio abre `android/`.

Para forçar o decodificador (ex.: software num aparelho cujo hardware falha):

```sh
./gradlew :app:runBundledRelease -Phiganbana.hwdec=no
adb shell am start -n dev.kingtimer12.higanbana/.MainActivity --es hwdec no
```

## O que falta

- iOS: o `Video` do NSH (AVPlayer) não abre mkv; precisa do mesmo componente com libmpv/MPVKit.
- Mangás, música e downloads ficaram fora desta etapa. Os episódios já trazem título e miniatura do TMDB (chaves `TMDB_TOKEN`/`TMDB_KEY` no `.env`, como no desktop).
- Depois do login no navegador é preciso voltar ao app à mão.

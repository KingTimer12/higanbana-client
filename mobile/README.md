# Higanbana mobile

Porte mobile do higanbana em [Native Shell](https://nsh.dev/docs/) (NSH): telas em TSX sobre o
`higanbana-core`, exposto por um crate Rust do próprio app: login, início, catálogo e busca de
animes e mangás (a mesma tela para as duas seções, como no desktop), franquia com abas, página da
obra, player e leitor de mangá.

```text
App.tsx            entrada: núcleo de verdade (lib/native.ts) + Root
Fixture*.tsx       entradas com o acervo de mentira (fixture/fake.ts), para testes e prévia
Root.tsx           casca: login ou a pilha de telas, e os avisos
store.ts           sessão, navegação, avisos, progresso visto na sessão
pages/             uma tela por arquivo
components/        controles do app (ui.tsx), pôster e card de episódio do desktop, o vídeo
lib/               contrato com o núcleo (backend.ts), seções do acervo (sections.ts), formatação
src/               crate Rust: `higanbana/native` (Account, Catalog, Library, Player, Reader)
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

Por padrão o APK leva `arm64-v8a` e `armeabi-v7a`: muitos celulares com Android 9 rodam o sistema em
32 bits (`adb shell getprop ro.product.cpu.abilist` mostra só `armeabi-v7a,armeabi`) e recusam um APK
só de 64 bits com “O app não foi instalado”.

Requisitos: SDK do Android com o NDK 28.2 e o CMake 3.22.1, JDK, `nsh`, e
`rustup target add aarch64-linux-android armv7-linux-androideabi` (mais um alvo por ABI extra). O Android Studio abre `android/`.

Para forçar o decodificador (ex.: software num aparelho cujo hardware falha):

```sh
./gradlew :app:runBundledRelease -Phiganbana.hwdec=no
adb shell am start -n dev.kingtimer12.higanbana/.MainActivity --es hwdec no
```

## Versão e atualização

A versão é o arquivo `VERSION` da raiz, a mesma do desktop: o Gradle tira dela o `versionName` e o
`versionCode` (`1.2.3` vira 1002003), e o app a mostra no menu da conta e na tela de login
(`Updates.version()`).

Ao abrir, o app procura no GitHub Releases o `Higanbana-<versão>-android.apk` mais novo
(`src/updates.rs`, sobre `higanbana_core::update`). Se há, o início mostra o aviso; “Atualizar” baixa o
APK para o cache, confere o SHA-256 contra o `SHA256SUMS` da release e chama
`openUrl("higanbana://update/install?file=...")`, que o host (`update/UpdateInstaller.java`) troca pelo
instalador de pacotes do Android via `FileProvider`. A pessoa confirma a instalação (e, na primeira vez,
permite instalar apps do Higanbana).

O Android só instala por cima um APK assinado com a mesma chave: as releases saem do CI
(`.github/workflows/release.yml`, job `android`) com a keystore dos secrets `ANDROID_KEYSTORE_*`. Um APK
instalado com a chave de debug precisa ser desinstalado uma vez antes da primeira versão do CI.

## Baixados (sem internet)

O botão ao lado de cada episódio e capítulo baixa o arquivo para a pasta de dados do app (`offline/`, no núcleo: `higanbana_core::offline`). O player toca a cópia (`file://`) e o leitor lê as faixas do `.cbz` dela, então os dois abrem sem rede; a tela "Baixados" (ícone no início) lista por obra, abre, apaga e tenta de novo o que falhou. Sem rede, a sessão guardada continua valendo para chegar até lá.

Limites: dois downloads por vez; o download só anda com o app aberto (fechar o app interrompe, e o arquivo volta como "falhou" para tentar de novo).

## O que falta

- iOS: o `Video` do NSH (AVPlayer) não abre mkv; precisa do mesmo componente com libmpv/MPVKit.
- Música e doramas ficaram fora desta etapa. Os episódios já trazem título e miniatura do TMDB (chaves `TMDB_TOKEN`/`TMDB_KEY` no `.env`, como no desktop).
- Leitor: só a leitura vertical contínua (sem página dupla nem direita para esquerda). Abre `.cbz`/`.zip` e `.rar` guardado sem compressão; o `.rar` compactado precisa da `unrar` (C++, feature `rar` do núcleo), que fica fora do celular. Páginas em AVIF só aparecem onde o sistema decodifica AVIF (Android 12 ou mais novo).
- Depois do login no navegador é preciso voltar ao app à mão.
- iOS não se atualiza pelo app (lá a distribuição é pela loja ou TestFlight).

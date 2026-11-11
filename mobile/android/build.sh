#!/usr/bin/env bash
# Monta o APK Android do higanbana: o runtime do NSH com o crate Rust do app
# (libnsh.so), as telas TSX e o player libmpv (host em android/src).
#
# O `nsh package --target android` não leva código Java nem bibliotecas além
# das do NSH, e o libmpv precisa dos dois; por isso este script empacota à mão,
# com as mesmas ferramentas do SDK (sem Gradle).
#
#   android/build.sh                 APK de release em .nsh/out/higanbana.apk
#   android/build.sh --dev           APK que carrega as telas do `nsh dev --server-only`
#   android/build.sh --install       instala e abre no aparelho que o adb vê
#   ENTRY=Fixture.tsx android/build.sh   empacota outra entrada (o acervo de mentira)
#   ABIS="arm64-v8a x86_64" android/build.sh
#
# Precisa de: nsh, cargo (+ `rustup target add aarch64-linux-android ...`), um JDK,
# o SDK do Android (ANDROID_HOME) com build-tools, uma plataforma e o NDK, e a
# chave do Supabase (ANITSU_SUPABASE_ANON_KEY ou `.env` na raiz do repositório).
set -euo pipefail

MPV_VERSION=1.0.0
KOTLIN_VERSION=2.2.10
MIN_SDK=28
TARGET_SDK=35

DEV=0
INSTALL=0
for arg in "$@"; do
  case "$arg" in
    --dev) DEV=1 ;;
    --install) INSTALL=1 ;;
    *) echo "opção desconhecida: $arg" >&2; exit 2 ;;
  esac
done

HERE="$(cd "$(dirname "$0")" && pwd)"
APP="$(dirname "$HERE")"
ENTRY="${ENTRY:-App.tsx}"
ABIS="${ABIS:-arm64-v8a}"
WORK="$APP/.nsh/android"
SUFFIX=""
FEATURES=()
if [ "$DEV" = 1 ]; then
  SUFFIX="-dev"
  FEATURES=(--features dev)
fi
OUT="$APP/.nsh/out/higanbana$SUFFIX.apk"
DEPS="$WORK/deps"

fail() { echo "erro: $*" >&2; exit 1; }
step() { echo "→ $*"; }
latest() { ls -d "$1"/* 2>/dev/null | sort -V | tail -1; }

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
[ -d "$SDK" ] || fail "SDK do Android não encontrado (defina ANDROID_HOME)"
BUILD_TOOLS="$(latest "$SDK/build-tools")"
[ -n "$BUILD_TOOLS" ] || fail "build-tools ausentes em $SDK"
ANDROID_JAR="$(latest "$SDK/platforms")/android.jar"
[ -f "$ANDROID_JAR" ] || fail "nenhuma plataforma (android.jar) em $SDK/platforms"
NDK="${ANDROID_NDK_HOME:-$(latest "$SDK/ndk")}"
[ -d "$NDK" ] || fail "NDK não encontrado (sdkmanager \"ndk;<versão>\")"
TOOLCHAIN="$(latest "$NDK/toolchains/llvm/prebuilt")/bin"
NSH_HOME="$(cd "$(dirname "$(readlink -f "$(command -v nsh)")")/.." && pwd)"
NSH_AAR="$NSH_HOME/embed/android/nsh$SUFFIX.aar"
[ -f "$NSH_AAR" ] || fail "runtime Android do NSH não encontrado em $NSH_AAR"

rust_target() {
  case "$1" in
    arm64-v8a) echo aarch64-linux-android ;;
    armeabi-v7a) echo armv7-linux-androideabi ;;
    x86_64) echo x86_64-linux-android ;;
    *) fail "ABI desconhecida: $1" ;;
  esac
}
clang_of() {
  case "$1" in
    armv7-linux-androideabi) echo "$TOOLCHAIN/armv7a-linux-androideabi$MIN_SDK-clang" ;;
    *) echo "$TOOLCHAIN/$1$MIN_SDK-clang" ;;
  esac
}

rm -rf "$WORK/apk" "$WORK/classes" "$WORK/dex"
mkdir -p "$DEPS" "$WORK/apk" "$WORK/classes" "$WORK/dex" "$(dirname "$OUT")"

# --- Dependências: libmpv (AAR do Maven Central) e o runtime do Kotlin que ele usa ---
fetch() { [ -f "$2" ] || { step "baixando $(basename "$2")"; curl -fsSL "$1" -o "$2.part" && mv "$2.part" "$2"; }; }
fetch "https://repo1.maven.org/maven2/dev/jdtech/mpv/libmpv/$MPV_VERSION/libmpv-$MPV_VERSION.aar" "$DEPS/libmpv-$MPV_VERSION.aar"
fetch "https://repo1.maven.org/maven2/org/jetbrains/kotlin/kotlin-stdlib/$KOTLIN_VERSION/kotlin-stdlib-$KOTLIN_VERSION.jar" "$DEPS/kotlin-stdlib-$KOTLIN_VERSION.jar"
[ -d "$DEPS/libmpv" ] || unzip -q -o "$DEPS/libmpv-$MPV_VERSION.aar" -d "$DEPS/libmpv"
rm -rf "$DEPS/nsh" && unzip -q -o "$NSH_AAR" classes.jar -d "$DEPS/nsh"

# --- Telas: o bundle TSX (só no release; em --dev vêm do servidor) ---
cd "$APP"
if [ "$DEV" = 1 ]; then
  step "gerando os fontes do runtime (nsh check)"
  nsh check >/dev/null
else
  step "empacotando $ENTRY"
  rm -rf "$WORK/bundle"
  nsh build "$ENTRY" --release --out "$WORK/bundle"
  mkdir -p "$WORK/apk/assets/nsh"
  cp "$WORK/bundle/bundle.nshc" "$WORK/apk/assets/nsh/"
  for extra in assets capabilities.json app.json; do
    if [ -e "$WORK/bundle/$extra" ]; then cp -R "$WORK/bundle/$extra" "$WORK/apk/assets/nsh/"; fi
  done
fi

# --- libnsh.so: o runtime do NSH ligado ao crate do app, por ABI ---
RUNNER="$APP/.nsh/rust/Cargo.toml"
[ -f "$RUNNER" ] || fail "fontes do runtime ausentes em .nsh/rust (rode um comando do nsh antes)"
for abi in $ABIS; do
  target="$(rust_target "$abi")"
  cc="$(clang_of "$target")"
  [ -x "$cc" ] || fail "compilador do NDK ausente: $cc"
  env_name="$(echo "$target" | tr 'a-z-' 'A-Z_')"
  step "compilando libnsh.so para $abi"
  # O bindgen (QuickJS) precisa dos cabeçalhos do NDK, não dos do sistema.
  env "CC_${target//-/_}=$cc" "CXX_${target//-/_}=$cc++" "AR_${target//-/_}=$TOOLCHAIN/llvm-ar" \
    "CARGO_TARGET_${env_name}_LINKER=$cc" "CLANG_PATH=$cc" \
    "BINDGEN_EXTRA_CLANG_ARGS_${target//-/_}=--sysroot=$(dirname "$TOOLCHAIN")/sysroot" \
    cargo build --manifest-path "$RUNNER" --target-dir "$APP/target" \
      -p nsh-app-higanbana-mobile-android --target "$target" --release ${FEATURES[@]+"${FEATURES[@]}"}
  mkdir -p "$WORK/apk/lib/$abi"
  "$TOOLCHAIN/llvm-strip" --strip-unneeded \
    -o "$WORK/apk/lib/$abi/libnsh.so" "$APP/target/$target/release/libnsh_app_higanbana_mobile_android.so"
  cp "$DEPS/libmpv/jni/$abi/"*.so "$WORK/apk/lib/$abi/"
done

# --- Java: o host (android/src) contra o runtime do NSH e o libmpv ---
step "compilando o host Android"
CLASSPATH="$ANDROID_JAR:$DEPS/nsh/classes.jar:$DEPS/libmpv/classes.jar"
find "$HERE/src" -name '*.java' > "$WORK/sources.txt"
javac -nowarn --release 11 -cp "$CLASSPATH" -d "$WORK/classes" @"$WORK/sources.txt"
find "$WORK/classes" -name '*.class' > "$WORK/classes.txt"
"$BUILD_TOOLS/d8" --release --min-api "$MIN_SDK" --lib "$ANDROID_JAR" --output "$WORK/dex" \
  @"$WORK/classes.txt" "$DEPS/nsh/classes.jar" "$DEPS/libmpv/classes.jar" "$DEPS/kotlin-stdlib-$KOTLIN_VERSION.jar"

# --- APK ---
step "montando o APK"
VERSION="$(sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' "$APP/nsh.json")"
BUILD="$(sed -n 's/.*"build": *\([0-9]*\).*/\1/p' "$APP/nsh.json")"
"$BUILD_TOOLS/aapt2" link -o "$WORK/unsigned.apk" --manifest "$HERE/AndroidManifest.xml" -I "$ANDROID_JAR" \
  --min-sdk-version "$MIN_SDK" --target-sdk-version "$TARGET_SDK" \
  --version-name "${VERSION:-0.1.0}" --version-code "${BUILD:-1}"
cp "$WORK"/dex/*.dex "$WORK/apk/"
(cd "$WORK/apk" && zip -q -r "$WORK/unsigned.apk" . -x '*.DS_Store')
"$BUILD_TOOLS/zipalign" -f -P 16 4 "$WORK/unsigned.apk" "$WORK/aligned.apk"

KEYSTORE="${KEYSTORE:-$HOME/.android/debug.keystore}"
if [ ! -f "$KEYSTORE" ]; then
  mkdir -p "$(dirname "$KEYSTORE")"
  keytool -genkeypair -keystore "$KEYSTORE" -storepass android -keypass android -alias androiddebugkey \
    -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=Android Debug,O=Android,C=US" >/dev/null
fi
"$BUILD_TOOLS/apksigner" sign --ks "$KEYSTORE" --ks-pass "pass:${KEYSTORE_PASSWORD:-android}" --out "$OUT" "$WORK/aligned.apk"
echo "✓ $OUT ($(du -h "$OUT" | cut -f1))"

if [ "$INSTALL" = 1 ]; then
  ADB="$SDK/platform-tools/adb"
  step "instalando"
  "$ADB" install -r "$OUT"
  if [ "$DEV" = 1 ]; then "$ADB" reverse tcp:8790 tcp:8790; fi
  "$ADB" shell am start -n dev.kingtimer12.higanbana/.MainActivity
fi

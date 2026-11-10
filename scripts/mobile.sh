#!/usr/bin/env bash
# Atalhos do app mobile (mobile/) a partir de qualquer pasta do repositório.
#
#   scripts/mobile.sh check            nsh check (cargo check + tsc)
#   scripts/mobile.sh build            nsh build (avisos do bundler: classes desconhecidas etc.)
#   scripts/mobile.sh test             testes das telas, headless, sobre o acervo de mentira
#   scripts/mobile.sh verify           check + build + test, nessa ordem
#   scripts/mobile.sh run [--fake]     o app no desktop (com --fake: acervo de mentira)
#   scripts/mobile.sh apk [--fake]     APK de release em mobile/.nsh/out/
#   scripts/mobile.sh install [--fake] monta, instala e abre no aparelho
#   scripts/mobile.sh dev              hot reload no aparelho: servidor do nsh + APK --dev
#   scripts/mobile.sh open [--sw]      abre o app no aparelho (--sw: decodificação por software)
#   scripts/mobile.sh log              logcat só do app
#   scripts/mobile.sh devices          aparelhos que o adb vê
#
# Variáveis repassadas ao android/build.sh: ABIS, KEYSTORE, ANDROID_HOME, ANDROID_NDK_HOME.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SELF="$ROOT/scripts/$(basename "$0")"
MOBILE="$ROOT/mobile"
PACKAGE="dev.kingtimer12.higanbana"
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
ADB="$SDK/platform-tools/adb"
[ -x "$ADB" ] || ADB="$(command -v adb || true)"

fail() { echo "erro: $*" >&2; exit 1; }
need_adb() { [ -n "$ADB" ] && [ -x "$ADB" ] || fail "adb não encontrado (instale platform-tools ou defina ANDROID_HOME)"; }
usage() { sed -n '2,16p' "$SELF" | sed 's/^# \{0,1\}//'; }

# `--fake` em qualquer posição troca a entrada pelo acervo de mentira.
FAKE=0
ARGS=()
for arg in "${@:2}"; do
  case "$arg" in
    --fake) FAKE=1 ;;
    *) ARGS+=("$arg") ;;
  esac
done
entry() { if [ "$FAKE" = 1 ]; then echo Fixture.tsx; else echo App.tsx; fi; }

cd "$MOBILE"
case "${1:-}" in
  check) nsh check ;;
  build) nsh build ;;
  test) nsh test ${ARGS[@]+"${ARGS[@]}"} ;;
  verify) nsh check && nsh build && nsh test ;;
  run) nsh run "$(entry)" ${ARGS[@]+"${ARGS[@]}"} ;;
  apk) ENTRY="$(entry)" android/build.sh ;;
  install) ENTRY="$(entry)" android/build.sh --install ;;
  dev)
    need_adb
    # O servidor serve as telas; o APK --dev as carrega por `adb reverse`.
    nsh dev --server-only &
    SERVER=$!
    trap 'kill "$SERVER" 2>/dev/null || true' EXIT INT TERM
    android/build.sh --dev --install
    echo "→ hot reload ativo; Ctrl-C encerra o servidor"
    wait "$SERVER"
    ;;
  open)
    need_adb
    if [ "${ARGS[0]:-}" = "--sw" ]; then
      "$ADB" shell am start -n "$PACKAGE/.MainActivity" --es hwdec no
    else
      "$ADB" shell am start -n "$PACKAGE/.MainActivity"
    fi
    ;;
  log)
    need_adb
    pid="$("$ADB" shell pidof -s "$PACKAGE" | tr -d '\r' || true)"
    [ -n "$pid" ] || fail "o app não está aberto no aparelho (scripts/mobile.sh open)"
    "$ADB" logcat --pid="$pid"
    ;;
  devices) need_adb; "$ADB" devices -l ;;
  ""|-h|--help|help) usage ;;
  *) usage; fail "comando desconhecido: $1" ;;
esac

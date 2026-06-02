#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANDROID_PROJECT_DIR="$ROOT_DIR/sdk/mobile/android"

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$ANDROID_HOME}"

find_gradle() {
  if [ -x "$ANDROID_PROJECT_DIR/gradlew" ]; then
    printf '%s\n' "$ANDROID_PROJECT_DIR/gradlew"
    return 0
  fi
  if command -v gradle >/dev/null 2>&1; then
    command -v gradle
    return 0
  fi

  local candidate
  for candidate in \
    "$HOME/.gradle/wrapper/dists/gradle-8.14-bin"/*/gradle-8.14/bin/gradle \
    "$HOME/.gradle/wrapper/dists/gradle-8.13-bin"/*/gradle-8.13/bin/gradle \
    "$HOME/.gradle/wrapper/dists/gradle-8.4-all"/*/gradle-8.4/bin/gradle; do
    if [ -x "$candidate" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  return 1
}

GRADLE_BIN="$(find_gradle)" || {
  echo "Gradle was not found. Install Gradle or generate a wrapper in sdk/mobile/android." >&2
  exit 1
}

if [ "$(basename "$GRADLE_BIN")" = "gradlew" ]; then
  (
    cd "$ANDROID_PROJECT_DIR"
    "$GRADLE_BIN" \
      :agentgrid-mobile-sdk-kotlin:assembleDebug \
      :agentgrid-mobile-sdk-kotlin:testDebugUnitTest
  )
else
  "$GRADLE_BIN" \
    -p "$ANDROID_PROJECT_DIR" \
    :agentgrid-mobile-sdk-kotlin:assembleDebug \
    :agentgrid-mobile-sdk-kotlin:testDebugUnitTest
fi

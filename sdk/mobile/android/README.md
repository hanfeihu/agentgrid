# AgentGrid Android Mobile SDK

This directory is a standalone Gradle project for the Android Mobile SDK.

## Module

```text
:agentgrid-mobile-sdk-kotlin
```

The module is an Android library, not a Worker runtime. It lets an Android
console app call Hub APIs, including Codex Bridge and Node Port Bridge control
plane APIs.

## Build

From the repository root:

```bash
scripts/check-android-mobile-sdk.sh
```

Or from this directory with a Gradle installation:

```bash
./gradlew :agentgrid-mobile-sdk-kotlin:assembleDebug
./gradlew :agentgrid-mobile-sdk-kotlin:testDebugUnitTest
```

If Android SDK environment variables are not configured, set one of:

```bash
export ANDROID_HOME="$HOME/Library/Android/sdk"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
```

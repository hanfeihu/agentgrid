# One-click setup (agentgrid.tminos.com)

What `https://agentgrid.tminos.com/setup` serves. The page detects Windows / macOS / Linux / phone and
shows one download or one command. The Hub URL `https://agentgrid.tminos.com` is hard-coded in these
files; change it in every file before reusing them for another Hub.

| File | Published at (Hub web dir `downloads/`) | Notes |
|---|---|---|
| `page/setup.html` | `/setup` (nginx → `/downloads/setup.html`) | Chinese / English by browser language |
| `windows/AgentGrid-Setup.cmd` | `/downloads/AgentGrid-Setup.cmd` | ASCII, CRLF. Elevates itself, asks a name, runs the `.ps1` |
| `windows/AgentGrid-Setup.ps1` | `/downloads/AgentGrid-Setup.ps1` | UTF-8 with BOM, CRLF. Edit this, not the `.cmd`. Keeps an existing node id / name / desktop helper |
| `mac/` → `AgentGrid-Setup-mac.zip` | `/downloads/AgentGrid-Setup-mac.zip` | Build below |
| `linux/setup.sh` | `/setup.sh` (nginx → `/downloads/setup.sh`) | `curl -fsSL https://agentgrid.tminos.com/setup.sh \| sudo sh` |

nginx must serve `.cmd` as `application/octet-stream` with `Content-Disposition: attachment`
(the Hub serves it as `text/plain`, and browsers would display it).

## Design notes

- Every installer keeps the node id and name of an existing worker, so re-running reconnects the same machine.
- "Done" is reported only after a heartbeat newer than the restart: the old process's last heartbeat keeps
  the node "online" for a while after it stops.
- Linux: the worker runs as user `agentgrid` and owns `/opt/agentgrid-worker`, so it can replace itself on update.
- macOS: launchd runs `/bin/sh -c 'while :; do "$0" "$@"; sleep 2; done' <worker> …`. launchd pins the code
  identity of what it spawns (`/bin/sh` here), so an auto-updated worker is not refused with
  `OS_REASON_CODESIGNING`; and the shell loop restarts the worker after it exits, which launchd did not do
  for a clean exit (`pended nondemand spawn = inefficient`).

## Build the macOS app

```bash
osacompile -o "AgentGrid Setup.app" mac/setup.applescript
cp mac/install.sh "AgentGrid Setup.app/Contents/Resources/install.sh"
/usr/libexec/PlistBuddy -c "Add :CFBundleIdentifier string com.tminos.agentgrid.setup" \
  -c "Add :CFBundleShortVersionString string 1.0" -c "Add :LSMinimumSystemVersion string 11.0" \
  "AgentGrid Setup.app/Contents/Info.plist"
codesign --force --options runtime --timestamp --sign "Developer ID Application: Fei Hu Han (T4NH3G33H3)" "AgentGrid Setup.app"
ditto -c -k --keepParent "AgentGrid Setup.app" submit.zip
xcrun notarytool submit submit.zip --keychain-profile tminos-notary --wait
xcrun stapler staple "AgentGrid Setup.app"
ditto -c -k --keepParent "AgentGrid Setup.app" AgentGrid-Setup-mac.zip
```

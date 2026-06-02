# Release Process

AgentGrid releases are built by GitHub Actions when a version tag is pushed.

## Versioning

Use semantic version tags:

```text
v0.1.0-alpha.0
v0.1.0-alpha.1
v0.1.0-beta.0
v0.1.0
```

Tags containing `alpha`, `beta`, or `rc` are published as GitHub prereleases.

## Publish A Release

```bash
git checkout main
git pull
git tag v0.1.0-alpha.0
git push origin main
git push origin v0.1.0-alpha.0
git push github main
git push github v0.1.0-alpha.0
```

The Release workflow builds:

- `agentgrid`
- `agentgrid-hub`
- `agentgrid-worker`
- `agentgrid-mcp`
- web console assets
- docs, examples, scripts

## Release Assets

The alpha workflow uploads:

```text
agentgrid-<version>-linux-x86_64.tar.gz
agentgrid-<version>-macos-arm64.tar.gz
agentgrid-<version>-windows-x86_64.zip
*.sha256
*.ed25519.sig
```

Worker auto-update packages should be signed with Ed25519 before production rollout. `scripts/publish-worker-updates.sh` writes `agentgrid-worker(.exe).sha256` and, when `AGENTGRID_WORKER_UPDATE_PRIVATE_KEY_FILE` is set, `agentgrid-worker(.exe).ed25519.sig`.

Hub exposes signature metadata through `/api/worker/update-manifest`. Workers verify signatures when `--update-public-key <base64-ed25519-public-key>` or `AGENTGRID_WORKER_UPDATE_PUBLIC_KEY` is configured. Use `--require-update-signature` or `AGENTGRID_REQUIRE_UPDATE_SIGNATURE=true` on Worker nodes to reject unsigned packages.

Hub can also enforce signed manifests. Set `AGENTGRID_WORKER_UPDATE_SIGNATURE_REQUIRED=true` and `AGENTGRID_WORKER_UPDATE_PUBLIC_KEY=<base64-ed25519-public-key>` on Hub; when enabled, Hub rejects update manifests if the signature file or public key is missing, or if signature verification fails.

macOS Intel builds are not published in the first alpha release because GitHub-hosted Intel macOS runners can have long queue times. Intel Mac users can build from source until a dedicated release job is added.

## Smoke Test

Before publishing, run the local end-to-end smoke test:

```bash
scripts/e2e-hub-worker-cli.sh
scripts/e2e-codex-bridge.sh
```

After a release is published:

```bash
curl -fsSL https://raw.githubusercontent.com/hanfeihu/agentgrid/main/scripts/install.sh | bash
agentgrid --help
agentgrid-hub --host 127.0.0.1 --port 20181 --db /opt/agentgrid/agentgrid-hub.db --web-dir /opt/agentgrid/web
```

Then start a Worker and submit a command task:

```bash
agentgrid-worker --hub http://127.0.0.1:20181 --id local-worker --name "Local Worker" --capability command --capability file --capability http
agentgrid submit-command --program hostname --wait
```

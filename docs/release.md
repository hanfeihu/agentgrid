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

The workflow uploads:

```text
agentgrid-<version>-linux-x86_64.tar.gz
agentgrid-<version>-macos-x86_64.tar.gz
agentgrid-<version>-macos-arm64.tar.gz
agentgrid-<version>-windows-x86_64.zip
*.sha256
```

## Smoke Test

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

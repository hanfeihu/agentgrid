# 发布流程

AgentGrid 使用 GitHub Actions 通过版本 tag 自动构建 Release。

## 版本规则

使用语义化版本：

```text
v0.1.0-alpha.0
v0.1.0-alpha.1
v0.1.0-beta.0
v0.1.0
```

包含 `alpha`、`beta`、`rc` 的 tag 会发布成 GitHub prerelease。

## 发布一个版本

```bash
git checkout main
git pull
git tag v0.1.0-alpha.0
git push origin main
git push origin v0.1.0-alpha.0
git push github main
git push github v0.1.0-alpha.0
```

Release workflow 会构建：

- `agentgrid`
- `agentgrid-hub`
- `agentgrid-worker`
- `agentgrid-mcp`
- Web 总控台静态资源
- 文档、示例、脚本

## 发布产物

Alpha workflow 会上传：

```text
agentgrid-<version>-linux-x86_64.tar.gz
agentgrid-<version>-macos-arm64.tar.gz
agentgrid-<version>-windows-x86_64.zip
*.sha256
*.ed25519.sig
```

生产环境的 Worker 自动更新包应该使用 Ed25519 签名。`scripts/publish-worker-updates.sh` 会写入 `agentgrid-worker(.exe).sha256`；如果设置了 `AGENTGRID_WORKER_UPDATE_PRIVATE_KEY_FILE`，还会生成 `agentgrid-worker(.exe).ed25519.sig`。

Hub 会通过 `/api/worker/update-manifest` 返回签名元数据。Worker 配置 `--update-public-key <base64-ed25519-public-key>` 或 `AGENTGRID_WORKER_UPDATE_PUBLIC_KEY` 后会验签。Worker 节点使用 `--require-update-signature` 或 `AGENTGRID_REQUIRE_UPDATE_SIGNATURE=true` 可以拒绝未签名更新包。

Hub 也可以强制签名清单。给 Hub 设置 `AGENTGRID_WORKER_UPDATE_SIGNATURE_REQUIRED=true` 和 `AGENTGRID_WORKER_UPDATE_PUBLIC_KEY=<base64-ed25519-public-key>` 后，如果签名文件缺失、公钥缺失或验签失败，Hub 会直接拒绝返回更新清单。

首个 alpha 暂不发布 macOS Intel 包，因为 GitHub 托管的 Intel macOS runner 排队时间可能很长。Intel Mac 用户可以先从源码构建，后续再补独立发布任务。

## 冒烟测试

发布前先跑本地端到端冒烟测试：

```bash
scripts/e2e-hub-worker-cli.sh
scripts/e2e-codex-bridge.sh
```

Release 发布后：

```bash
curl -fsSL https://raw.githubusercontent.com/hanfeihu/agentgrid/main/scripts/install.sh | bash
agentgrid --help
agentgrid-hub --host 127.0.0.1 --port 20181 --db /opt/agentgrid/agentgrid-hub.db --web-dir /opt/agentgrid/web
```

然后启动 Worker 并提交一个命令任务：

```bash
agentgrid-worker --hub http://127.0.0.1:20181 --id local-worker --name "Local Worker" --capability command --capability file --capability http
agentgrid submit-command --program hostname --wait
```

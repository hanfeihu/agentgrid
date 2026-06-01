# 产物和发布资产

AgentGrid 把执行证据当成一等数据。任务不应该只是显示“完成”，还应该说明发生了什么，并附上有用证据。

## 证据类型

常见证据：

- stdout 日志
- stderr 日志
- 截图
- 文件产物
- 目录列表
- 浏览器文本
- DOM 快照
- 下载文件
- 串口输出
- 测试报告
- 操作时间线
- 插件结果

## Artifact Store v2 目标

Artifact Store v2 是任务证据存储和预览标准。

它应该支持：

- content type
- byte size
- SHA-256 hash
- 小文件 base64 内联
- 大文件外部对象存储引用
- 保留策略
- task/job/node 关联
- 预览提示
- 下载地址
- 长日志切片
- 产物打包

## Web 总控台

总控台应该展示：

- 产物列表
- 关联任务
- 截图预览
- 文本/日志预览
- 下载按钮
- hash 和大小
- 创建时间

## Worker 更新包

Worker 自动更新使用 Hub 发布的二进制产物：

```text
web/downloads/<target>/agentgrid-worker
web/downloads/<target>/agentgrid-worker.sha256
```

Windows 使用：

```text
web/downloads/windows-x86_64/agentgrid-worker.exe
```

推荐 target：

- `linux-x86_64`
- `linux-x86_64-legacy`
- `darwin-aarch64`
- `darwin-x86_64`
- `windows-x86_64`

## Release 检查清单

发布 GitHub Release 前：

```bash
cargo build --release -p agentgrid-hub -p agentgrid-worker-app -p agentgrid-cli -p agentgrid-mcp
npm --prefix apps/agentgrid-web run build
node scripts/validate-agentgrid-schemas.js
```

打包：

- Hub 二进制
- 各平台 Worker 二进制
- CLI 二进制
- MCP 二进制
- web console `dist`
- checksums
- release notes

不要包含：

- 数据库文件
- 日志
- 私有服务器清单
- SMTP 密钥
- SSH 凭据
- 带私有信息的截图

# Worker Plugin Runtime v2 Standard

Worker Plugin Runtime v2 defines how an AgentGrid Worker exposes node-local
custom tools through stable, AI-readable contracts.

The runtime separates an installed implementation package from the tool contract
that AI clients call. A plugin may provide one or more tools, but every callable
tool must have its own schema, risk, timeout, evidence, and probe contract.

Schema: `schemas/plugin-manifest.schema.json`

## 1. Goals

- Let nodes add local capabilities without changing Worker core code.
- Give AI clients a precise manifest for payload generation and result parsing.
- Let Hub validate plugin identity, platform support, tool contracts, probe
  behavior, risk, and expected evidence.
- Keep scheduling trust separate from plugin self-declaration.
- Support human installation and troubleshooting with the same manifest AI uses.

## 2. Non-Goals

- Plugin Runtime does not grant authorization by itself.
- Plugin Runtime does not make unsafe local actions safe.
- Plugin Runtime does not require every node to install the same plugins.
- Plugin Runtime does not allow natural-language task payloads unless a tool
  explicitly declares a natural-language field in its JSON Schema.

## 3. Core Objects

- `plugin_id`: stable installed package id, such as
  `agentgrid-plugin-document-parser`.
- `tool_id`: stable AI-facing callable id, such as `document.parse`.
- `executor`: plugin executor reference, always `plugin:<plugin_id>`.
- `manifest`: package identity, platform support, tools, dependencies, probe,
  risk, and evidence requirements.
- `probe`: a real runtime call used to verify the plugin on a node.
- `plugin_request`: JSON sent by Worker to the plugin process.
- `plugin_result`: JSON returned by the plugin process.

## 4. Manifest Shape

Minimum manifest:

```json
{
  "api_version": "agentgrid.plugin-manifest/v2",
  "kind": "PluginManifest",
  "plugin_id": "agentgrid-plugin-document-parser",
  "name": "Document Parser",
  "version": "1.2.0",
  "description": "Parse local documents into structured text.",
  "publisher": {
    "name": "AgentGrid",
    "url": "https://agentgrid.local"
  },
  "runtime": {
    "entrypoint": "./bin/document-parser",
    "protocol": "stdio-json",
    "working_directory": "plugin",
    "timeout_seconds": 120
  },
  "platforms": [
    {
      "os": "linux",
      "arch": "x86_64"
    },
    {
      "os": "macos",
      "arch": "arm64"
    }
  ],
  "tools": [
    {
      "tool_id": "document.parse",
      "title": "Parse document",
      "description": "Extract text and metadata from a document artifact.",
      "capabilities": ["plugin", "document"],
      "input_schema": {
        "type": "object",
        "required": ["artifact_id"],
        "properties": {
          "artifact_id": { "type": "string" },
          "extract_mode": { "enum": ["text", "metadata", "full"] }
        },
        "additionalProperties": false
      },
      "output_schema": {
        "type": "object",
        "required": ["text"],
        "properties": {
          "text": { "type": "string" },
          "metadata": { "type": "object" }
        },
        "additionalProperties": true
      },
      "risk": "medium",
      "default_timeout_seconds": 120,
      "expected_evidence": ["structured_result", "file_artifact"]
    }
  ],
  "probe": {
    "tool_id": "document.parse",
    "input": {
      "artifact_id": "probe_document",
      "extract_mode": "metadata"
    },
    "success_condition": {
      "type": "json_path_equals",
      "path": "$.ok",
      "value": true
    },
    "interval_seconds": 3600
  },
  "risk": {
    "level": "medium",
    "summary": "Reads local document artifacts and may process sensitive text."
  }
}
```

## 5. Runtime Request

Worker invokes a plugin with a structured request over the declared protocol.

```json
{
  "api_version": "agentgrid.plugin-request/v2",
  "kind": "PluginRequest",
  "plugin_id": "agentgrid-plugin-document-parser",
  "tool_id": "document.parse",
  "action": "run",
  "task_id": "task_xxx",
  "job_id": "job_xxx",
  "node_id": "hub-node",
  "input": {
    "artifact_id": "artifact_xxx",
    "extract_mode": "text"
  },
  "context": {
    "timeout_seconds": 120,
    "work_dir": "/var/lib/agentgrid/work/task_xxx",
    "evidence_dir": "/var/lib/agentgrid/work/task_xxx/evidence"
  }
}
```

Rules:

- Worker must validate `input` against the tool `input_schema` before launch.
- Worker must pass only structured JSON to the plugin.
- Worker must enforce timeout and local policy even if the plugin omits checks.
- Worker should provide a task-scoped working directory.
- Worker should capture stdout, stderr, exit code, duration, and produced files
  as evidence.

## 6. Runtime Result

Plugin stdout must be a single JSON object:

```json
{
  "api_version": "agentgrid.plugin-result/v2",
  "kind": "PluginResult",
  "ok": true,
  "plugin_id": "agentgrid-plugin-document-parser",
  "tool_id": "document.parse",
  "output": {
    "text": "Parsed text",
    "metadata": {
      "page_count": 3
    }
  },
  "evidence": [
    {
      "type": "structured_result",
      "title": "Parsed document JSON"
    }
  ],
  "metrics": {
    "duration_ms": 1280
  }
}
```

Failure result:

```json
{
  "api_version": "agentgrid.plugin-result/v2",
  "kind": "PluginResult",
  "ok": false,
  "plugin_id": "agentgrid-plugin-document-parser",
  "tool_id": "document.parse",
  "error": {
    "code": "dependency_missing",
    "message": "pdfinfo was not found",
    "retryable": false
  }
}
```

Worker wraps the plugin result into the normal AgentGrid task result. The Hub
then normalizes evidence through Evidence Pipeline v2.

## 7. Error Codes

Standard plugin error codes:

- `plugin_not_found`
- `plugin_manifest_invalid`
- `plugin_platform_unsupported`
- `plugin_disabled`
- `dependency_missing`
- `input_schema_invalid`
- `plugin_timeout`
- `plugin_exit_nonzero`
- `invalid_plugin_output`
- `output_schema_invalid`
- `evidence_missing`
- `policy_denied`
- `probe_failed`

Plugins may add tool-specific codes, but shared scheduler and UI logic should
rely on the standard codes above.

## 8. Probe Contract

A manifest probe is a lightweight real call that proves the installed plugin
can run on a node.

Probe states:

- `declared_unverified`
- `pending`
- `verified`
- `failed`
- `expired`
- `unsupported`
- `disabled`

Scheduling rule:

```text
hard eligibility -> resource score -> probe trust -> plugin risk -> capability graph fit -> selected node
```

A plugin declaration makes a node eligible only when the requested `tool_id`,
platform, policy, and required capabilities match. Probe status affects trust
and ranking. A failed probe should block or strongly down-rank the tool,
depending on policy.

## 9. Security And Policy

Manifest risk is advisory input to policy; it is not a permission grant.

Risk levels:

- `low`: read-only or deterministic local processing.
- `medium`: may read sensitive local artifacts, use network, or allocate
  meaningful resources.
- `high`: can mutate local files, devices, services, credentials, firmware, or
  external systems.

Plugin manifests must declare:

- Whether the tool reads files.
- Whether the tool writes files.
- Whether the tool uses network access.
- Whether the tool may access devices.
- Whether the tool may execute child processes.
- Required environment variables or secrets by reference, not by value.

## 10. Dependency Contract

Dependencies are declared so humans and AI can explain installation failures.

Dependency types:

- `binary`
- `library`
- `python_package`
- `node_package`
- `system_package`
- `service`
- `device`
- `environment_variable`

Dependency checks should be usable by the probe engine. A missing dependency
must produce `dependency_missing` with the dependency id.

## 11. AI Client Rules

AI clients should:

- Discover plugin-backed tools through the runtime manifest or capability graph.
- Select by `tool_id`, not `plugin_id`, unless installing or debugging plugins.
- Build payloads from `input_schema`.
- Expect results matching `output_schema`.
- Request evidence listed in `expected_evidence`.
- Treat unverified, expired, disabled, or high-risk plugin tools as requiring
  extra explanation or approval.

## 12. Human Operator Rules

Human operators should be able to answer these questions from the manifest:

- What does this plugin do?
- Which platforms can run it?
- What command does Worker execute?
- What local dependencies are required?
- Which AI-facing tools does it expose?
- What payload shape and result shape are expected?
- What risks and side effects can it have?
- How does Hub verify that it works?

## 13. Compatibility

Plugin Runtime v2 can wrap v1 plugins when a v2 manifest is provided beside the
old executable.

Migration rules:

- Map v1 `plugin_id` and `action` to a stable v2 `tool_id`.
- Preserve the v1 stdin/stdout shape inside the plugin adapter if needed.
- Add `input_schema`, `output_schema`, `risk`, and `probe` before advertising
  the tool in the cluster manifest.
- Do not expose a v1 plugin as verified until the v2 probe passes.

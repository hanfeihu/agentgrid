# Evidence Pipeline v2 Standard

Evidence Pipeline v2 defines how AgentGrid turns task outputs into durable,
auditable records that both AI clients and human operators can inspect.

AgentGrid tasks should not be trusted only because a Worker returned `ok`.
Tasks are trusted when their result, logs, screenshots, files, reports, and
timeline entries can be connected to the original request, the selected node,
the tool contract, and the scheduler decision.

Schema: `schemas/evidence-item.schema.json`

## 1. Goals

- Give every important execution output a stable evidence record.
- Let AI clients cite concrete proof instead of paraphrasing Worker output.
- Let humans preview screenshots, logs, reports, and files from Web, Mobile,
  CLI, or MCP clients.
- Preserve integrity metadata, including content type, size, hash, producer,
  timestamps, and related task/job ids.
- Keep sensitive evidence manageable through explicit redaction and retention
  fields.

## 2. Non-Goals

- Evidence Pipeline does not decide whether a task should run.
- Evidence Pipeline does not replace the task result contract.
- Evidence Pipeline does not infer truth from natural language.
- Evidence Pipeline does not require all evidence to be stored forever.

## 3. Evidence Item

An evidence item is the smallest durable proof object produced by a task,
job attempt, plugin, desktop helper, probe, reducer, or external event.

Minimum shape:

```json
{
  "api_version": "agentgrid.evidence/v2",
  "kind": "EvidenceItem",
  "metadata": {
    "id": "evidence_01J00000000000000000000000",
    "created_at": "2026-06-01T00:00:00Z",
    "producer": {
      "type": "worker",
      "id": "hub-node"
    }
  },
  "subject": {
    "task_id": "task_xxx",
    "job_id": "job_xxx",
    "job_attempt_id": "attempt_xxx",
    "node_id": "hub-node",
    "tool_id": "command.run",
    "operation": "command.run"
  },
  "evidence": {
    "type": "stdout_log",
    "title": "Command stdout",
    "content_type": "text/plain",
    "storage": {
      "kind": "inline",
      "text": "hello AgentGrid\n"
    },
    "sha256": "9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca7",
    "size_bytes": 16
  },
  "integrity": {
    "verified": true,
    "verification_method": "sha256"
  },
  "visibility": {
    "redaction_state": "none",
    "retention_policy": "default"
  }
}
```

## 4. Evidence Types

Standard evidence types:

- `screenshot`: PNG, JPEG, or WebP visual state.
- `stdout_log`: command, plugin, or tool stdout.
- `stderr_log`: command, plugin, or tool stderr.
- `serial_log`: serial console capture from a hardware bench.
- `file_artifact`: generated, downloaded, uploaded, or transformed file.
- `test_report`: test result, benchmark report, or validation report.
- `operation_timeline`: ordered execution events.
- `structured_result`: JSON result object from a tool, reducer, probe, or job.
- `metric_series`: time-series samples such as CPU, memory, latency, or power.
- `external_reference`: immutable pointer to a record stored outside Hub.

Unknown custom types are not valid in v2. Add a new standard type before using
one in shared contracts.

## 5. Pipeline Stages

The Hub processes evidence through these stages:

```text
captured -> normalized -> stored -> indexed -> verified -> available
```

Failure states:

```text
rejected
quarantined
expired
deleted
```

Stage meanings:

- `captured`: Worker, plugin, desktop helper, or client produced raw output.
- `normalized`: Hub assigned type, content type, size, hash, and subject links.
- `stored`: inline content, artifact id, URI, or external reference is recorded.
- `indexed`: evidence is linked to task, job, attempt, shard, node, and tool.
- `verified`: integrity checks passed, when verification is available.
- `available`: evidence may be shown to AI clients and humans.
- `rejected`: evidence did not match schema or policy.
- `quarantined`: evidence exists but should not be displayed by default.
- `expired`: retention policy removed payload access while preserving metadata.
- `deleted`: metadata and payload are intentionally removed where policy allows.

## 6. Producer Contract

Evidence producers must:

- Include `task_id` when evidence belongs to a task.
- Include `job_id`, `job_attempt_id`, and `job_shard_id` when evidence belongs
  to Job Runtime.
- Include `node_id` when evidence was produced on a Worker node.
- Include `tool_id` for tool, plugin, probe, reducer, and task-result evidence.
- Use a standard evidence type.
- Provide either inline content, an artifact id, a URI, or an external reference.
- Provide `content_type`, `size_bytes`, and `sha256` when payload bytes are
  known.
- Mark redaction state honestly. Do not label sensitive content as `none`.

## 7. Consumer Contract

AI clients should:

- Prefer evidence items over free-form task summaries when explaining results.
- Cite `evidence.metadata.id`, `subject.task_id`, and `subject.tool_id` when
  reporting important conclusions.
- Treat `quarantined`, `expired`, `deleted`, or unverified evidence as lower
  confidence.
- Avoid displaying raw inline content when `visibility.redaction_state` is
  `redacted` or `restricted`.
- Use `preview` fields for UI display before downloading large artifacts.

Human-facing clients should:

- Show title, type, producer, timestamp, node, tool, and verification status.
- Provide safe previews for images, logs, reports, and JSON.
- Preserve download links and hashes for audit workflows.
- Show redaction and retention status near the evidence, not hidden in details.

## 8. Storage Modes

`evidence.storage.kind` controls where the payload lives:

- `inline`: small text or JSON stored directly in the evidence record.
- `artifact`: payload stored in AgentGrid artifact storage.
- `uri`: payload retrievable through a Hub-controlled URL or object-store URI.
- `external`: immutable reference to an external system.
- `none`: metadata-only evidence, usually for deleted or unavailable payloads.

Inline payloads should stay small. Large logs, screenshots, reports, and files
should use `artifact` or `uri`.

## 9. Integrity

For byte payloads, `sha256` and `size_bytes` are required once the item reaches
`stored`.

`integrity.verified` means AgentGrid verified the stored payload against the
recorded metadata. It does not mean the task result is semantically correct.

Recommended verification methods:

- `sha256`
- `artifact_store`
- `signed_manifest`
- `external_system`

## 10. Redaction And Retention

Redaction states:

- `none`: no known sensitive content.
- `redacted`: sensitive content was removed or masked.
- `restricted`: payload should be visible only to authorized operators.
- `unknown`: producer could not determine sensitivity.

Retention policies:

- `default`: keep according to Hub default.
- `short`: keep only for transient debugging.
- `long`: keep for audit or compliance.
- `manual`: do not expire without operator action.

Redaction does not remove the need for evidence. It changes how evidence is
stored, previewed, and exposed.

## 11. Error Handling

If evidence capture fails, the task result should still report the execution
state, but the execution record must include an evidence error item or timeline
entry.

Standard evidence error codes:

- `evidence_schema_invalid`
- `evidence_payload_missing`
- `evidence_hash_mismatch`
- `evidence_storage_failed`
- `evidence_redaction_required`
- `evidence_type_unsupported`
- `evidence_retention_expired`

## 12. AI Planning Rules

When an AI submits work, it should request evidence that matches the risk of
the task:

- Desktop operation: before and after screenshots plus operation timeline.
- Command execution: stdout, stderr, exit code, and structured result.
- Hardware test: serial log, flash log, test report, and final summary.
- Plugin execution: plugin result, declared artifacts, and probe status.
- Job Runtime: per-attempt evidence plus reducer evidence.

A task that mutates real machines or devices should not be considered complete
until required evidence is available or an explicit evidence failure is present.

## 13. Compatibility

Evidence Pipeline v2 is compatible with v1 evidence records when the Hub can
normalize them into `EvidenceItem`.

Migration rules:

- Map v1 `artifacts[]` entries to v2 `EvidenceItem` records.
- Map v1 `audit[]` entries to `operation_timeline` evidence or task events.
- Preserve original ids in `metadata.correlation_id` or `links`.
- Do not silently drop old evidence fields. Store unknown v1 details in
  `annotations` when needed.

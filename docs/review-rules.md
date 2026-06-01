# AgentGrid Phase 1 Review Rules

This document defines the first-stage review rules for AgentGrid code,
protocol, API, and documentation changes.

Reviewers should use these rules to decide whether a change can merge, needs
revision, or should be split before review.

## 1. Review Goals

Phase 1 is focused on making the Hub and the single-machine runtime foundation
stable enough for AI employees and future compute workers.

Every review should protect:

- Protocol compatibility for `agentgrid.io/v1` and `agentmessage.io/v1`.
- Clear module boundaries between protocol, runtime, storage, execution, Hub,
  and apps.
- Conservative security defaults for HTTP jobs, command jobs, secrets, logs,
  and local resources.
- Documentation that stays synchronized with behavior, schemas, and examples.
- Small changes that can be understood, tested, and rolled back.

## 2. Blocking Issues

A change must not merge while any blocking issue remains.

### 2.1 Protocol Compatibility

Block the change if it:

- Removes, renames, or changes the meaning of a public protocol field without a
  versioning plan.
- Changes required states for jobs, nodes, tasks, messages, attempts, leases,
  or results without updating documentation and examples.
- Introduces an object that does not include the expected `api_version`, `kind`,
  `metadata`, `spec`, or `status` shape when that shape is part of the public
  contract.
- Uses incompatible names for the same concept, such as mixing `Task` and
  `Job` in compute runtime contracts without a migration explanation.
- Stores machine-readable protocol data only in human text instead of structured
  fields.
- Changes AgentMessage task collaboration semantics without sending a
  `contract.changed` message.

### 2.2 Module Boundaries

Block the change if it violates the ownership model in `docs/project-structure.md`.

Examples:

- Protocol crates depend on runtime, storage, executor, UI, or Hub crates.
- `agentgrid-core` contains HTTP routing, SQLite implementation details, or
  process execution code.
- `agentgrid-store` owns scheduling decisions or HTTP routes.
- `agentgrid-executor` owns cluster-wide scheduling or policy decisions.
- `agentgrid-policy` executes jobs instead of returning `allow`, `deny`, or
  `ask_user` decisions.
- `agentgrid-worker` decides global scheduling instead of polling for assigned
  work and executing it.
- Compute runtime crates depend on Hub collaboration state.
- Apps contain business logic that belongs in crates.

### 2.3 Security And Policy

Block the change if it weakens conservative defaults or hides risk.

Examples:

- HTTP jobs can call arbitrary hosts without allowlists, dangerous target
  blocking, timeouts, and response size limits.
- Command jobs accept raw shell strings where `program` plus `args` is feasible.
- Command execution lacks allowlists, working directory restrictions, timeouts,
  or stdout/stderr limits.
- Secrets are passed, stored, logged, or returned as raw values when a
  `secret_ref` model should be used.
- Logs, events, errors, or review comments can expose tokens, credentials,
  sensitive headers, or local secret paths.
- Policy decisions are enforced only in UI code instead of runtime services.
- A user approval path is bypassed for high-risk actions.
- Network, file, process, or secret access expands without corresponding policy
  and audit coverage.

### 2.4 State, Persistence, And Reliability

Block the change if it can lose work or corrupt runtime state.

Examples:

- Job, attempt, lease, node, task, or message status transitions are ambiguous
  or impossible to audit.
- Lease expiration, retry, cancellation, or lost-job handling creates duplicate
  execution without an idempotency or dedupe plan.
- Persistent records do not include enough timestamps or IDs to reconstruct what
  happened.
- Writes that must be atomic are split without transaction handling.
- Polling endpoints cannot safely handle repeated requests.
- Task messages requiring acknowledgement do not have a clear acknowledgement
  path.

### 2.5 API And Error Behavior

Block the change if public APIs become hard for AI clients to use safely.

Examples:

- JSON responses are not structured or lack stable error shapes.
- CLI or API behavior cannot produce machine-readable output where required.
- New endpoints are undocumented or missing request and response examples.
- Status codes do not distinguish validation errors, authentication failures,
  policy denials, missing resources, and internal failures.
- Error messages hide the actionable cause or expose sensitive data.

### 2.6 Tests And Verification

Block the change if the risk level requires tests and no equivalent verification
is provided.

Tests are required for:

- Public protocol schema or serialization changes.
- State machine transitions.
- Scheduler filtering and selection decisions.
- Policy allow, deny, and ask-user decisions.
- HTTP and command executor safety limits.
- Store migrations and persistence behavior.
- Worker registration, heartbeat, polling, lease renewal, and completion flow.
- Hub message creation, listing, routing, and task assignment flow.

### 2.7 Documentation Synchronization

Block the change if implementation and documentation disagree on public
behavior.

Documentation must be updated when a change affects:

- Public protocol objects or JSON schemas.
- HTTP API endpoints, payloads, or errors.
- CLI commands or output formats.
- Job, node, attempt, lease, task, or message states.
- Security policy behavior.
- Module ownership or dependency direction.
- Hub employee workflow or AgentMessage types.

## 3. Non-Blocking Suggestions

Reviewers may leave non-blocking comments for improvements that should not stop
merge by themselves.

Examples:

- Naming can be clearer but does not conflict with public contracts.
- A helper could reduce local duplication.
- A small function could be split later for readability.
- Documentation could include one more example, while existing behavior remains
  accurate.
- Logging could include additional context without affecting correctness.
- A test could be made more focused or easier to read.
- An internal type could use stricter validation in a follow-up.
- A future extension point is visible but not needed for the current MVP.

Non-blocking comments should be marked as suggestions and should not be mixed
with merge-blocking requirements.

## 4. Protocol Change Review

Protocol changes require extra care because AI clients, SDKs, workers, Hub
employees, and schemas depend on stable contracts.

Before approving a protocol change, confirm:

- The changed object belongs in the correct protocol family:
  `agentgrid.io/v1` for compute runtime objects, or `agentmessage.io/v1` for
  collaboration objects.
- The change is additive when possible.
- Renames or removals include a migration path or version bump.
- Required and optional fields are explicit.
- State values are documented and examples use the same values.
- JSON schema, Rust types, docs, and examples agree.
- Existing messages or records can still be read.
- A `contract.changed` AgentMessage is sent when shared protocol/API behavior
  changes.

Reviewers should request a `review.requested` message to `review-agent` when a
protocol-owning employee changes a shared contract.

## 5. API Change Review

Before approving an API change, confirm:

- The endpoint path and method match the documented responsibility.
- Request bodies use structured JSON and stable field names.
- Responses include enough machine-readable data for AI clients.
- Pagination or limits exist for list endpoints that may grow.
- Validation errors identify the field and reason.
- Authentication and policy checks run before risky work.
- Side-effecting operations are idempotent or have a documented retry strategy.
- Examples in docs can be copied and run.

For Hub API changes, also confirm that AI employees can still complete this
loop:

```text
list messages -> receive task.assigned -> send task.started
-> send task.progress or task.blocked -> send task.completed
```

## 6. Security Review Checklist

Use this checklist for changes touching execution, networking, storage, secrets,
policy, logs, or user approval.

- Does the change reduce default access rather than expand it silently?
- Are risky HTTP targets blocked by default?
- Are command jobs represented as `program` plus `args`?
- Are timeouts and output limits enforced?
- Are secrets referenced by name instead of copied into payloads?
- Are secrets redacted from logs, errors, events, and results?
- Are policy decisions auditable?
- Can a local user understand and override high-risk decisions?
- Does the test plan include both allowed and denied cases?

## 7. Documentation Review Checklist

Docs should be treated as part of the contract.

Before approving docs or docs-affecting code, confirm:

- Terminology is consistent with the current product direction:
  `Job` for compute runtime work, `AgentTask` for Hub collaboration work.
- Examples use valid JSON and current endpoint paths.
- Required states match `docs/requirements.md` and `docs/hub-api.md`.
- Module ownership matches `docs/project-structure.md`.
- Security guidance does not imply permissive defaults.
- AI employee instructions are operational and include message types when needed.

## 8. Review Communication

Review comments should be concise, actionable, and tied to a rule.

Use this structure for blocking comments:

```text
Blocking: <short issue>
Rule: <section name>
Why: <risk or broken contract>
Requested change: <specific fix>
```

Use this structure for suggestions:

```text
Suggestion: <short improvement>
Why: <benefit>
```

When reporting through AgentMessage:

- Use `review.comment` for review findings.
- Use `review.requested` when another employee should review.
- Use `task.blocked` when a task cannot continue without input.
- Use `task.completed` only after outputs are present and acceptance criteria are
  met.
- Put file paths, rule IDs, task IDs, and machine-readable findings in
  `payload`.

## 9. Minimum Approval Standard

A change is ready to approve when:

- No blocking issues remain.
- Required tests or equivalent verification are complete.
- Public docs, schemas, examples, and implementation agree.
- Security behavior remains conservative by default.
- The change respects module ownership and dependency direction.
- The reviewer can explain the remaining risks in one short paragraph.

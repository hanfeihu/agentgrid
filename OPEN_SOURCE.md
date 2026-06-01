# Open Source License

AgentGrid is released under the Apache License, Version 2.0.

## What This Means

- You may use AgentGrid for personal, research, internal, and commercial work.
- You may modify, distribute, and build products on top of AgentGrid.
- You must keep the Apache-2.0 license notice when redistributing the project.
- Contributions submitted to this repository are accepted under Apache-2.0 unless explicitly stated otherwise.
- The license includes a patent grant from contributors.

## Project Positioning

AgentGrid is not a generic natural-language automation product. Its open standard focuses on:

- AI-operated real machines and hardware workbenches.
- Worker node capability registration and probing.
- Resource-aware task and job scheduling.
- Evidence, artifacts, logs, screenshots, and execution audit.
- Hub-driven orchestration with nodes that actively connect from private networks.

## Trademark And Branding

The Apache-2.0 license grants code rights. It does not grant trademark rights.
If an ecosystem project uses the AgentGrid name, it should clearly state whether it is official or community-maintained.

## Secrets And Deployment Assets

Do not commit production secrets, SMTP authorization codes, SSH passwords, private keys, private Hub URLs, or private server inventories.

Use local environment variables, private deployment notes, or files under ignored paths such as:

```text
docs/private/
private/
*.private.md
```

## Recommended Public Repository Checklist

- Keep `LICENSE` and `NOTICE`.
- Keep package manifests on `Apache-2.0`.
- Remove private deployment credentials before publishing.
- Review generated docs for real hostnames, tokens, emails, and screenshots.
- Add contribution rules before accepting external pull requests.

# Security, persistence and failure contract

Upload is untrusted data. A Markdown command, embedded system prompt, hidden ZIP member or manifest instruction does not grant any tool or network authority. Harden path traversal, symlink handling, archive nesting, decompression ratios, file count/bytes, media verification, secret classification, quota and tenant isolation. Separate inspection, plan acceptance and executable authorization.

A durable controller owns run state. Before effect: record operation ID, scope, lease/fencing token, expected version and intent criterion; after effect: reconcile observed git/GitHub/CI/deployment state. Crash anywhere: rerun reconciliation before retry. Affected irreversible operation must never be blindly replayed. Leases must expire but stale worker completions cannot override newer fencing generations. Stop on policy/authorization failure; continue independent safe work. Support resume on host reboot and provider outage only when an actual controller/runtime is configured.

Fault injection checkpoints: before blob commit, after blob commit, during extraction, during graph reconciliation, before worker spawn, after patch before result, before PR creation, after PR creation, before merge, after remote merge, during deployment, during controller update. Every scenario has expected replay outcome and evidence.

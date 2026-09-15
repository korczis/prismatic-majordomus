# Majordomus ChatGPT Project Provider — Claude Code Prompt Pack

Purpose: implement machine access and subscription-like synchronization for the user's own ChatGPT Projects as a first-class Majordomus provider.

The architecture deliberately separates:

1. official/supported access when available,
2. authenticated Chromium/CDP observation as the robust gray-zone compatibility layer,
3. validated observed private HTTP as an optional fast path,
4. canonical provider models, sync/checkpoints/events and provenance,
5. all Majordomus surfaces derived from one backend.

This is not a DOM scraper project. It is a generic external-workspace-provider subsystem whose first demanding adapter is ChatGPT.

## Recommended execution

### Maximum-quality mode

Use the repository's canonical feature branch/worktree workflow. Run one fresh large-context Claude Code session per phase, in order:

1. `01_REPO_AUDIT_AND_ARCHITECTURE.md`
2. `02_CANONICAL_PROVIDER_CORE.md`
3. `03_BROWSER_CDP_TRANSPORT.md`
4. `04_PROTOCOL_OBSERVATION_PRIVATE_HTTP.md`
5. `05_SYNC_EVENTS_PROVENANCE.md`
6. `06_CHATGPT_MODEL_MAPPING.md`
7. `07_CLI_API_OPENAPI_MCP_COCKPIT.md`
8. `08_DOCS_RULES_DOCTRINES_GHPAGES.md`
9. `09_SECURITY_RESILIENCE_TESTS.md`
10. `10_END_TO_END_CLOSURE.md`

For every phase:
- start from the same feature worktree;
- let Claude rediscover current repo state;
- require implementation + tests + docs + gates, not a report only;
- use repository session/handover machinery rather than manually carrying file lists;
- run the canonical quality gate after meaningful changes;
- do not paste credentials, cookies, HARs with secrets, or private endpoint lists.

### One-shot mode

Use `00_MASTER_ORCHESTRATOR.md`. It contains the full mission and quality bar.

## The short request this architecture should eventually support

After the rules/provider skill exist, a future request should be enough:

> Add or extend ChatGPT Project machine access as a Majordomus external workspace provider. Prefer official access, use authenticated browser/CDP when necessary, use validated observed private HTTP only as a transport optimization, preserve provenance, and update every canonical surface, schema, rule/doctrine, test and generated document automatically.

If future agents need a sacred hand-maintained list of every CLI route, API handler, MCP tool, Cockpit page and docs file to touch, the architecture failed.

## Intended dependency direction

```text
official source ────────────────┐
authenticated browser/CDP ──────┼─> transport adapters
validated private HTTP ─────────┘
                                  |
                                  v
                         sanitized observations
                                  |
                                  v
                       provider-specific mapper
                                  |
                                  v
                    canonical external workspace model
                                  |
                  sync/checkpoint/event/provenance layer
                                  |
               CLI / REST / OpenAPI / MCP / Cockpit / docs
```

No consumer owns its own provider truth.

## Safety boundary

This pack supports automation of the user's own already-authorized workspace. It explicitly does not instruct Claude to bypass MFA, CAPTCHA, access controls, account boundaries, or anti-abuse controls.

# Prompt 13 — Quality Doctrine, Security, Performance, and Complete Test Matrix

## Mission

Make the Cockpit IDE/control-plane evolution difficult to regress.

## Quality doctrine

For every new/changed public module/function/command/capability according to repository conventions:
- documentation;
- tests;
- doctests where required;
- integration tests;
- use-case coverage;
- benchmark cases where required.

Every bug discovered:
1. reproduce;
2. identify root cause;
3. add regression test;
4. fix root cause;
5. cover edge cases;
6. update docs if behavior/architecture changed.

## Test layers

### Unit
- schemas;
- descriptors;
- projection metadata;
- event ordering;
- status transitions;
- parsers/normalizers;
- redaction;
- URL/state encoding.

### Property
- deterministic order;
- stable IDs;
- arbitrary discovery ordering;
- event replay consistency;
- schema round trips.

### Integration
- executor;
- CLI;
- HTTP;
- MCP;
- OpenAPI;
- Cockpit server routes;
- generated docs.

### Browser/e2e
- navigation;
- palette;
- runner;
- streaming;
- reconnect;
- cancellation;
- filters/deep links;
- no-JS baseline;
- accessibility critical path;
- mutation protections.

### Multi-client
- peers;
- claims;
- overlap;
- reconnect;
- handover.

### Git/worktree
- topology;
- misplaced worktree;
- dirty states;
- scope refusal;
- diff.

### Docs
- generation;
- drift;
- links;
- GH Pages build.

## Security review

Threat model:
- XSS from repository-controlled metadata/content;
- command injection;
- path traversal;
- CSRF/origin attacks;
- exposing write operations remotely;
- secret leakage in environment/output/logs;
- malicious file names;
- symlink escapes;
- huge output memory exhaustion;
- websocket/SSE abuse;
- unauthorized remote bind;
- unsafe PTY;
- SSRF if external integrations exist.

Preserve/enhance:
- escaped markup;
- strict CSP;
- same-origin mutation checks;
- loopback/local binding policy unless explicitly configured;
- typed inputs;
- output limits;
- redaction.

Add targeted regression tests.

## Performance

Benchmark:
- process startup;
- shared server reuse;
- registry/index build;
- OpenAPI projection build;
- Cockpit home render;
- capability list render;
- execution events;
- search/palette backend;
- large registries;
- monitoring refresh.

Prove pages do not rebuild immutable startup structures.

## Gates

Wire tests, generation check, docs check, schema check, parity check and benchmark coverage into canonical local/CI gates.

Do not create fifteen unrelated CI workflows if one existing quality pipeline should own them.

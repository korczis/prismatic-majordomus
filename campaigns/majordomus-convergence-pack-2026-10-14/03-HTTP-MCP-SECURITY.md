# Mission 03 — P0 HTTP/UI/MCP attack surface

Investigate #996 (I2151), #995 (I2147), #980 (I2123), other latest relevant issues, current source and docs. Distinguish local-only from user-exposed ports and mesh-bound listeners.

1. Asset/threat inventory: capabilities with read/write semantics, auth credentials, browser origins, reverse proxy assumptions, host allowlist, local network attackers, DNS rebinding, malicious web pages, XSS, CSRF, SSRF, CORS, CSP, mixed content, WebSocket/SSE auth, request smuggling and file path traversal.
2. Bind authority to origin/host/auth token and effective peer trust. Localhost is not a sufficient security boundary against browser DNS rebinding. Use explicit accepted host/origin policy; reject rebinding and cross-site unauthorized writes. Avoid storing bearer tokens in URL or HTML/JS logs. Test request normalization/casing/IP forms, IPv4/IPv6, proxy forwarded headers (trusted proxy only), DNS rebinding simulation.
3. Static UI: implement restrictive CSP compatible with the actual asset pipeline, no unsafe remote scripts by default, locally pinned and integrity-checked p5/vendor assets where appropriate. Validate rendering and UX after CSP; don't silently disable CSP for cosmetics.
4. Mutations: capability-specific authorization, validation, idempotency where needed, bounded body/frame sizes, timeouts and anti-CSRF protections on browser-driven state changes. Read-only state access must not incidentally execute commands or touch secrets.
5. MCP Streamable HTTP: verify transport protocol response semantics, session IDs, Origin requirements, negotiated versions, response size, SSE reconnect/cursor and cancellation; use protocol compliance fixtures. Separate public schema introspection from sensitive data.
6. Rust and shell exec surfaces: trace whether any query parameter, API body, MCP call or displayed artifact can end in shell execution, path write, system process, or network call. Confirm principle least authority and refuse privilege escalation.
7. Logging: redaction must apply before every consumer, including trace events, GitHub review body, screenshots, bug reports and persisted evidence. Add negative tests with synthetic credentials, no real credentials in fixtures.
8. Fuzz critical parsers and HTTP boundaries, set byte/time/count limits, document authentication and transport limitations honestly.

Acceptance: anonymous or cross-origin mutations are denied with no side effects; DNS rebinding test passes; unsafe scripts blocked by tested CSP; MCP behavior meets its declared spec; secrets not emitted; no skipped gate. Evidence must include exact tests and surfaces.

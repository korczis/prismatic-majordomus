# Serving: the shared server this repository runs, its two user interfaces, and the
# assets one of them needs. One server per repository; every client attaches to it.

# MCP on stdio for the client that spawned it, joining or starting the repository's one shared server (its home page at / lists every surface). Extra arguments pass through.
[group('serve')]
mcp *args: build
    "{{rust_bin}}" mcp {{args}}

# The shared server alone, on 127.0.0.1:8741 by default: the home page /, the documentation /docs/, the Cockpit /cockpit, Swagger UI /swagger, /openapi.json, MCP over HTTP /mcp. Exits 0 if one already runs.
[group('serve')]
serve *args: build
    "{{rust_bin}}" serve {{args}}

# What `mcp` would serve, and every diagnostic; exit 10 when the layer is degraded.
[group('serve')]
inspect *args: build
    "{{rust_bin}}" mcp --inspect {{args}}

# Where the repository's shared server is, if one runs (from the lease under .ai/local/).
[group('serve')]
mcp-status:
    @f=".ai/local/state/mcp/server.json"; if [ -f "$f" ]; then cat "$f"; echo; else echo "no shared server lease at $f"; fi

# Open Swagger UI of the running shared server in the browser (macOS `open`, else xdg-open).
[group('serve')]
swagger-ui:
    @f=".ai/local/state/mcp/server.json"; [ -f "$f" ] || { echo "no shared server is running (just serve, or start an MCP client)"; exit 1; }; \
    url="$(sed -n 's/.*"url":"\([^"]*\)".*/\1/p' "$f")"; echo "$url/swagger"; (command -v open >/dev/null && open "$url/swagger") || xdg-open "$url/swagger"

# Open the running shared server's home page — every surface it serves, derived from the topology.
[group('serve')]
home:
    @f=".ai/local/state/mcp/server.json"; [ -f "$f" ] || { echo "no shared server is running (just serve, or start an MCP client)"; exit 1; }; \
    url="$(sed -n 's/.*"url":"\([^"]*\)".*/\1/p' "$f")"; echo "$url/"; (command -v open >/dev/null && open "$url/") || xdg-open "$url/"

# Open the Cockpit of the running shared server in the browser (macOS `open`, else xdg-open).
[group('serve')]
cockpit:
    @f=".ai/local/state/mcp/server.json"; [ -f "$f" ] || { echo "no shared server is running (just serve, or start an MCP client)"; exit 1; }; \
    url="$(sed -n 's/.*"url":"\([^"]*\)".*/\1/p' "$f")"; echo "$url/cockpit"; (command -v open >/dev/null && open "$url/cockpit") || xdg-open "$url/cockpit"

# Build the Cockpit's static assets: compile share/cockpit/cockpit.css, vendor the pinned libraries (needs npm ci).
[group('serve')]
cockpit-assets:
    scripts/cockpit-assets

# The committed Cockpit stylesheet matches its source; exit 10 when it is stale.
[group('serve')]
cockpit-assets-check:
    scripts/cockpit-assets --check

# Every Cockpit route the server serves, answered; then the pages and the interactions in a real browser.
[group('serve')]
cockpit-probe *args:
    scripts/cockpit-probe {{args}}
